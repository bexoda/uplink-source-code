//! Port of libmikmod (c) 1998-2004 Miodrag Vallat and others, LGPL-2.0-or-later; this file keeps that licence.
//!
//! Loader for MikMod's UniMod format, versions UN04 and UN05 (what Uplink's music.dat holds).
//!
//! Follows libmikmod's `load_uni.c` (`loadinstr5`/`loadsmp5`) and `mloader.c` defaults, so the
//! player sees the same module structure libmikmod would build.

use crate::AudioError;

pub const UF_XMPERIODS: u16 = 0x0001;
pub const UF_LINEAR: u16 = 0x0002;
pub const UF_INST: u16 = 0x0004;
pub const UF_NOWRAP: u16 = 0x0080;
pub const UF_PANNING: u16 = 0x0400;

pub const SF_16BITS: u16 = 0x0001;
pub const SF_SIGNED: u16 = 0x0004;
pub const SF_BIG_ENDIAN: u16 = 0x0008;
pub const SF_DELTA: u16 = 0x0010;
pub const SF_LOOP: u16 = 0x0100;
pub const SF_BIDI: u16 = 0x0200;
pub const SF_REVERSE: u16 = 0x0400;
pub const SF_SUSTAIN: u16 = 0x0800;

pub const EF_ON: u8 = 1;
pub const EF_SUSTAIN: u8 = 2;
pub const EF_LOOP: u8 = 4;

/// End-of-song marker in the order list.
pub const LAST_PATTERN: u16 = u16::MAX;
pub const INSTNOTES: usize = 120;

/// Number of operand bytes per UniMod opcode (libmikmod `unioperands`).
pub const OPERANDS: [u8; 62] = [
    0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 2, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 1, 2, 2, 0, 0, 0, 2,
];

pub mod op {
    pub const NOTE: u8 = 1;
    pub const INSTRUMENT: u8 = 2;
    pub const PTEFFECT0: u8 = 3;
    pub const PTEFFECT1: u8 = 4;
    pub const PTEFFECT2: u8 = 5;
    pub const PTEFFECT3: u8 = 6;
    pub const PTEFFECT4: u8 = 7;
    pub const PTEFFECT5: u8 = 8;
    pub const PTEFFECT6: u8 = 9;
    pub const PTEFFECT7: u8 = 10;
    pub const PTEFFECT8: u8 = 11;
    pub const PTEFFECT9: u8 = 12;
    pub const PTEFFECTA: u8 = 13;
    pub const PTEFFECTB: u8 = 14;
    pub const PTEFFECTC: u8 = 15;
    pub const PTEFFECTD: u8 = 16;
    pub const PTEFFECTE: u8 = 17;
    pub const PTEFFECTF: u8 = 18;
    pub const S3MEFFECTA: u8 = 19;
    pub const S3MEFFECTD: u8 = 20;
    pub const S3MEFFECTE: u8 = 21;
    pub const S3MEFFECTF: u8 = 22;
    pub const S3MEFFECTI: u8 = 23;
    pub const S3MEFFECTQ: u8 = 24;
    pub const S3MEFFECTR: u8 = 25;
    pub const S3MEFFECTT: u8 = 26;
    pub const S3MEFFECTU: u8 = 27;
    pub const KEYOFF: u8 = 28;
    pub const KEYFADE: u8 = 29;
    pub const VOLEFFECTS: u8 = 30;
    pub const XMEFFECT4: u8 = 31;
    pub const XMEFFECT6: u8 = 32;
    pub const XMEFFECTA: u8 = 33;
    pub const XMEFFECTE1: u8 = 34;
    pub const XMEFFECTE2: u8 = 35;
    pub const XMEFFECTEA: u8 = 36;
    pub const XMEFFECTEB: u8 = 37;
    pub const XMEFFECTG: u8 = 38;
    pub const XMEFFECTH: u8 = 39;
    pub const XMEFFECTL: u8 = 40;
    pub const XMEFFECTP: u8 = 41;
    pub const XMEFFECTX1: u8 = 42;
    pub const XMEFFECTX2: u8 = 43;
    pub const LAST: u8 = 62;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EnvPt {
    pub pos: i16,
    pub val: i16,
}

#[derive(Debug, Clone, Default)]
pub struct Envelope {
    pub flg: u8,
    pub pts: u8,
    pub susbeg: u8,
    pub susend: u8,
    pub beg: u8,
    pub end: u8,
    pub env: Vec<EnvPt>,
}

#[derive(Debug, Clone)]
pub struct Instrument {
    pub name: String,
    pub samplenumber: [u16; INSTNOTES],
    pub samplenote: [u8; INSTNOTES],
    pub globvol: u8,
    pub volfade: u16,
    pub flags: u8,
    pub panning: u16,
    pub vol: Envelope,
    pub pan: Envelope,
    pub pit: Envelope,
}

#[derive(Debug, Clone)]
pub struct Sample {
    pub name: String,
    /// c2spd / finetune-coded speed.
    pub speed: u32,
    pub volume: u8,
    pub panning: u16,
    pub flags: u16,
    pub length: u32,
    pub loopstart: u32,
    pub loopend: u32,
    pub vibtype: u8,
    pub vibsweep: u8,
    pub vibdepth: u8,
    pub vibrate: u8,
    pub globvol: u8,
    /// Signed 16-bit PCM, `length` frames.
    pub data: Vec<i16>,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub flags: u16,
    pub numchn: u8,
    pub positions: Vec<u16>,
    pub reppos: u16,
    pub initspeed: u8,
    pub inittempo: u8,
    pub initvolume: u8,
    pub bpmlimit: u16,
    pub panning: Vec<u16>,
    pub chanvol: Vec<u8>,
    pub instruments: Vec<Instrument>,
    pub samples: Vec<Sample>,
    pub pattrows: Vec<u16>,
    /// `numpat * numchn` track indices.
    pub patterns: Vec<u16>,
    pub tracks: Vec<Vec<u8>>,
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn bytes(&mut self, n: usize) -> Result<&[u8], AudioError> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.b.len());
        let end = end.ok_or(AudioError::Truncated)?;
        let s = &self.b[self.at..end];
        self.at = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, AudioError> {
        Ok(self.bytes(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, AudioError> {
        let b = self.bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn i16(&mut self) -> Result<i16, AudioError> {
        Ok(self.u16()? as i16)
    }
    fn u32(&mut self) -> Result<u32, AudioError> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn string(&mut self) -> Result<String, AudioError> {
        let n = self.u16()? as usize;
        Ok(String::from_utf8_lossy(self.bytes(n)?)
            .trim_end_matches('\0')
            .to_string())
    }
}

struct Smp5 {
    c2spd: u16,
    transpose: i8,
    volume: u8,
    panning: u8,
    length: u32,
    loopstart: u32,
    loopend: u32,
    flags: u16,
    name: String,
    vib: [u8; 4],
}

fn envelope5(r: &mut Reader) -> Result<Envelope, AudioError> {
    let flg = r.u8()?;
    let pts = r.u8()?;
    let susbeg = r.u8()?;
    let beg = r.u8()?;
    let end = r.u8()?;
    let mut env = Vec::with_capacity(12);
    for _ in 0..12 {
        env.push(EnvPt {
            pos: r.i16()?,
            val: r.i16()?,
        });
    }
    Ok(Envelope {
        flg,
        pts,
        susbeg,
        susend: susbeg,
        beg,
        end,
        env,
    })
}

/// Validates a track the way `readtrack()` does, so the player can index it without checks.
fn check_track(t: &[u8]) -> bool {
    let mut cur = 0;
    loop {
        let Some(&c) = t.get(cur) else { return false };
        cur += 1;
        if c == 0 {
            return true;
        }
        let mut chunk = (c & 0x1f) as i32 - 1;
        while chunk > 0 {
            let Some(&opcode) = t.get(cur) else {
                return false;
            };
            if opcode == 0 || opcode >= op::LAST {
                return false;
            }
            let oplen = OPERANDS[opcode as usize] as i32 + 1;
            cur += oplen as usize;
            chunk -= oplen;
        }
        if chunk < 0 || cur >= t.len() {
            return false;
        }
    }
}

impl Module {
    pub fn load(bytes: &[u8]) -> Result<Module, AudioError> {
        let mut r = Reader { b: bytes, at: 0 };
        let id = r.bytes(4)?;
        if &id[..3] != b"UN0" || !(b'4'..=b'5').contains(&id[3]) {
            return Err(AudioError::Unsupported("not a UN04/UN05 module".into()));
        }
        let version = id[3] - b'0';
        let numchn = r.u8()?;
        let numpos = r.u16()?;
        let reppos = if version == 5 { r.u16()? } else { 0 };
        let numpat = r.u16()?;
        let numtrk = r.u16()?;
        let numins = r.u16()?;
        let initspeed = r.u8()?;
        let inittempo = r.u8()?;
        let positions_raw = r.bytes(256)?.to_vec();
        let panning_raw = r.bytes(32)?.to_vec();
        let mut flags = r.u8()? as u16;
        flags &= UF_XMPERIODS | UF_LINEAR;
        flags |= UF_INST | UF_NOWRAP | UF_PANNING;

        let name = r.string()?;
        let _oldtype = r.string()?;
        let _comment = r.string()?;

        if numpos > 256 || numchn > 32 || numchn == 0 || numins == 0 {
            return Err(AudioError::Unsupported("bad header".into()));
        }
        let positions = positions_raw[..numpos as usize]
            .iter()
            .map(|&p| if p == 255 { LAST_PATTERN } else { p as u16 })
            .collect();
        let panning = panning_raw[..numchn as usize]
            .iter()
            .map(|&p| p as u16)
            .collect();

        // loadinstr5
        let mut instruments = Vec::with_capacity(numins as usize);
        let mut smp5: Vec<Smp5> = Vec::new();
        for _ in 0..numins {
            let numsmp = r.u8()?;
            let mut samplenumber = [0xffffu16; INSTNOTES];
            let base = smp5.len() as u16;
            for s in samplenumber.iter_mut().take(96) {
                *s = base + r.u8()? as u16;
            }
            let vol = envelope5(&mut r)?;
            let pan = envelope5(&mut r)?;
            let vib = [r.u8()?, r.u8()?, r.u8()?, r.u8()?];
            let volfade = r.u16()?;
            let iname = r.string()?;
            for _ in 0..numsmp {
                smp5.push(Smp5 {
                    c2spd: r.u16()?,
                    transpose: r.u8()? as i8,
                    volume: r.u8()?,
                    panning: r.u8()?,
                    length: r.u32()?,
                    loopstart: r.u32()?,
                    loopend: r.u32()?,
                    flags: r.u16()?,
                    name: r.string()?,
                    vib,
                });
            }
            instruments.push(Instrument {
                name: iname,
                samplenumber,
                samplenote: [0; INSTNOTES],
                globvol: 64,
                volfade,
                flags: 0,
                panning: 0,
                vol,
                pan,
                pit: Envelope::default(),
            });
        }
        if smp5.is_empty() {
            return Err(AudioError::Unsupported("no samples".into()));
        }

        // loadsmp5
        let numsmp = smp5.len();
        let mut samples: Vec<Sample> = smp5
            .iter()
            .map(|s| {
                let mut f = 0;
                for (bit, sf) in [
                    (128, SF_REVERSE),
                    (64, SF_SUSTAIN),
                    (32, SF_BIDI),
                    (16, SF_LOOP),
                    (8, SF_BIG_ENDIAN),
                    (4, SF_DELTA),
                    (2, SF_SIGNED),
                    (1, SF_16BITS),
                ] {
                    if s.flags & bit != 0 {
                        f |= sf;
                    }
                }
                Sample {
                    name: s.name.clone(),
                    speed: s.c2spd as u32,
                    volume: s.volume,
                    panning: s.panning as u16,
                    flags: f,
                    length: s.length,
                    loopstart: s.loopstart,
                    loopend: s.loopend,
                    vibtype: s.vib[0],
                    vibsweep: s.vib[1],
                    vibdepth: s.vib[2],
                    vibrate: s.vib[3],
                    globvol: 64,
                    data: Vec::new(),
                }
            })
            .collect();
        for ins in instruments.iter_mut() {
            for t in 0..INSTNOTES {
                let n = ins.samplenumber[t] as usize;
                ins.samplenote[t] = if n >= numsmp {
                    255
                } else {
                    (t as i32 + smp5[n].transpose as i32) as u8
                };
            }
        }
        // Did the original file have no instruments?
        if numsmp == instruments.len() {
            let plain = instruments.iter().enumerate().all(|(t, d)| {
                d.vol.pts == 0
                    && d.pan.pts == 0
                    && d.globvol == 64
                    && (0..96)
                        .all(|u| d.samplenumber[u] as usize == t && d.samplenote[u] as usize == u)
            });
            if plain {
                flags &= !(UF_INST | UF_NOWRAP);
                for (s, d) in samples.iter_mut().zip(&instruments) {
                    s.name = d.name.clone();
                }
            }
        }

        // patterns
        let mut pattrows = Vec::with_capacity(numpat as usize);
        for _ in 0..numpat {
            pattrows.push(r.u16()?);
        }
        let mut patterns = Vec::with_capacity(numpat as usize * numchn as usize);
        for _ in 0..numpat as usize * numchn as usize {
            patterns.push(r.u16()?);
        }
        // tracks
        let mut tracks = Vec::with_capacity(numtrk as usize);
        for _ in 0..numtrk {
            let len = r.u16()? as usize;
            let t = r.bytes(len)?.to_vec();
            if len == 0 || !check_track(&t) {
                return Err(AudioError::Unsupported("bad track".into()));
            }
            tracks.push(t);
        }
        if patterns.iter().any(|&p| p as usize >= tracks.len()) {
            return Err(AudioError::Unsupported(
                "pattern references missing track".into(),
            ));
        }

        // sample data, stored in order after the tracks
        for s in samples.iter_mut() {
            if s.length == 0 {
                continue;
            }
            let n = s.length as usize;
            let mut data = Vec::with_capacity(n);
            if s.flags & SF_16BITS != 0 {
                let raw = r.bytes(n * 2)?;
                for c in raw.chunks_exact(2) {
                    data.push(if s.flags & SF_BIG_ENDIAN != 0 {
                        i16::from_be_bytes([c[0], c[1]])
                    } else {
                        i16::from_le_bytes([c[0], c[1]])
                    });
                }
            } else {
                data.extend(r.bytes(n)?.iter().map(|&b| (b as i8 as i16) << 8));
            }
            if s.flags & SF_DELTA != 0 {
                let mut old = 0i16;
                for v in data.iter_mut() {
                    *v = v.wrapping_add(old);
                    old = *v;
                }
            }
            if s.flags & SF_SIGNED == 0 {
                for v in data.iter_mut() {
                    *v = (*v as u16 ^ 0x8000) as i16;
                }
            }
            // VC1_SampleLoad: loop sanity, then pad 16 frames past the loop end (or the sample
            // end) so the interpolating mixer can read one frame ahead without a branch.
            if s.loopend > s.length {
                s.loopend = s.length;
            }
            if s.loopstart >= s.loopend {
                s.flags &= !SF_LOOP;
            }
            data.resize(n + 20, 0);
            if s.flags & SF_LOOP != 0 {
                let (ls, le) = (s.loopstart as usize, s.loopend as usize);
                let looplen = le - ls;
                for t in 0..16.min(looplen) {
                    data[le + t] = if s.flags & SF_BIDI != 0 {
                        data[le - t - 1]
                    } else {
                        data[t + ls]
                    };
                }
            }
            s.data = data;
        }

        Ok(Module {
            name,
            flags,
            numchn,
            positions,
            reppos,
            initspeed,
            inittempo,
            initvolume: 128,
            bpmlimit: 32,
            panning,
            chanvol: vec![64; numchn as usize],
            instruments,
            samples,
            pattrows,
            patterns,
            tracks,
        })
    }
}
