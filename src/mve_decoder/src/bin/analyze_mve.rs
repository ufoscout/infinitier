/// Analyze MVE file segment structure — dump audio segment sizes, audio_size,
/// and data_size so we can spot any mismatch or off-by-one in the decoder.
use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    path::PathBuf,
};

fn read_u8(r: &mut impl Read) -> u8 {
    let mut b = [0u8; 1];
    r.read_exact(&mut b).unwrap();
    b[0]
}
fn read_u16(r: &mut (impl Read + Seek)) -> u16 {
    let mut b = [0u8; 2];
    r.read_exact(&mut b).unwrap();
    u16::from_le_bytes(b)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: analyze_mve <input.mve>");
        std::process::exit(1);
    }

    let path = PathBuf::from(&args[1]);
    let f = File::open(&path).expect("open failed");
    let mut r = BufReader::new(f);

    // Skip 26-byte signature
    r.seek(SeekFrom::Start(26)).unwrap();

    let mut chunk_idx = 0usize;
    let mut total_data_size_consumed = 0i64;

    loop {
        let chunk_start = r.stream_position().unwrap();
        let chunk_size_result = {
            let mut b = [0u8; 2];
            match r.read_exact(&mut b) {
                Ok(_) => u16::from_le_bytes(b),
                Err(_) => break,
            }
        };
        let chunk_size = chunk_size_result;
        let _chunk_type = read_u16(&mut r);

        let mut offset = 0u16;
        while offset < chunk_size {
            let seg_size = read_u16(&mut r);
            let seg_type = read_u8(&mut r);
            let seg_ver = read_u8(&mut r);

            match seg_type {
                0x08 | 0x09 => {
                    // AUDIO_DATA or AUDIO_SILENCE
                    let seq = read_u16(&mut r);
                    let stream_mask = read_u16(&mut r);
                    let audio_size = read_u16(&mut r);
                    let data_size = seg_size.saturating_sub(6);
                    let remaining = data_size as usize;

                    let kind = if seg_type == 0x08 { "AUDIO_DATA  " } else { "AUDIO_SILENCE" };
                    println!(
                        "chunk={chunk_idx:3} {kind} seg_size={seg_size:5} audio_size={audio_size:5} data_size={data_size:5} \
                         seq={seq:4} mask={stream_mask:#06x}"
                    );
                    total_data_size_consumed += remaining as i64;

                    // Skip the audio data
                    r.seek(SeekFrom::Current(remaining as i64)).unwrap();
                }
                _ => {
                    // Skip everything else
                    r.seek(SeekFrom::Current(seg_size as i64)).unwrap();
                }
            }

            offset = offset.saturating_add(4 + seg_size);
        }

        chunk_idx += 1;
        let _ = chunk_start;
    }

    println!("Total audio data bytes consumed: {total_data_size_consumed}");
}
