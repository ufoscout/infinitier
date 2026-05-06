use infinitier_acm_encoder::{encode_pcm, encode_pcm_packed};
fn main() {
    // 1 second of 22050Hz mono speech-like signal: low-amplitude noise
    // around 4000 Hz with silent gaps between phrases.
    let mut pcm: Vec<i16> = Vec::new();
    for i in 0..22050 {
        let phase = i % 4410;            // 0..4410 = 200 ms
        let amp = if phase < 3600 { 6000 } else { 0 };  // silence in tail
        let s = ((i as f32 * 0.4).sin() * amp as f32) as i16;
        pcm.push(s);
    }
    let mut v1 = Vec::new(); encode_pcm(&pcm, 1, 22050, &mut v1).unwrap();
    let mut p  = Vec::new(); encode_pcm_packed(&pcm, 1, 22050, &mut p).unwrap();
    println!("v1: {} bytes  packer: {} bytes  ratio: {:.2}",
             v1.len(), p.len(), p.len() as f64 / v1.len() as f64);
}
