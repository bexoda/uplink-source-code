//! Runs against the real files from the Uplink developer CD that sits in this repository
//! (or any install pointed to by UPLINK_DIR). Skipped loudly when neither exists.

use std::path::PathBuf;

use ur_assets::*;

fn dir() -> Option<PathBuf> {
    let d = std::env::var_os("UPLINK_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../uplink/bin"));
    if d.join("graphics.dat").is_file() {
        Some(d)
    } else {
        eprintln!("SKIPPED: no Uplink data found (set UPLINK_DIR)");
        None
    }
}

#[test]
fn every_sprite_in_graphics_decodes() {
    let Some(d) = dir() else { return };
    let mut s = Store::from_dir(&d).unwrap();
    let names: Vec<String> = s
        .archive("graphics.dat")
        .unwrap()
        .names()
        .iter()
        .filter(|n| n.ends_with(".tif"))
        .map(|n| n.to_string())
        .collect();
    assert!(names.len() > 200, "found {}", names.len());
    let mut failures = vec![];
    for n in &names {
        let rel = n.strip_prefix("graphics/").unwrap();
        match s.image(rel) {
            Ok(img) => assert_eq!(
                img.pixels.len(),
                (img.width * img.height * 4) as usize,
                "{n}"
            ),
            Err(e) => failures.push(format!("{n}: {e}")),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} failed:\n{}",
        failures.len(),
        names.len(),
        failures.join("\n")
    );
}

#[test]
fn world_map_and_mask_have_matching_geometry() {
    let Some(d) = dir() else { return };
    let mut s = Store::from_dir(&d).unwrap();
    let map = s.image("worldmaplarge.tif").unwrap();
    let mask = s.image("worldmaplarge_mask.tif").unwrap();
    assert_eq!((map.width, map.height), (mask.width, mask.height));
    let land = mask.pixels.chunks_exact(4).filter(|p| p[0] > 127).count();
    let total = (mask.width * mask.height) as usize;
    assert!(
        land > total / 10 && land < total / 2,
        "land fraction looks wrong: {land}/{total}"
    );
}

#[test]
fn patches_override_the_base_graphics() {
    let Some(d) = dir() else { return };
    let mut s = Store::from_dir(&d).unwrap();
    // patch3 ships a replacement world map; the lookup must prefer it over graphics.dat.
    let in_base = s
        .archive("graphics.dat")
        .unwrap()
        .contains("graphics/worldmaplarge_defcon.tif");
    assert!(!in_base);
    assert!(s.image("worldmaplarge_defcon.tif").is_ok());
}

#[test]
fn fonts_data_sounds_and_wrapped_files() {
    let Some(d) = dir() else { return };
    let mut s = Store::from_dir(&d).unwrap();
    for f in ["BATTLE3.TTF", "dungeon.TTF", "battle3.ttf"] {
        let ttf = s.font(f).unwrap();
        assert!(
            ttf.len() > 1000 && (ttf[..4] == [0, 1, 0, 0] || &ttf[..4] == b"true"),
            "{f} is not a TrueType font"
        );
    }
    let wav = s.read("sounds.dat", "sounds/1.wav").unwrap();
    assert_eq!(&wav[..4], b"RIFF");
    // data.dat is a REDSHIRT-wrapped archive of text tables.
    let txt =
        String::from_utf8_lossy(&s.read("data.dat", "data/gatewaydefs.txt").unwrap()).to_string();
    assert!(!txt.is_empty());
    // world.dat is wrapped but not an archive: it is an MP3.
    let mp3 = s.unwrapped("world.dat").unwrap();
    assert_eq!(&mp3[..2], &[0xFF, 0xFB]);
}

#[test]
fn missing_things_are_errors_not_panics() {
    let Some(d) = dir() else { return };
    let mut s = Store::from_dir(&d).unwrap();
    assert!(matches!(s.image("nope.tif"), Err(AssetError::NotFound(_))));
    assert!(matches!(
        s.read("graphics.dat", "../../etc/passwd"),
        Err(AssetError::NotFound(_))
    ));
    assert!(Store::from_dir("/definitely/not/here").is_err());
    let mut mem = Store::from_memory(Default::default());
    assert!(matches!(
        mem.image("hud/status.tif"),
        Err(AssetError::NotFound(_))
    ));
}

#[test]
fn memory_store_works_like_the_browser_file_picker() {
    let Some(d) = dir() else { return };
    let mut files = std::collections::HashMap::new();
    for f in ["graphics.dat", "fonts.dat"] {
        files.insert(f.to_string(), std::fs::read(d.join(f)).unwrap());
    }
    let mut s = Store::from_memory(files);
    assert!(s.image("hud/status.tif").is_ok());
    assert!(s.font("dungeon.TTF").is_ok());
}

#[test]
fn hostile_inputs_do_not_panic() {
    assert!(decode_tiff(b"II*\0garbage").is_err());
    assert!(decode_tiff(&[]).is_err());
    let _ = decode_wrapper(b"REDSHRT2\0");
    assert!(Archive::from_bytes(b"PK\x03\x04truncated").is_err());
}
