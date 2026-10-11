//! Writes the raw music modules out of music.dat (for reference tools; never commit them).
//! `cargo run -p gl-audio --example extract -- <uplink dir> <out dir>`

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut store = gl_assets::Store::from_dir(&a[1]).expect("store");
    let names: Vec<String> = store
        .archive("music.dat")
        .expect("music.dat")
        .names()
        .iter()
        .map(|s| s.to_string())
        .collect();
    for n in names {
        let b = store.read("music.dat", &n).expect("read");
        let out = std::path::Path::new(&a[2]).join(n.rsplit('/').next().unwrap());
        std::fs::write(&out, b).expect("write");
        println!("{}", out.display());
    }
}
