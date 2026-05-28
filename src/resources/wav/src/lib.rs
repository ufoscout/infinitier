#![doc = include_str!("../readme.md")]

use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

use log::debug;
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use thiserror::Error as ThisError;

use infinitier_acm_decoder::{AcmDecoder, OutputChannels};
use infinitier_datasource::{DataSource, DataTrait, Importer, ReadExt, Reader};

pub type Result<T> = std::result::Result<T, WavError>;

/// A WAV / WAVC / OGG sound resource importer.
pub struct WavImporter<'a> {
    pub name: &'a str,
}

impl Importer for WavImporter<'_> {
    type T = WavDecoder;

    fn import(&self, source: &DataSource) -> io::Result<Self::T> {
        Ok(WavDecoder::open(source, self.name)?)
    }
}


#[derive(Debug, ThisError)]
pub enum WavError {
    #[error(
        "not a WAV / WAVC / OGG file: unknown magic {:?} ({:02x?})",
        std::str::from_utf8(.0).unwrap_or("<non-utf8>"),
        .0
    )]
    UnknownFormat([u8; 4]),
    #[error(
        "unsupported PCM format: bits_per_sample={bits}, format_tag=0x{format_tag:04x} (only 8- \
         and 16-bit PCM, format_tag=0x0001, is supported)"
    )]
    UnsupportedPcmFormat { bits: u16, format_tag: u16 },
    #[error("malformed RIFF/WAVE: {0}")]
    MalformedWav(&'static str),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("acm decoder error: {0}")]
    Acm(#[from] infinitier_acm_decoder::AcmError),
    #[error("symphonia error: {0}")]
    Symphonia(#[from] SymphoniaError),
    #[error("ogg metadata: {0}")]
    OggMetadata(&'static str),
    #[error("ogg stream produced too many samples to fit in u32: {0}")]
    OggTooLong(u64),
}

impl From<WavError> for io::Error {
    fn from(err: WavError) -> Self {
        match err {
            WavError::Io(e) => e,
            other => io::Error::other(other),
        }
    }
}

/// Stream metadata, mirroring [`infinitier_acm_decoder::AcmInfo`] so callers
/// can treat both decoders uniformly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WavInfo {
    pub channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    /// Total interleaved PCM values the stream will produce
    /// (`channels × frames`).
    pub total_values: u32,
}

impl WavInfo {
    /// Number of frames (samples per channel) in the stream.
    pub fn frames(&self) -> u32 {
        self.total_values / self.channels.max(1) as u32
    }
}

/// The flavours of `*.WAV` resource shipped by Infinity Engine games.
///
/// Note that Enhanced Editions also bundle Ogg/Vorbis streams under the
/// `.WAV` extension; this variant covers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WavFormat {
    /// Standard `RIFF`/`WAVE` PCM.
    Wav,
    /// Interplay's WAVC: a 28-byte header wrapping an ACM stream.
    Wavc,
    /// Ogg-encapsulated Vorbis audio (Enhanced Editions ship these
    /// under the `.WAV` extension).
    Ogg,
}

/// Streaming decoder for the two `*.WAV` flavours used by Infinity Engine
/// games. Pulls samples block-by-block from the underlying source so memory
/// stays bounded regardless of file size, mirroring
/// [`AcmDecoder`]'s API.
pub struct WavDecoder {
    info: WavInfo,
    /// Caller-supplied label (resource name, file path, …) prefixed to log
    /// records and forwarded to the inner [`AcmDecoder`] for WAVC sources,
    /// so consumers decoding many streams can tell entries apart.
    name: String,
    datasource: DataSource,
    inner: WavInner,
}

enum WavInner {
    /// In-house RIFF/WAVE PCM streamer.
    ///
    /// We hand-parse RIFF rather than using `hound` or `symphonia`'s WAV
    /// reader because some Infinity Engine assets ship with an internally
    /// inconsistent fmt chunk — e.g. `block_align` set to a stereo-sized
    /// value while `channels=1` and `bits_per_sample=16`. Hound rejects
    /// those outright (`Error::FormatError("inconsistent fmt chunk")`,
    /// `hound-3.5.1/src/read.rs:421`); symphonia accepts the file at
    /// probe time but trusts the literal `block_align` to size packets,
    /// silently producing roughly half the samples. NearInfinity and
    /// gemrb just ignore `block_align` for PCM streams. We do the same:
    /// `block_align` and `byte_rate` are redundant fields, fully
    /// derivable from `channels × bits_per_sample`, so we recompute them
    /// instead of believing the file. See
    /// `assets/WAV/broken/AFT_M01.md` for a real-world example.
    Wav(RiffPcmReader),
    Wavc(AcmDecoder),
    /// Streaming Ogg/Vorbis decoder via Symphonia. The format reader's
    /// initial probe seeks to the file's tail and reads the last Ogg
    /// page's granule_position, so `WavInfo::total_values` is exact
    /// without decoding the whole stream up-front. Samples are decoded
    /// packet-by-packet as `read_samples` requests them.
    Ogg(OggState),
}

struct OggState {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    /// Decoded samples not yet handed out via `read_samples`. Stored
    /// as `Vec<i16>` + a cursor instead of `VecDeque<i16>` because the
    /// access pattern is "fill once per packet, drain linearly" — a
    /// flat Vec gives better cache locality, lets us copy the
    /// whole packet in one `memcpy`, and avoids `pop_front` overhead.
    pending: Vec<i16>,
    /// Index of the next unconsumed sample inside `pending`. When
    /// `consumed == pending.len()` the buffer is reset to empty.
    consumed: usize,
    /// Set once `next_packet` returned a clean end-of-stream so we
    /// stop polling the format reader on subsequent calls.
    eos: bool,
}

impl OggState {
    fn read_samples(&mut self, out: &mut [i16]) -> Result<usize> {
        let mut written = 0usize;
        while written < out.len() {
            // Fast path: copy from the pending buffer in bulk.
            if self.consumed < self.pending.len() {
                let avail = self.pending.len() - self.consumed;
                let want = (out.len() - written).min(avail);
                out[written..written + want]
                    .copy_from_slice(&self.pending[self.consumed..self.consumed + want]);
                self.consumed += want;
                written += want;
                if self.consumed == self.pending.len() {
                    self.pending.clear();
                    self.consumed = 0;
                }
                continue;
            }
            if self.eos {
                break;
            }

            // Refill the buffer from the next packet.
            let packet = match self.format.next_packet() {
                Ok(Some(p)) => p,
                Ok(None) => {
                    self.eos = true;
                    break;
                }
                Err(SymphoniaError::ResetRequired) => {
                    // Chained Vorbis bitstream — not common in BG data.
                    // Treat as natural EOS for now.
                    self.eos = true;
                    break;
                }
                Err(e) => return Err(e.into()),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let n_samples = decoded.samples_interleaved();
                    let prev_len = self.pending.len();
                    self.pending.resize(prev_len + n_samples, 0);
                    decoded.copy_to_slice_interleaved::<i16, _>(&mut self.pending[prev_len..]);
                }
                Err(SymphoniaError::DecodeError(_)) => {
                    // Bad packet — Symphonia recommends skipping and
                    // continuing.
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }
        Ok(written)
    }
}

const RIFF_MAGIC: &[u8; 4] = b"RIFF";
const WAVE_MAGIC: &[u8; 4] = b"WAVE";
const FMT_MAGIC: &[u8; 4] = b"fmt ";
const DATA_MAGIC: &[u8; 4] = b"data";
/// `WAVE_FORMAT_PCM` — uncompressed integer PCM. The only fmt tag we accept.
const FORMAT_PCM: u16 = 0x0001;

/// Streaming reader for the data chunk of a RIFF/WAVE PCM file.
///
/// Created by [`open_riff_pcm`], which has already walked the RIFF
/// header and positioned the underlying reader at the first sample
/// byte of the `data` chunk.
struct RiffPcmReader {
    reader: Reader<Box<dyn DataTrait>>,
    /// 8 or 16; validated up front by [`open_riff_pcm`].
    bits_per_sample: u16,
    /// Bytes left to consume in the data chunk. Decremented as samples
    /// are read; reads return `Ok(0)` once it hits zero.
    data_remaining: u64,
}

impl RiffPcmReader {
    fn read_samples(&mut self, out: &mut [i16]) -> Result<usize> {
        if out.is_empty() || self.data_remaining == 0 {
            return Ok(0);
        }
        let bytes_per = (self.bits_per_sample / 8) as u64;
        let max_values = ((self.data_remaining / bytes_per) as usize).min(out.len());
        if max_values == 0 {
            return Ok(0);
        }

        // Stack-allocated I/O scratch — sized so a single read covers
        // up to 2048 i16 samples (4 KB) regardless of bit depth.
        let mut scratch = [0u8; 4096];
        let chunk_capacity = scratch.len() / bytes_per as usize;
        let mut written = 0usize;
        while written < max_values {
            let n = (max_values - written).min(chunk_capacity);
            let bytes = n * bytes_per as usize;
            self.reader.read_exact(&mut scratch[..bytes])?;
            if self.bits_per_sample == 16 {
                for i in 0..n {
                    out[written + i] = i16::from_le_bytes([scratch[i * 2], scratch[i * 2 + 1]]);
                }
            } else {
                // 8-bit unsigned PCM. Center on 0 and shift to fill the
                // i16 range — same conversion gemrb's WAVReader applies
                // for `wBitsPerSample == 8`.
                for i in 0..n {
                    out[written + i] = ((scratch[i] as i16) - 128) << 8;
                }
            }
            written += n;
            self.data_remaining -= bytes as u64;
        }
        Ok(written)
    }
}

/// Walk a RIFF/WAVE header and return stream metadata plus a streaming
/// reader positioned at the first byte of the `data` chunk.
///
/// We deliberately *ignore* the literal `block_align` and
/// `nAvgBytesPerSec` fields in the fmt chunk — they're redundant
/// (derivable from `channels × bits_per_sample × sample_rate`) and some
/// in-the-wild assets ship them with bogus values that would break
/// strict parsers like hound or symphonia. See
/// `assets/WAV/broken/AFT_M01.md` for a worked example.
fn open_riff_pcm(mut reader: Reader<Box<dyn DataTrait>>) -> Result<(WavInfo, RiffPcmReader)> {
    let head: [u8; 12] = reader.read_exact_to_array()?;
    if &head[0..4] != RIFF_MAGIC {
        return Err(WavError::MalformedWav("missing RIFF marker"));
    }
    if &head[8..12] != WAVE_MAGIC {
        return Err(WavError::MalformedWav("RIFF form is not WAVE"));
    }

    let mut fmt: Option<(u16, u16, u32, u16)> = None; // tag, channels, rate, bits

    loop {
        let hdr: [u8; 8] = match reader.read_exact_to_array() {
            Ok(b) => b,
            Err(_) => return Err(WavError::MalformedWav("missing data chunk")),
        };
        let chunk_id: [u8; 4] = hdr[0..4].try_into().unwrap();
        let chunk_size = u32::from_le_bytes(hdr[4..8].try_into().unwrap());

        match &chunk_id {
            b if b == FMT_MAGIC => {
                if chunk_size < 16 {
                    return Err(WavError::MalformedWav("fmt chunk too small"));
                }
                let body: [u8; 16] = reader.read_exact_to_array()?;
                let format_tag = u16::from_le_bytes(body[0..2].try_into().unwrap());
                let channels = u16::from_le_bytes(body[2..4].try_into().unwrap());
                let sample_rate = u32::from_le_bytes(body[4..8].try_into().unwrap());
                // body[8..12]  = byte_rate    — ignored, see fn doc.
                // body[12..14] = block_align  — ignored, see fn doc.
                let bits = u16::from_le_bytes(body[14..16].try_into().unwrap());
                fmt = Some((format_tag, channels, sample_rate, bits));

                // Skip any extension fields (cb_size + extras) plus the
                // RIFF 1-byte pad if the chunk size is odd.
                let extra = chunk_size as i64 - 16 + (chunk_size & 1) as i64;
                if extra > 0 {
                    reader.seek(SeekFrom::Current(extra))?;
                }
            }
            b if b == DATA_MAGIC => {
                let (format_tag, channels, sample_rate, bits) =
                    fmt.ok_or(WavError::MalformedWav("data chunk before fmt chunk"))?;
                if format_tag != FORMAT_PCM || (bits != 8 && bits != 16) {
                    return Err(WavError::UnsupportedPcmFormat { bits, format_tag });
                }
                if channels == 0 {
                    return Err(WavError::MalformedWav("zero channels"));
                }
                let bytes_per = (bits / 8) as u32;
                let total_values = chunk_size / bytes_per;
                let info = WavInfo {
                    channels,
                    sample_rate,
                    bits_per_sample: bits,
                    total_values,
                };
                let pcm = RiffPcmReader {
                    reader,
                    bits_per_sample: bits,
                    data_remaining: chunk_size as u64,
                };
                return Ok((info, pcm));
            }
            _ => {
                let skip = chunk_size as i64 + (chunk_size & 1) as i64;
                reader.seek(SeekFrom::Current(skip))?;
            }
        }
    }
}

/// Write a 44-byte canonical RIFF/WAVE PCM header for the given
/// stream. Caller follows up by writing `total_values × 2` bytes of
/// little-endian i16 samples (`decode_to_file` always emits 16-bit).
fn write_riff_pcm_header<W: Write>(w: &mut W, info: &WavInfo) -> io::Result<()> {
    let bits: u16 = 16;
    let bytes_per_sample = u32::from(bits / 8);
    let block_align = info.channels * (bits / 8);
    let byte_rate = info.sample_rate * u32::from(block_align);
    let data_size = info.total_values * bytes_per_sample;
    let riff_size = 36 + data_size;

    w.write_all(RIFF_MAGIC)?;
    w.write_all(&riff_size.to_le_bytes())?;
    w.write_all(WAVE_MAGIC)?;
    w.write_all(FMT_MAGIC)?;
    w.write_all(&16u32.to_le_bytes())?;
    w.write_all(&FORMAT_PCM.to_le_bytes())?;
    w.write_all(&info.channels.to_le_bytes())?;
    w.write_all(&info.sample_rate.to_le_bytes())?;
    w.write_all(&byte_rate.to_le_bytes())?;
    w.write_all(&block_align.to_le_bytes())?;
    w.write_all(&bits.to_le_bytes())?;
    w.write_all(DATA_MAGIC)?;
    w.write_all(&data_size.to_le_bytes())?;
    Ok(())
}

impl std::fmt::Debug for WavDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WavDecoder")
            .field("name", &self.name)
            .field("format", &self.format())
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}

impl WavDecoder {
    /// Open a WAV / WAVC / Ogg-Vorbis stream from a [`DataSource`].
    /// The first four bytes determine the flavour:
    ///
    /// - `RIFF` → standard PCM, decoded by our in-house parser (see
    ///   [`RiffPcmReader`] for why we don't use `hound` or `symphonia`);
    /// - `WAVC` → Interplay's ACM-wrapping header, delegated to
    ///   [`AcmDecoder`] (which skips the 28-byte WAVC header itself);
    /// - `OggS` → Ogg-encapsulated Vorbis, decoded via Symphonia.
    ///   Enhanced Edition titles ship Vorbis audio under the `.WAV`
    ///   extension — we transparently accept that.
    ///
    /// `name` is a caller-supplied label (resource id, file path, …) that
    /// gets prefixed to every log record this decoder emits and is
    /// forwarded to the inner [`AcmDecoder`] for WAVC sources.
    pub fn open(datasource: &DataSource, name: impl Into<String>) -> Result<Self> {
        let name = name.into();
        let magic: ([u8; 4], _) = datasource.reader()?.read_at_most_to_array()?;
        match &magic.0 {
            b"RIFF" => Self::open_riff(datasource, name),
            b"WAVC" => Self::open_wavc(datasource, name),
            b"OggS" => Self::open_ogg(datasource, name),
            _ => Err(WavError::UnknownFormat(magic.0)),
        }
    }

    fn open_riff(datasource: &DataSource, name: String) -> Result<Self> {
        let (info, pcm) = open_riff_pcm(datasource.reader()?)?;

        debug!(
            "[{}] Opened WAV: channels={}, rate={}, bits={}, total_values={}",
            name, info.channels, info.sample_rate, info.bits_per_sample, info.total_values,
        );

        Ok(Self {
            info,
            name,
            datasource: datasource.clone(),
            inner: WavInner::Wav(pcm),
        })
    }

    fn open_wavc(datasource: &DataSource, name: String) -> Result<Self> {
        // AcmDecoder already validates the WAVC header (`'WAVC'`, `'V1.0'`,
        // 28-byte length) before reading the ACM body, so we don't re-parse
        // it here.
        let decoder = AcmDecoder::open(datasource, OutputChannels::Original, name.clone())?;
        let acm_info = decoder.info.clone();

        let info = WavInfo {
            channels: acm_info.channels as u16,
            sample_rate: acm_info.rate,
            bits_per_sample: acm_info.bits_per_sample(),
            total_values: acm_info.total_values,
        };

        debug!(
            "[{}] Opened WAVC: channels={}, rate={}, bits={}, total_values={}",
            name, info.channels, info.sample_rate, info.bits_per_sample, info.total_values,
        );

        Ok(Self {
            info,
            name,
            datasource: datasource.clone(),
            inner: WavInner::Wavc(decoder),
        })
    }

    fn open_ogg(datasource: &DataSource, name: String) -> Result<Self> {
        let media_source = DataTraitMediaSource::new(datasource.reader()?)?;
        let mss = MediaSourceStream::new(Box::new(media_source), Default::default());

        let mut hint = Hint::new();
        hint.with_extension("ogg");

        let format = symphonia::default::get_probe().probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )?;

        let track = format
            .default_track(TrackType::Audio)
            .ok_or(WavError::OggMetadata("no default audio track"))?;
        let track_id = track.id;

        let frames_per_channel = track.num_frames.unwrap_or(0);
        let audio_params = track
            .codec_params
            .as_ref()
            .and_then(|cp| cp.audio())
            .ok_or(WavError::OggMetadata("missing audio codec params"))?
            .clone();

        let channels = audio_params
            .channels
            .as_ref()
            .map(|c| c.count() as u16)
            .ok_or(WavError::OggMetadata("missing channel count"))?;
        let sample_rate = audio_params
            .sample_rate
            .ok_or(WavError::OggMetadata("missing sample rate"))?;
        let total_values_u64 = frames_per_channel.saturating_mul(channels as u64);
        let total_values =
            u32::try_from(total_values_u64).map_err(|_| WavError::OggTooLong(total_values_u64))?;

        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&audio_params, &AudioDecoderOptions::default())?;

        let info = WavInfo {
            channels,
            sample_rate,
            bits_per_sample: 16,
            total_values,
        };

        debug!(
            "[{}] Opened OGG: channels={}, rate={}, bits=16, total_values={}",
            name, info.channels, info.sample_rate, info.total_values,
        );

        Ok(Self {
            info,
            name,
            datasource: datasource.clone(),
            inner: WavInner::Ogg(OggState {
                format,
                decoder,
                track_id,
                pending: Vec::new(),
                consumed: 0,
                eos: false,
            }),
        })
    }

    /// Stream metadata.
    pub fn info(&self) -> &WavInfo {
        &self.info
    }

    /// Caller-supplied label passed at [`open`](Self::open) time — useful
    /// for logging or surfacing in a UI.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Container flavour the decoder is reading.
    pub fn format(&self) -> WavFormat {
        match &self.inner {
            WavInner::Wav { .. } => WavFormat::Wav,
            WavInner::Wavc { .. } => WavFormat::Wavc,
            WavInner::Ogg { .. } => WavFormat::Ogg,
        }
    }

    /// Decode the next chunk of PCM samples into `out`, returning the number
    /// of `i16` samples written. Returns `Ok(0)` only on natural end of
    /// stream. Samples are interleaved (frame-major) for stereo streams.
    pub fn read_samples(&mut self, out: &mut [i16]) -> Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        match &mut self.inner {
            WavInner::Wav(pcm) => pcm.read_samples(out),
            WavInner::Wavc(dec) => Ok(dec.read_samples(out)?),
            WavInner::Ogg(state) => Ok(state.read_samples(out)?),
        }
    }

    /// Decode the entire stream and return all PCM samples as interleaved
    /// signed 16-bit values.
    pub fn decode_all(&mut self) -> Result<Vec<i16>> {
        let total = self.info.total_values as usize;
        let mut samples = vec![0i16; total];
        let mut written = 0usize;
        while written < total {
            let n = self.read_samples(&mut samples[written..])?;
            if n == 0 {
                break;
            }
            written += n;
        }
        Ok(samples)
    }

    /// Decode the stream into a 16-bit PCM `RIFF`/`WAVE` file.
    pub fn decode_to_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let mut writer = io::BufWriter::new(std::fs::File::create(path)?);
        write_riff_pcm_header(&mut writer, &self.info)?;

        let mut samples = [0i16; 4096];
        let mut bytes = [0u8; 4096 * 2];
        loop {
            let n = self.read_samples(&mut samples)?;
            if n == 0 {
                break;
            }
            for i in 0..n {
                let le = samples[i].to_le_bytes();
                bytes[i * 2] = le[0];
                bytes[i * 2 + 1] = le[1];
            }
            writer.write_all(&bytes[..n * 2])?;
        }
        writer.flush()?;
        Ok(())
    }

    /// Rewind the decoder to the start of the stream by reopening the
    /// underlying [`DataSource`]. The original `name` is preserved.
    pub fn reset(&mut self) -> Result<()> {
        let fresh = Self::open(&self.datasource, self.name.clone())?;
        *self = fresh;
        Ok(())
    }
}

/// Adaptor that lets a [`Box<dyn DataTrait>`] (the streaming reader our
/// [`DataSource`] hands out) flow directly into Symphonia's
/// [`MediaSourceStream`].
///
/// `MediaSource` is just `Read + Seek + Send + Sync` plus two
/// metadata methods. `DataTrait` already promises all four
/// supertraits, so the [`Read`] / [`Seek`] impls just delegate. The
/// only tricky bit is `byte_len`, which symphonia uses to seek
/// directly to the file tail on probe (so the Ogg demuxer can read
/// the last page's `granule_position` cheaply); `MediaSource::byte_len`
/// takes `&self`, so we measure it once at construction time and
/// cache it.
struct DataTraitMediaSource {
    inner: Reader<Box<dyn DataTrait>>,
    byte_len: Option<u64>,
}

impl DataTraitMediaSource {
    fn new(mut inner: Reader<Box<dyn DataTrait>>) -> io::Result<Self> {
        // Stash the cursor, seek to end to measure, restore. For a
        // file this is two `lseek`s; for an in-memory cursor it's
        // arithmetic on the slice length.
        let pos = inner.stream_position()?;
        let len = inner.seek(SeekFrom::End(0))?;
        inner.seek(SeekFrom::Start(pos))?;
        Ok(Self {
            inner,
            byte_len: Some(len),
        })
    }
}

impl io::Read for DataTraitMediaSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Seek for DataTraitMediaSource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

impl MediaSource for DataTraitMediaSource {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        self.byte_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_magic_is_rejected() {
        let bytes = b"XYZW....".to_vec();
        let err = WavDecoder::open(&DataSource::new(bytes), "test").unwrap_err();
        assert!(matches!(err, WavError::UnknownFormat(_)));
    }

    #[test]
    fn truncated_input_is_rejected() {
        let bytes = b"WA".to_vec();
        let err = WavDecoder::open(&DataSource::new(bytes), "test").unwrap_err();
        // Two bytes pad to `[W, A, 0, 0]` and miss both magics.
        assert!(matches!(err, WavError::UnknownFormat(_)));
    }
}
