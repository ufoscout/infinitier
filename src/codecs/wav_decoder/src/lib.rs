#![doc = include_str!("../readme.md")]

use std::io::{self, Seek, SeekFrom};
use std::path::Path;

use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use log::debug;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use thiserror::Error as ThisError;

use infinitier_acm_decoder::{AcmDecoder, OutputChannels};
use infinitier_datasource::{DataSource, DataTrait, Reader};

pub type Result<T> = std::result::Result<T, WavError>;

#[derive(Debug, ThisError)]
pub enum WavError {
    #[error(
        "not a WAV / WAVC / OGG file: unknown magic {:?} ({:02x?})",
        std::str::from_utf8(.0).unwrap_or("<non-utf8>"),
        .0
    )]
    UnknownFormat([u8; 4]),
    #[error(
        "unsupported PCM format: bits_per_sample={bits}, sample_format={fmt:?} (only 16-bit \
         integer PCM is supported)"
    )]
    UnsupportedPcmFormat { bits: u16, fmt: SampleFormat },
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("hound error: {0}")]
    Wav(#[from] hound::Error),
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
    /// hound-driven RIFF/WAVE reader that owns its underlying file handle
    /// (or in-memory cursor) so the decoder is `'static`.
    Wav {
        reader: WavReader<Reader<Box<dyn DataTrait>>>,
        /// Total samples already produced; used to clamp output to the
        /// declared `total_values` if hound reports more.
        produced: u32,
    },
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
    decoder: Box<dyn Decoder>,
    track_id: u32,
    /// Decoded samples not yet handed out via `read_samples`. Stored
    /// as `Vec<i16>` + a cursor instead of `VecDeque<i16>` because the
    /// access pattern is "fill once per packet, drain linearly" — a
    /// flat Vec gives better cache locality, lets us `extend_from_slice`
    /// the whole packet in one `memcpy`, and avoids `pop_front` overhead.
    pending: Vec<i16>,
    /// Index of the next unconsumed sample inside `pending`. When
    /// `consumed == pending.len()` the buffer is reset to empty.
    consumed: usize,
    /// Reused interleaved-i16 buffer for `copy_interleaved_ref`. Held
    /// across packets so we don't allocate a fresh `SampleBuffer` (and
    /// its internal `cap × channels` i16 storage) on every Vorbis
    /// packet. Symphonia's `SampleBuffer` doesn't expose its capacity,
    /// so we track it ourselves and rebuild the buffer only when a
    /// packet's capacity outgrows the previous one.
    sample_buf: Option<SampleBuffer<i16>>,
    sample_buf_cap: u64,
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
                Ok(p) => p,
                Err(SymphoniaError::IoError(e))
                    if e.kind() == io::ErrorKind::UnexpectedEof =>
                {
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
            if packet.track_id() != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let spec = *decoded.spec();
                    let cap = decoded.capacity() as u64;
                    // Re-use the SampleBuffer across packets; only
                    // rebuild when a packet outgrows the previous
                    // capacity.
                    let sb = match self.sample_buf.as_mut() {
                        Some(sb) if cap <= self.sample_buf_cap => sb,
                        _ => {
                            self.sample_buf = Some(SampleBuffer::<i16>::new(cap, spec));
                            self.sample_buf_cap = cap;
                            self.sample_buf.as_mut().unwrap()
                        }
                    };
                    sb.copy_interleaved_ref(decoded);
                    self.pending.extend_from_slice(sb.samples());
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
    /// - `RIFF` → standard PCM, decoded via [`hound`];
    /// - `WAVC` → Interplay's ACM-wrapping header, delegated to
    ///   [`AcmDecoder`] (which skips the 28-byte WAVC header itself);
    /// - `OggS` → Ogg-encapsulated Vorbis, decoded via [`lewton`].
    ///   Enhanced Edition titles ship Vorbis audio under the `.WAV`
    ///   extension — we transparently accept that.
    ///
    /// `name` is a caller-supplied label (resource id, file path, …) that
    /// gets prefixed to every log record this decoder emits and is
    /// forwarded to the inner [`AcmDecoder`] for WAVC sources.
    pub fn open(datasource: &DataSource, name: impl Into<String>) -> Result<Self> {
        let name = name.into();
        let magic: ([u8; 4], _) = datasource.reader()?.read_at_most()?;
        match &magic.0 {
            b"RIFF" => Self::open_riff(datasource, name),
            b"WAVC" => Self::open_wavc(datasource, name),
            b"OggS" => Self::open_ogg(datasource, name),
            _ => Err(WavError::UnknownFormat(magic.0)),
        }
    }

    fn open_riff(datasource: &DataSource, name: String) -> Result<Self> {
        let reader_box = datasource.reader()?;
        let reader = WavReader::new(reader_box)?;
        let spec = reader.spec();

        if spec.sample_format != SampleFormat::Int || spec.bits_per_sample != 16 {
            return Err(WavError::UnsupportedPcmFormat {
                bits: spec.bits_per_sample,
                fmt: spec.sample_format,
            });
        }

        // hound exposes `len()` as total interleaved samples, which is exactly
        // what we want for `total_values`.
        let total_values = reader.len();

        let info = WavInfo {
            channels: spec.channels,
            sample_rate: spec.sample_rate,
            bits_per_sample: spec.bits_per_sample,
            total_values,
        };

        debug!(
            "[{}] Opened WAV: channels={}, rate={}, bits={}, total_values={}",
            name, info.channels, info.sample_rate, info.bits_per_sample, info.total_values,
        );

        Ok(Self {
            info,
            name,
            datasource: datasource.clone(),
            inner: WavInner::Wav {
                reader,
                produced: 0,
            },
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

        let media_source = DataTraitMediaSource::new(datasource.reader()?.data)?;
        let mss = MediaSourceStream::new(Box::new(media_source), Default::default());

        let mut hint = Hint::new();
        hint.with_extension("ogg");

        let probed = symphonia::default::get_probe().format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?;
        let format = probed.format;

        let track = format
            .default_track()
            .ok_or(WavError::OggMetadata("no default track"))?;
        let track_id = track.id;
        let codec_params = track.codec_params.clone();

        let channels = codec_params
            .channels
            .map(|c| c.count() as u16)
            .ok_or(WavError::OggMetadata("missing channel count"))?;
        let sample_rate = codec_params
            .sample_rate
            .ok_or(WavError::OggMetadata("missing sample rate"))?;
        let frames_per_channel = codec_params.n_frames.unwrap_or(0);
        let total_values_u64 = frames_per_channel.saturating_mul(channels as u64);
        let total_values = u32::try_from(total_values_u64)
            .map_err(|_| WavError::OggTooLong(total_values_u64))?;

        let decoder = symphonia::default::get_codecs()
            .make(&codec_params, &DecoderOptions::default())?;

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
                sample_buf: None,
                sample_buf_cap: 0,
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
            WavInner::Wav { reader, produced } => {
                let remaining = self.info.total_values.saturating_sub(*produced) as usize;
                let want = out.len().min(remaining);
                if want == 0 {
                    return Ok(0);
                }

                let mut iter = reader.samples::<i16>();
                let mut written = 0usize;
                while written < want {
                    match iter.next() {
                        Some(Ok(s)) => {
                            out[written] = s;
                            written += 1;
                        }
                        Some(Err(e)) => return Err(e.into()),
                        None => break,
                    }
                }
                *produced += written as u32;
                Ok(written)
            }
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
        let spec = WavSpec {
            channels: self.info.channels,
            sample_rate: self.info.sample_rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut writer = WavWriter::create(path, spec)?;
        let mut buf = [0i16; 4096];
        loop {
            let n = self.read_samples(&mut buf)?;
            if n == 0 {
                break;
            }
            for &s in &buf[..n] {
                writer.write_sample(s)?;
            }
        }
        writer.finalize()?;
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
    inner: Box<dyn DataTrait>,
    byte_len: Option<u64>,
}

impl DataTraitMediaSource {
    fn new(mut inner: Box<dyn DataTrait>) -> io::Result<Self> {
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
