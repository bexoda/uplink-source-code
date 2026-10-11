//! Lists the original music modules and which UniMod opcodes they use.
//! `cargo run -p ur-audio --example inspect -- <uplink dir>`

use ur_assets::Store;
use ur_audio::uni::{Module, OPERANDS};

fn main() {
    let dir = std::env::args()
        .nth(1)
        .expect("usage: inspect <dir with music.dat>");
    let mut store = Store::from_dir(&dir).expect("store");
    let names: Vec<String> = store
        .archive("music.dat")
        .expect("music.dat")
        .names()
        .iter()
        .map(|s| s.to_string())
        .collect();
    for name in names {
        let bytes = store.read("music.dat", &name).expect("read");
        let m = match Module::load(&bytes) {
            Ok(m) => m,
            Err(e) => {
                println!("{name}: {e}");
                continue;
            }
        };
        let mut hist = [0u32; 62];
        let mut ops_e = std::collections::BTreeSet::new();
        let mut vol_fx = std::collections::BTreeSet::new();
        for t in &m.tracks {
            let mut cur = 0;
            while t[cur] != 0 {
                let len = (t[cur] & 0x1f) as usize;
                let mut i = cur + 1;
                while i < cur + len {
                    let o = t[i] as usize;
                    hist[o] += 1;
                    if o == 17 {
                        ops_e.insert(t[i + 1] >> 4);
                    }
                    if o == 30 {
                        vol_fx.insert(t[i + 1]);
                    }
                    i += 1 + OPERANDS[o] as usize;
                }
                cur += len;
            }
        }
        let used: Vec<String> = hist
            .iter()
            .enumerate()
            .filter(|(_, &n)| n > 0)
            .map(|(o, n)| format!("{o}x{n}"))
            .collect();
        let sf: std::collections::BTreeSet<u16> = m.samples.iter().map(|s| s.flags).collect();
        let envs = m
            .instruments
            .iter()
            .filter(|i| i.vol.flg & 1 != 0 || i.pan.flg & 1 != 0)
            .count();
        let vib = m.samples.iter().filter(|s| s.vibdepth != 0).count();
        println!(
            "{name}: '{}' chn={} pos={} pat={} trk={} ins={} smp={} flags={:#x} speed={} tempo={} rep={}\n  ops {}\n  E-sub {:?} volfx {:?} sflags {:x?} envs={} autovib={} pan={:?}",
            m.name,
            m.numchn,
            m.positions.len(),
            m.pattrows.len(),
            m.tracks.len(),
            m.instruments.len(),
            m.samples.len(),
            m.flags,
            m.initspeed,
            m.inittempo,
            m.reppos,
            used.join(" "),
            ops_e,
            vol_fx,
            sf,
            envs,
            vib,
            m.panning
        );
    }
}
