//! Renders one original music module to a 16-bit stereo WAV for listening or comparison.
//! `cargo run -p gl-audio --example render -- <uplink dir> music/serenity.uni out.wav [rate] [seconds]`

use gl_assets::Store;
use gl_audio::{ModPlayer, Module};
use std::io::Write;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let rate: u32 = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(44100);
    let secs: f64 = a
        .get(5)
        .and_then(|s| s.parse().ok())
        .unwrap_or(f64::INFINITY);
    let mut store = Store::from_dir(&a[1]).expect("store");
    let bytes = store.read("music.dat", &a[2]).expect("module");
    let mut p = ModPlayer::new(Module::load(&bytes).expect("load"), rate);
    p.set_volume(85);
    let mut pcm: Vec<i16> = Vec::new();
    let chunk: usize = std::env::var("GLCHUNK")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1024);
    let mut buf = vec![0f32; chunk * 2];
    while !p.finished() && (pcm.len() as f64) < secs * rate as f64 * 2.0 {
        p.render(&mut buf);
        pcm.extend(
            buf.iter()
                .map(|&x| (x * 32768.0).clamp(-32768.0, 32767.0) as i16),
        );
    }
    let mut f = std::fs::File::create(&a[3]).expect("create");
    let data = (pcm.len() * 2) as u32;
    let mut h = Vec::new();
    h.extend(b"RIFF");
    h.extend((36 + data).to_le_bytes());
    h.extend(b"WAVEfmt ");
    h.extend(16u32.to_le_bytes());
    h.extend(1u16.to_le_bytes());
    h.extend(2u16.to_le_bytes());
    h.extend(rate.to_le_bytes());
    h.extend((rate * 4).to_le_bytes());
    h.extend(4u16.to_le_bytes());
    h.extend(16u16.to_le_bytes());
    h.extend(b"data");
    h.extend(data.to_le_bytes());
    f.write_all(&h).unwrap();
    for s in pcm {
        f.write_all(&s.to_le_bytes()).unwrap();
    }
    eprintln!("{}: {:.1}s", a[2], data as f64 / 4.0 / rate as f64);
}
