//! Lists the WAV effects in sounds.dat with their formats.
fn main() {
    let dir = std::env::args().nth(1).expect("dir");
    let mut store = ur_assets::Store::from_dir(&dir).expect("store");
    let names: Vec<String> = store
        .archive("sounds.dat")
        .expect("sounds.dat")
        .names()
        .iter()
        .map(|s| s.to_string())
        .collect();
    for n in names {
        let b = store.read("sounds.dat", &n).expect("read");
        let fmt = b
            .windows(4)
            .position(|w| w == b"fmt ")
            .map(|i| &b[i + 8..i + 24]);
        if let Some(f) = fmt {
            let tag = u16::from_le_bytes([f[0], f[1]]);
            let ch = u16::from_le_bytes([f[2], f[3]]);
            let rate = u32::from_le_bytes([f[4], f[5], f[6], f[7]]);
            let bits = u16::from_le_bytes([f[14], f[15]]);
            println!(
                "{n}: tag={tag} ch={ch} rate={rate} bits={bits} bytes={}",
                b.len()
            );
        } else {
            println!("{n}: no fmt, {} bytes", b.len());
        }
    }
}
