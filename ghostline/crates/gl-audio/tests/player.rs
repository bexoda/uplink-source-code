//! Player behaviour on a synthetic module (always runs) and on the original music (when the
//! Uplink data is present; durations were measured with libmikmod 3.3.11 using the same settings).

use gl_audio::{ModPlayer, Module, Sound};
use std::path::PathBuf;

/// One channel, one looping square-wave sample, a 4-row pattern: a note on row 0 plus `fx`.
fn module(fx: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    let s = |v: &mut Vec<u8>, t: &str| {
        v.extend((t.len() as u16).to_le_bytes());
        v.extend(t.as_bytes());
    };
    v.extend(b"UN05");
    v.push(1); // numchn
    v.extend(1u16.to_le_bytes()); // numpos
    v.extend(0u16.to_le_bytes()); // reppos
    v.extend(1u16.to_le_bytes()); // numpat
    v.extend(1u16.to_le_bytes()); // numtrk
    v.extend(1u16.to_le_bytes()); // numins
    v.push(6); // speed
    v.push(125); // tempo
    v.extend([0u8; 256]);
    v.extend([128u8; 32]);
    v.push(0); // flags: Amiga periods
    s(&mut v, "test");
    s(&mut v, "");
    s(&mut v, "");
    // instrument
    v.push(1);
    v.extend([0u8; 96]);
    for _ in 0..2 {
        v.extend([0u8; 5]);
        v.extend([0u8; 48]);
    }
    v.extend([0u8; 4]);
    v.extend(0u16.to_le_bytes());
    s(&mut v, "");
    // sample: 8363 Hz, volume 64, 32 frames, looped, signed 8-bit
    v.extend(8363u16.to_le_bytes());
    v.push(0);
    v.push(64);
    v.push(128);
    v.extend(32u32.to_le_bytes());
    v.extend(0u32.to_le_bytes());
    v.extend(32u32.to_le_bytes());
    v.extend((16u16 | 2).to_le_bytes());
    s(&mut v, "");
    // pattern
    v.extend(4u16.to_le_bytes());
    v.extend(0u16.to_le_bytes());
    // track: row 0 = note 48 + instrument 0 + fx, rows 1..3 empty
    let mut row0 = vec![1, 48, 2, 0];
    row0.extend(fx);
    let mut t = vec![(row0.len() + 1) as u8];
    t.extend(row0);
    t.push((2 << 5) | 1);
    t.push(0);
    v.extend((t.len() as u16).to_le_bytes());
    v.extend(t);
    // sample data: square wave with a 16-frame period
    for i in 0..32 {
        v.push(if (i / 8) % 2 == 0 {
            100
        } else {
            (-100i8) as u8
        });
    }
    v
}

fn render_all(p: &mut ModPlayer) -> Vec<f32> {
    let mut all = Vec::new();
    let mut buf = vec![0f32; 512];
    for _ in 0..100_000 {
        if p.finished() {
            break;
        }
        p.render(&mut buf);
        all.extend_from_slice(&buf);
    }
    all
}

#[test]
fn plays_a_note_at_the_amiga_pitch_for_the_pattern_length() {
    let rate = 44100;
    let mut p = ModPlayer::new(Module::load(&module(&[])).unwrap(), rate);
    p.set_volume(128);
    let out = render_all(&mut p);
    // 4 rows x 6 ticks, 125 BPM = 50 ticks/s; rendering stops at the next 256-frame buffer.
    let frames = out.len() / 2;
    let expect = 24 * (rate as usize * 125 / (125 * 50));
    assert!(
        frames >= expect && frames < expect + 256 + 882,
        "{frames} vs {expect}"
    );
    // C-4 on an 8363 Hz sample plays it at 8363 Hz: the 16-frame square is 522.7 Hz.
    let left: Vec<f32> = out.chunks(2).take(expect).map(|f| f[0]).collect();
    let rises = left
        .windows(2)
        .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
        .count();
    let hz = rises as f64 / (expect as f64 / rate as f64);
    assert!((hz - 8363.0 / 16.0).abs() < 3.0, "{hz}");
    // Centre pan (128): libmikmod weights left 127/256 and right 128/256.
    assert!(out
        .chunks(2)
        .take(expect)
        .all(|f| (f[0] - f[1]).abs() <= f[1].abs() / 64.0 + 1e-4));
    assert!(left.iter().any(|&x| x.abs() > 0.05));
}

#[test]
fn set_volume_effect_silences_the_note() {
    // UNI_PTEFFECTC (15) with 0
    let mut p = ModPlayer::new(Module::load(&module(&[15, 0])).unwrap(), 22050);
    p.set_volume(128);
    let out = render_all(&mut p);
    assert!(!out.is_empty());
    assert!(out.iter().all(|&x| x == 0.0));
}

#[test]
fn pattern_break_ends_the_song_early() {
    // UNI_PTEFFECTD (16) on row 0 of the only order: the song ends after one row.
    let rate = 22050;
    let mut p = ModPlayer::new(Module::load(&module(&[16, 0])).unwrap(), rate);
    let frames = render_all(&mut p).len() / 2;
    assert!(frames < 2 * 6 * (rate as usize / 50) + 256, "{frames}");
}

#[test]
fn rejects_corrupt_modules() {
    let good = module(&[]);
    assert!(Module::load(&good[..100]).is_err());
    assert!(Module::load(b"UN06xxxxxxxx").is_err());
    let mut bad = good.clone();
    // Corrupt the track's first opcode into an invalid one.
    let at = bad.len() - 32 - 7;
    bad[at] = 200;
    assert!(Module::load(&bad).is_err());
}

fn dir() -> Option<PathBuf> {
    let d = std::env::var_os("UPLINK_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../uplink/bin"));
    if d.join("music.dat").is_file() {
        Some(d)
    } else {
        eprintln!("SKIPPED: no Uplink data found (set UPLINK_DIR)");
        None
    }
}

#[test]
fn original_songs_play_for_their_libmikmod_length() {
    let Some(d) = dir() else { return };
    let mut store = gl_assets::Store::from_dir(&d).unwrap();
    // Seconds at 44.1 kHz from libmikmod's own render of each module.
    for (name, secs) in [
        ("a94final", 189.8),
        ("bluevalley", 720.9),
        ("myst2", 445.7),
        ("mystique", 217.2),
        ("serenity", 164.6),
        ("symphonic", 250.6),
    ] {
        let bytes = store
            .read("music.dat", &format!("music/{name}.uni"))
            .unwrap();
        let rate = 8000;
        let mut p = ModPlayer::new(Module::load(&bytes).unwrap(), rate);
        let mut buf = vec![0f32; 2 * 4000];
        let mut frames = 0usize;
        let mut peak = 0f32;
        while !p.finished() && frames < rate as usize * 1000 {
            p.render(&mut buf);
            frames += 4000;
            peak = buf.iter().fold(peak, |m, &x| m.max(x.abs()));
        }
        let got = frames as f64 / rate as f64;
        assert!(
            (got - secs).abs() < 1.0,
            "{name}: {got}s, libmikmod {secs}s"
        );
        assert!(peak > 0.1, "{name}: peak {peak}");
    }
}

#[test]
fn every_original_sound_effect_decodes() {
    let Some(d) = dir() else { return };
    let mut store = gl_assets::Store::from_dir(&d).unwrap();
    let names: Vec<String> = store
        .archive("sounds.dat")
        .unwrap()
        .names()
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert!(names.len() >= 48);
    for n in names.iter().filter(|n| n.ends_with(".wav")) {
        let s = Sound::decode(&store.read("sounds.dat", n).unwrap()).unwrap();
        assert!(!s.samples.is_empty() && s.rate >= 11025, "{n}");
    }
}
