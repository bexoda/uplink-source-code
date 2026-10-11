//! Port of libmikmod (c) 1998-2004 Miodrag Vallat and others, LGPL-2.0-or-later; this file keeps that licence.
//!
//! UniMod replayer, ported from libmikmod's `mplayer.c` for the module features Uplink's music
//! uses: plain samples (no instrument envelopes, no NNA), Amiga periods with c2spd, Protracker
//! effects and the Scream Tracker ones `mikcvt` emits. Unknown opcodes are skipped.
//!
//! Playback settings match how SDL_mixer drove libmikmod for the original game: `loop = 0`,
//! `wrap = 0` (a song ends; the playlist moves on), `extspd = 1`, `panflag = 1`, `md_volume = 96`,
//! `md_pansep = 128`, no reverb.
//!
//! Integer types follow the C fields (`UWORD` periods wrap, `SBYTE` LFO positions wrap) because
//! the effects rely on that arithmetic.

use crate::uni::{self, op, Module, LAST_PATTERN, OPERANDS, SF_BIDI, SF_LOOP, SF_REVERSE};

const OCTAVE: u16 = 12;

const OLDPERIODS: [u16; 24] = [
    0x6b00, 0x6800, 0x6500, 0x6220, 0x5f50, 0x5c80, 0x5a00, 0x5740, 0x54d0, 0x5260, 0x5010, 0x4dc0,
    0x4b90, 0x4960, 0x4750, 0x4540, 0x4350, 0x4160, 0x3f90, 0x3dc0, 0x3c10, 0x3a40, 0x38b0, 0x3700,
];

const VIBRATO_TABLE: [u8; 32] = [
    0, 24, 49, 74, 97, 120, 141, 161, 180, 197, 212, 224, 235, 244, 250, 253, 255, 253, 250, 244,
    235, 224, 212, 197, 180, 161, 141, 120, 97, 74, 49, 24,
];

const FINETUNE: [u32; 16] = [
    8363, 8413, 8463, 8529, 8581, 8651, 8723, 8757, 7895, 7941, 7985, 8046, 8107, 8169, 8232, 8280,
];

const KICK_ABSENT: u8 = 0;
const KICK_NOTE: u8 = 1;
const KICK_KEYOFF: u8 = 2;
const KICK_ENV: u8 = 4;

const POS_NONE: i16 = -2;
const PAN_SURROUND: i16 = 512;

/// libmikmod `md_volume` as SDL_mixer set it.
const MD_VOLUME: u32 = 96;
/// Software mixer volume ramp length (virtch `CLICK_BUFFER`) in output frames.
const CLICK_SHIFT: i32 = 6;
const RAMP: i32 = 1 << CLICK_SHIFT;
const FRACBITS: i64 = 11;
const FRACMASK: i64 = (1 << FRACBITS) - 1;

fn getoldperiod(note: u16, speed: u32) -> u16 {
    if speed == 0 {
        return 4242;
    }
    let n = (note % (2 * OCTAVE)) as usize;
    let o = note / (2 * OCTAVE);
    (((8363u64 * OLDPERIODS[n] as u64) >> o) / speed as u64) as u16
}

fn get_period(flags: u16, note: u16, speed: u32) -> u16 {
    if flags & uni::UF_XMPERIODS != 0 {
        if flags & uni::UF_LINEAR != 0 {
            // getlinearperiod
            (((20 + 2 * 2) * OCTAVE as i64 + 2 - note as i64) * 32 - (speed as i64 >> 1)) as u16
        } else {
            // Log periods aren't produced by UN04/05 loaders; fall back to Amiga periods.
            getoldperiod(note, 8363)
        }
    } else {
        getoldperiod(note, speed)
    }
}

fn get_frequency(period: u16) -> u32 {
    (8363 * 1712) / (if period == 0 { 1 } else { period as u32 })
}

#[derive(Clone, Copy, Default)]
struct Main {
    sample: Option<usize>,
    /// Index of the instrument/sample slot last selected (`main.sample`).
    slot: u8,
    note: u8,
    outvolume: i16,
    chanvol: i8,
    fadevol: u16,
    panning: i16,
    kick: u8,
    period: u16,
    notedelay: u8,
    start: i64,
}

#[derive(Clone, Copy)]
struct Control {
    main: Main,
    /// Byte offset of this tick's row inside its track, if any.
    row: Option<(usize, usize)>,
    anote: u8,
    ownper: bool,
    ownvol: bool,
    retrig: i8,
    speed: u32,
    volume: i16,
    tmpvolume: i16,
    tmpperiod: u16,
    wantedperiod: u16,
    arpmem: u8,
    slidespeed: u16,
    portspeed: u16,
    s3mtremor: u8,
    s3mtronof: u8,
    s3mvolslide: u8,
    s3mrtgspeed: u8,
    s3mrtgslide: u8,
    wavecontrol: u8,
    vibpos: i8,
    vibspd: u8,
    vibdepth: u8,
    trmpos: i8,
    trmspd: u8,
    trmdepth: u8,
    hioffset: u32,
    soffset: u16,
    pat_reppos: i16,
    pat_repcnt: u16,
}

impl Default for Control {
    fn default() -> Self {
        Control {
            main: Main::default(),
            row: None,
            anote: 0,
            ownper: false,
            ownvol: false,
            retrig: 0,
            speed: 0,
            volume: 0,
            tmpvolume: 0,
            tmpperiod: 0,
            wantedperiod: 0,
            arpmem: 0,
            slidespeed: 0,
            portspeed: 0,
            s3mtremor: 0,
            s3mtronof: 0,
            s3mvolslide: 0,
            s3mrtgspeed: 0,
            s3mrtgslide: 0,
            wavecontrol: 0,
            vibpos: 0,
            vibspd: 0,
            vibdepth: 0,
            trmpos: 0,
            trmspd: 0,
            trmdepth: 0,
            hioffset: 0,
            soffset: 0,
            pat_reppos: POS_NONE,
            pat_repcnt: 0,
        }
    }
}

/// One software-mixer voice (virtch `VINFO`), in the same 11-bit fixed point so pitch and
/// interpolation round exactly as libmikmod's did.
#[derive(Clone, Copy, Default)]
struct Voice {
    main: Main,
    active: bool,
    kick: bool,
    sample: usize,
    start: u32,
    flags: u16,
    size: u32,
    reppos: u32,
    repend: u32,
    frq: u32,
    vol: u32,
    pan: i16,
    current: i64,
    increment: i64,
    lvolsel: i32,
    rvolsel: i32,
    oldlvol: i32,
    oldrvol: i32,
    rampvol: i32,
}

/// Cursor over one row of a UniMod track (munitrk `UniSetRow`/`UniGetByte`).
struct RowReader<'a> {
    t: &'a [u8],
    pc: usize,
    end: usize,
    last: u8,
}

impl<'a> RowReader<'a> {
    fn new(t: &'a [u8], start: usize) -> Self {
        let end = start + (t[start] & 0x1f) as usize;
        RowReader {
            t,
            pc: start + 1,
            end,
            last: 0,
        }
    }
    fn byte(&mut self) -> u8 {
        self.last = if self.pc < self.end {
            self.pc += 1;
            self.t[self.pc - 1]
        } else {
            0
        };
        self.last
    }
    fn skip_opcode(&mut self) {
        if (self.last as usize) < OPERANDS.len() {
            for _ in 0..OPERANDS[self.last as usize] {
                self.byte();
            }
        }
    }
}

fn find_row(t: &[u8], mut row: u16) -> Option<usize> {
    let mut at = 0usize;
    loop {
        let c = *t.get(at)?;
        if c == 0 {
            return None;
        }
        let l = (c >> 5) as u16 + 1;
        if l > row {
            return Some(at);
        }
        row -= l;
        at += (c & 0x1f) as usize;
    }
}

pub struct ModPlayer {
    m: Module,
    rate: u32,
    control: Vec<Control>,
    voices: Vec<Voice>,
    panning: Vec<i16>,

    sngpos: i16,
    patpos: u16,
    numrow: u16,
    vbtick: u16,
    sngspd: u16,
    bpm: u16,
    volume: i16,
    initvolume: u8,
    patbrk: u16,
    posjmp: u8,
    patdly: u8,
    patdly2: u8,
    pat_repcrazy: bool,
    loop_: bool,

    tickleft: u32,
    rng: u32,
    acc: Vec<i32>,
}

impl ModPlayer {
    pub fn new(m: Module, rate: u32) -> ModPlayer {
        let n = m.numchn as usize;
        let mut control = vec![Control::default(); n];
        for (t, c) in control.iter_mut().enumerate() {
            c.main.chanvol = m.chanvol[t] as i8;
            c.main.panning = m.panning[t] as i16;
        }
        let panning = m.panning.iter().map(|&p| p as i16).collect();
        let sngspd = if m.initspeed != 0 {
            m.initspeed.min(32) as u16
        } else {
            6
        };
        let initvolume = m.initvolume;
        ModPlayer {
            control,
            voices: vec![Voice::default(); n],
            panning,
            sngpos: 0,
            patpos: 0,
            numrow: u16::MAX,
            vbtick: sngspd,
            sngspd,
            bpm: (m.inittempo as u16).max(32),
            volume: initvolume.min(128) as i16,
            initvolume,
            patbrk: 0,
            posjmp: 2,
            patdly: 0,
            patdly2: 0,
            pat_repcrazy: false,
            loop_: false,
            tickleft: 0,
            rng: 0x1234_5678,
            acc: Vec::new(),
            rate: rate.max(1),
            m,
        }
    }

    pub fn module(&self) -> &Module {
        &self.m
    }

    /// `Player_SetVolume`: 0..=128.
    pub fn set_volume(&mut self, v: u8) {
        let v = v.min(128);
        self.volume = v as i16;
        self.initvolume = v;
    }

    /// True once the song has played its last order (`!Player_Active`).
    pub fn finished(&self) -> bool {
        self.sngpos as i32 >= self.m.positions.len() as i32
            || self.m.positions.get(self.sngpos.max(0) as usize) == Some(&LAST_PATTERN)
    }

    /// Current (order, row), for tests and debugging.
    pub fn position(&self) -> (i16, u16) {
        (self.sngpos, self.patpos)
    }

    fn random(&mut self, ceil: u32) -> u16 {
        self.rng = self.rng.wrapping_mul(1_103_515_245).wrapping_add(12345);
        ((self.rng >> 16) & (ceil - 1)) as u16
    }

    // ---------------------------------------------------------------- tick handling

    fn handle_tick(&mut self) {
        if self.finished() {
            return;
        }
        self.vbtick += 1;
        if self.vbtick >= self.sngspd {
            if self.pat_repcrazy {
                self.pat_repcrazy = false;
            } else {
                self.patpos = self.patpos.wrapping_add(1);
            }
            self.vbtick = 0;

            if self.patdly != 0 {
                self.patdly2 = self.patdly;
                self.patdly = 0;
            }
            if self.patdly2 != 0 {
                self.patdly2 -= 1;
                if self.patdly2 != 0 && self.patpos != 0 {
                    self.patpos -= 1;
                }
            }

            if self.patpos >= self.numrow && self.numrow > 0 && self.posjmp == 0 {
                self.posjmp = 3;
            }

            if self.posjmp != 0 {
                self.patpos = if self.numrow != 0 {
                    self.patbrk % self.numrow
                } else {
                    0
                };
                self.pat_repcrazy = false;
                self.sngpos += self.posjmp as i16 - 2;
                for c in self.control.iter_mut() {
                    c.pat_reppos = -1;
                }
                self.patbrk = 0;
                self.posjmp = 0;
                if self.sngpos < 0 {
                    self.sngpos = self.m.positions.len() as i16 - 1;
                }
                if self.finished() {
                    // wrap = 0: the song ends here.
                    return;
                }
            }

            if self.patdly2 == 0 {
                self.notes();
            }
        }

        // fadeout = 0, so max_volume is always 128.
        self.effects_pass1();
        self.setup_voices();
        self.update_voices(128);
    }

    fn notes(&mut self) {
        let numpos = self.m.positions.len() as i16;
        for ch in 0..self.m.numchn as usize {
            let tr = if self.sngpos >= numpos {
                self.numrow = 0;
                None
            } else {
                let pat = self.m.positions[self.sngpos as usize] as usize;
                self.numrow = self.m.pattrows[pat];
                Some(self.m.patterns[pat * self.m.numchn as usize + ch] as usize)
            };
            let row = tr.and_then(|tr| find_row(&self.m.tracks[tr], self.patpos).map(|r| (tr, r)));
            let vbtick = self.vbtick;
            let a = &mut self.control[ch];
            a.row = row;
            if vbtick == 0 {
                a.main.notedelay = 0;
            }
            let Some((tr, at)) = row else { continue };
            let mut r = RowReader::new(&self.m.tracks[tr], at);
            let mut funky = 0;
            loop {
                let c = r.byte();
                if c == 0 {
                    break;
                }
                match c {
                    op::NOTE => {
                        funky |= 1;
                        a.anote = r.byte();
                        a.main.kick = KICK_NOTE;
                        a.main.start = -1;
                        if a.wavecontrol & 0x80 == 0 {
                            a.trmpos = 0;
                        }
                        if a.wavecontrol & 0x08 == 0 {
                            a.vibpos = 0;
                        }
                    }
                    op::INSTRUMENT => {
                        let inst = r.byte();
                        if inst as usize >= self.m.instruments.len() {
                            continue;
                        }
                        funky |= 2;
                        a.retrig = 0;
                        a.s3mtremor = 0;
                        a.main.slot = inst;
                    }
                    _ => r.skip_opcode(),
                }
            }
            if funky == 0 {
                continue;
            }
            let s_idx = if self.m.flags & uni::UF_INST != 0 {
                let ins = &self.m.instruments[a.main.slot as usize];
                let n = ins.samplenumber[(a.anote as usize).min(uni::INSTNOTES - 1)] as usize;
                if n >= self.m.samples.len() {
                    continue;
                }
                a.main.note = ins.samplenote[(a.anote as usize).min(uni::INSTNOTES - 1)];
                n
            } else {
                a.main.note = a.anote;
                a.main.slot as usize
            };
            let Some(s) = self.m.samples.get(s_idx) else {
                continue;
            };
            a.main.sample = Some(s_idx);
            a.main.panning = self.panning[ch];
            if s.flags & 0x1000 != 0 {
                a.main.panning = s.panning as i16;
            }
            a.speed = s.speed;
            if funky & 2 != 0 {
                a.volume = s.volume as i16;
                a.tmpvolume = s.volume as i16;
            }
            let p = get_period(self.m.flags, (a.main.note as u16) << 1, a.speed);
            a.wantedperiod = p;
            a.tmpperiod = p;
        }
    }

    fn effects_pass1(&mut self) {
        for ch in 0..self.m.numchn as usize {
            {
                let v = self.voices[ch];
                let a = &mut self.control[ch];
                a.main.fadevol = v.main.fadevol;
                a.main.period = v.main.period;
            }
            let Some((tr, at)) = self.control[ch].row else {
                continue;
            };
            self.control[ch].ownper = false;
            self.control[ch].ownvol = false;
            self.play_effects(ch, tr, at);
            let a = &mut self.control[ch];
            if !a.ownper {
                a.main.period = a.tmpperiod;
            }
            if !a.ownvol {
                a.volume = a.tmpvolume;
            }
            if let Some(s) = a.main.sample {
                let gv = self.m.samples[s].globvol as i32;
                a.main.outvolume = ((a.volume as i32 * gv) >> 4).clamp(0, 256) as i16;
            }
        }
    }

    fn play_effects(&mut self, ch: usize, tr: usize, at: usize) {
        // Tracks are immutable; split the borrow by cloning the row slice bounds.
        let track = std::mem::take(&mut self.m.tracks[tr]);
        {
            let mut r = RowReader::new(&track, at);
            loop {
                let c = r.byte();
                if c == 0 {
                    break;
                }
                let dat = if OPERANDS.get(c as usize).copied().unwrap_or(0) > 0 {
                    r.byte()
                } else {
                    0
                };
                // Opcodes with two operands consume the second here.
                if OPERANDS.get(c as usize).copied().unwrap_or(0) > 1 {
                    r.byte();
                }
                self.effect(ch, c, dat);
            }
        }
        self.m.tracks[tr] = track;
    }

    fn effect(&mut self, ch: usize, c: u8, dat: u8) {
        let tick = self.vbtick;
        let flags = self.m.flags;
        match c {
            op::NOTE | op::INSTRUMENT => {}
            op::PTEFFECT0 => {
                let a = &mut self.control[ch];
                if tick == 0 {
                    a.arpmem = dat;
                }
                if a.main.period != 0 && a.arpmem != 0 {
                    let mut note = a.main.note;
                    match tick % 3 {
                        1 => note = note.wrapping_add(a.arpmem >> 4),
                        2 => note = note.wrapping_add(a.arpmem & 0xf),
                        _ => {}
                    }
                    a.main.period = get_period(flags, (note as u16) << 1, a.speed);
                    a.ownper = true;
                }
            }
            op::PTEFFECT1 | op::PTEFFECT2 => {
                let a = &mut self.control[ch];
                if tick == 0 && dat != 0 {
                    a.slidespeed = (dat as u16) << 2;
                }
                if a.main.period != 0 && tick != 0 {
                    a.tmpperiod = if c == op::PTEFFECT1 {
                        a.tmpperiod.wrapping_sub(a.slidespeed)
                    } else {
                        a.tmpperiod.wrapping_add(a.slidespeed)
                    };
                }
            }
            op::PTEFFECT3 => {
                let a = &mut self.control[ch];
                if tick == 0 && dat != 0 {
                    a.portspeed = (dat as u16) << 2;
                }
                if a.main.period != 0 {
                    tone_slide(tick, a);
                }
            }
            op::PTEFFECT4 => {
                let a = &mut self.control[ch];
                if tick == 0 {
                    if dat & 0x0f != 0 {
                        a.vibdepth = dat & 0xf;
                    }
                    if dat & 0xf0 != 0 {
                        a.vibspd = (dat & 0xf0) >> 2;
                    }
                }
                if self.control[ch].main.period != 0 {
                    self.vibrato(tick, ch);
                }
            }
            op::PTEFFECT5 => {
                let a = &mut self.control[ch];
                if a.main.period != 0 {
                    tone_slide(tick, a);
                }
                if tick != 0 {
                    vol_slide(a, dat);
                }
            }
            op::PTEFFECT6 => {
                if self.control[ch].main.period != 0 {
                    self.vibrato(tick, ch);
                }
                if tick != 0 {
                    vol_slide(&mut self.control[ch], dat);
                }
            }
            op::PTEFFECT7 => {
                {
                    let a = &mut self.control[ch];
                    if tick == 0 {
                        if dat & 0x0f != 0 {
                            a.trmdepth = dat & 0xf;
                        }
                        if dat & 0xf0 != 0 {
                            a.trmspd = (dat & 0xf0) >> 2;
                        }
                    }
                }
                if self.control[ch].main.period != 0 {
                    self.tremolo(tick, ch, 6);
                }
            }
            op::PTEFFECT8 => {
                self.control[ch].main.panning = dat as i16;
                self.panning[ch] = dat as i16;
            }
            op::PTEFFECT9 => {
                if tick == 0 {
                    let a = &mut self.control[ch];
                    if dat != 0 {
                        a.soffset = (dat as u16) << 8;
                    }
                    a.main.start = (a.hioffset | a.soffset as u32) as i64;
                    if let Some(s) = a.main.sample.map(|i| &self.m.samples[i]) {
                        if a.main.start > s.length as i64 {
                            a.main.start = if s.flags & (SF_LOOP | SF_BIDI) != 0 {
                                s.loopstart as i64
                            } else {
                                s.length as i64
                            };
                        }
                    }
                }
            }
            op::PTEFFECTA => {
                if tick != 0 {
                    vol_slide(&mut self.control[ch], dat);
                }
            }
            op::PTEFFECTB => self.pattern_jump(tick, dat, flags),
            op::PTEFFECTC => {
                if tick == 0 {
                    let a = &mut self.control[ch];
                    let mut d = dat;
                    if d == 0xff {
                        a.anote = 0;
                        d = 0;
                    } else if d > 64 {
                        d = 64;
                    }
                    a.tmpvolume = d as i16;
                }
            }
            op::PTEFFECTD => self.pattern_break(tick, dat, flags),
            op::PTEFFECTE => self.e_effects(tick, ch, dat, flags),
            op::PTEFFECTF => {
                if tick == 0 && self.patdly2 == 0 {
                    // extspd = 1
                    if dat as u16 >= self.m.bpmlimit {
                        self.bpm = dat as u16;
                    } else if dat != 0 {
                        self.sngspd = dat as u16;
                        self.vbtick = 0;
                    }
                }
            }
            op::S3MEFFECTA => {
                if tick == 0 && self.patdly2 == 0 {
                    let speed = if dat > 128 { dat - 128 } else { dat };
                    if speed != 0 {
                        self.sngspd = speed as u16;
                        self.vbtick = 0;
                    }
                }
            }
            op::S3MEFFECTD => s3m_vol_slide(tick, flags, &mut self.control[ch], dat),
            op::S3MEFFECTE | op::S3MEFFECTF => {
                let a = &mut self.control[ch];
                if a.main.period != 0 {
                    let inf = if dat != 0 {
                        a.slidespeed = dat as u16;
                        dat
                    } else {
                        a.slidespeed as u8
                    };
                    let (hi, lo) = (inf >> 4, inf & 0xf);
                    let delta = if hi == 0xf {
                        (tick == 0).then_some((lo as u16) << 2)
                    } else if hi == 0xe {
                        (tick == 0).then_some(lo as u16)
                    } else {
                        (tick != 0).then_some((inf as u16) << 2)
                    };
                    if let Some(d) = delta {
                        a.tmpperiod = if c == op::S3MEFFECTE {
                            a.tmpperiod.wrapping_add(d)
                        } else {
                            a.tmpperiod.wrapping_sub(d)
                        };
                    }
                }
            }
            op::S3MEFFECTI => {
                let a = &mut self.control[ch];
                let inf = if dat != 0 {
                    a.s3mtronof = dat;
                    dat
                } else {
                    a.s3mtronof
                };
                if inf != 0 && tick != 0 {
                    let on = (inf >> 4) + 1;
                    let off = (inf & 0xf) + 1;
                    a.s3mtremor %= on + off;
                    a.volume = if a.s3mtremor < on { a.tmpvolume } else { 0 };
                    a.ownvol = true;
                    a.s3mtremor = a.s3mtremor.wrapping_add(1);
                }
            }
            op::S3MEFFECTQ => {
                let a = &mut self.control[ch];
                if a.main.period != 0 {
                    if dat != 0 {
                        a.s3mrtgslide = dat >> 4;
                        a.s3mrtgspeed = dat & 0xf;
                    }
                    if a.s3mrtgspeed > 0 {
                        if a.retrig == 0 {
                            if a.main.kick != KICK_NOTE {
                                a.main.kick = KICK_KEYOFF;
                            }
                            a.retrig = a.s3mrtgspeed as i8;
                            if tick != 0 || flags & 0x0010 != 0 {
                                let v = a.tmpvolume;
                                a.tmpvolume = match a.s3mrtgslide {
                                    1..=5 => v - (1 << (a.s3mrtgslide - 1)),
                                    6 => (2 * v) / 3,
                                    7 => v >> 1,
                                    9..=0xd => v + (1 << (a.s3mrtgslide - 9)),
                                    0xe => (3 * v) >> 1,
                                    0xf => v << 1,
                                    _ => v,
                                }
                                .clamp(0, 64);
                            }
                        }
                        a.retrig -= 1;
                    }
                }
            }
            op::S3MEFFECTR => {
                let a = &mut self.control[ch];
                if tick == 0 {
                    if dat & 0x0f != 0 {
                        a.trmdepth = dat & 0xf;
                    }
                    if dat & 0xf0 != 0 {
                        a.trmspd = (dat & 0xf0) >> 2;
                    }
                }
                self.tremolo(tick, ch, 7);
            }
            op::S3MEFFECTT if tick == 0 && self.patdly2 == 0 => {
                self.bpm = (dat as u16).max(32);
            }
            _ => {}
        }
    }

    fn lfo(&mut self, pos: i8, wave: u8) -> u16 {
        let mut q = ((pos as u8 >> 2) & 0x1f) as u16;
        match wave & 3 {
            0 => VIBRATO_TABLE[q as usize] as u16,
            1 => {
                q = (q << 3) & 0xff;
                if pos < 0 {
                    q = 255 - q;
                }
                q
            }
            2 => 255,
            _ => self.random(256),
        }
    }

    fn vibrato(&mut self, tick: u16, ch: usize) {
        if tick == 0 {
            return;
        }
        let (pos, wave) = (self.control[ch].vibpos, self.control[ch].wavecontrol);
        let mut temp = self.lfo(pos, wave);
        let a = &mut self.control[ch];
        temp = temp.wrapping_mul(a.vibdepth as u16);
        temp >>= 7;
        temp <<= 2;
        a.main.period = if a.vibpos >= 0 {
            a.tmpperiod.wrapping_add(temp)
        } else {
            a.tmpperiod.wrapping_sub(temp)
        };
        a.ownper = true;
        a.vibpos = a.vibpos.wrapping_add(a.vibspd as i8);
    }

    fn tremolo(&mut self, tick: u16, ch: usize, shift: u32) {
        let (pos, wave) = (self.control[ch].trmpos, self.control[ch].wavecontrol >> 4);
        let mut temp = self.lfo(pos, wave);
        let a = &mut self.control[ch];
        temp = temp.wrapping_mul(a.trmdepth as u16);
        temp >>= shift;
        a.volume = if a.trmpos >= 0 {
            (a.tmpvolume + temp as i16).min(64)
        } else {
            (a.tmpvolume - temp as i16).max(0)
        };
        a.ownvol = true;
        if tick != 0 {
            a.trmpos = a.trmpos.wrapping_add(a.trmspd as i8);
        }
    }

    fn pattern_jump(&mut self, tick: u16, dat: u8, flags: u16) {
        if tick != 0 || self.patdly2 != 0 {
            return;
        }
        let dat = dat as i16;
        if dat == self.sngpos && self.patbrk == self.patpos {
            return;
        }
        let numpos = self.m.positions.len() as i16;
        if !self.loop_
            && self.patbrk == 0
            && (dat < self.sngpos
                || (self.sngpos == numpos - 1 && self.patbrk == 0)
                || (dat == self.sngpos && flags & uni::UF_NOWRAP != 0))
        {
            self.posjmp = 3;
        } else {
            if self.sngpos == numpos - 1 {
                self.volume = self.initvolume.min(128) as i16;
            }
            self.sngpos = dat;
            self.posjmp = 2;
            self.patpos = 0;
        }
    }

    fn pattern_break(&mut self, tick: u16, dat: u8, flags: u16) {
        if tick != 0 || self.patdly2 != 0 {
            return;
        }
        let mut dat = dat as u16;
        let pos = self.m.positions.get(self.sngpos.max(0) as usize).copied();
        if let Some(p) = pos.filter(|&p| p != LAST_PATTERN) {
            let rows = self.m.pattrows[p as usize];
            if dat > rows {
                dat = rows;
            }
        }
        self.patbrk = dat;
        if self.posjmp == 0 {
            let numpos = self.m.positions.len() as i16;
            let last_pat = pos == Some(self.m.pattrows.len() as u16 - 1);
            if self.sngpos == numpos - 1
                && dat != 0
                && (self.loop_ || (last_pat && flags & uni::UF_NOWRAP == 0))
            {
                self.sngpos = 0;
                self.posjmp = 2;
            } else {
                self.posjmp = 3;
            }
        }
    }

    fn e_effects(&mut self, tick: u16, ch: usize, dat: u8, flags: u16) {
        let nib = dat & 0xf;
        let a = &mut self.control[ch];
        match dat >> 4 {
            0x1 => {
                if a.main.period != 0 && tick == 0 {
                    a.tmpperiod = a.tmpperiod.wrapping_sub((nib as u16) << 2);
                }
            }
            0x2 => {
                if a.main.period != 0 && tick == 0 {
                    a.tmpperiod = a.tmpperiod.wrapping_add((nib as u16) << 2);
                }
            }
            0x4 => a.wavecontrol = (a.wavecontrol & 0xf0) | nib,
            0x5 => {
                if a.main.period != 0 {
                    a.speed = if flags & uni::UF_XMPERIODS != 0 {
                        nib as u32 + 128
                    } else {
                        FINETUNE[nib as usize]
                    };
                    a.tmpperiod = get_period(flags, (a.main.note as u16) << 1, a.speed);
                }
            }
            0x6 => {
                if tick != 0 {
                    return;
                }
                if nib != 0 {
                    if a.pat_repcnt != 0 {
                        a.pat_repcnt -= 1;
                    } else {
                        a.pat_repcnt = nib as u16;
                    }
                    if a.pat_repcnt != 0 {
                        if a.pat_reppos == POS_NONE {
                            a.pat_reppos = self.patpos as i16 - 1;
                        }
                        if a.pat_reppos == -1 {
                            self.pat_repcrazy = true;
                            self.patpos = 0;
                        } else {
                            self.patpos = a.pat_reppos as u16;
                        }
                    } else {
                        a.pat_reppos = POS_NONE;
                    }
                } else {
                    a.pat_reppos = self.patpos as i16 - 1;
                }
            }
            0x7 => a.wavecontrol = (a.wavecontrol & 0x0f) | (nib << 4),
            0x8 => {
                let p = if nib <= 8 {
                    (nib as i16) << 4
                } else {
                    nib as i16 * 17
                };
                a.main.panning = p;
                self.panning[ch] = p;
            }
            0x9 => {
                if tick == 0 {
                    return;
                }
                if nib != 0 {
                    if a.retrig == 0 {
                        if a.main.period != 0 {
                            a.main.kick = KICK_NOTE;
                        }
                        a.retrig = nib as i8;
                    }
                    a.retrig -= 1;
                }
            }
            0xa => {
                if tick == 0 {
                    a.tmpvolume = (a.tmpvolume + nib as i16).min(64);
                }
            }
            0xb => {
                if tick == 0 {
                    a.tmpvolume = (a.tmpvolume - nib as i16).max(0);
                }
            }
            0xc => {
                if tick >= nib as u16 {
                    a.tmpvolume = 0;
                }
            }
            0xd => {
                if tick == 0 {
                    a.main.notedelay = nib;
                } else if a.main.notedelay != 0 {
                    a.main.notedelay -= 1;
                }
            }
            0xe if tick == 0 && self.patdly2 == 0 => {
                self.patdly = nib + 1;
            }
            _ => {}
        }
    }

    fn setup_voices(&mut self) {
        for ch in 0..self.m.numchn as usize {
            let a = &mut self.control[ch];
            if a.main.notedelay != 0 {
                continue;
            }
            self.voices[ch].main = a.main;
            a.main.kick = KICK_ABSENT;
        }
    }

    fn update_voices(&mut self, max_volume: u32) {
        for ch in 0..self.voices.len() {
            let Some(si) = self.voices[ch].main.sample else {
                continue;
            };
            let s = &self.m.samples[si];
            if s.length == 0 {
                continue;
            }
            let v = &mut self.voices[ch];
            v.main.period = v.main.period.clamp(40, 50000);

            if v.main.kick == KICK_NOTE || v.main.kick == KICK_KEYOFF {
                // Voice_Play
                let start = if v.main.start == -1 {
                    0
                } else {
                    v.main.start as u32
                };
                let mut repend = s.loopend;
                if s.flags & SF_LOOP != 0 && repend > s.length {
                    repend = s.length;
                }
                v.sample = si;
                v.start = start;
                v.size = s.length;
                v.reppos = s.loopstart;
                v.repend = repend;
                v.flags = s.flags;
                v.kick = true;
                v.main.fadevol = 32768;
            }
            v.main.kick = KICK_ABSENT;

            let mut tmpvol = v.main.fadevol as u32;
            tmpvol *= v.main.chanvol.max(0) as u32;
            tmpvol *= v.main.outvolume.max(0) as u32;
            tmpvol /= 256 * 64;
            tmpvol *= 256; // envvol
            tmpvol *= self.volume.max(0) as u32;
            tmpvol /= 128 * 256 * 128;
            tmpvol = if self.sngpos as i32 >= self.m.positions.len() as i32 {
                0
            } else {
                (tmpvol * max_volume) / 128
            };
            // Voice_SetVolume: vol * md_volume * md_musicvolume(128) / 16384
            let vol = tmpvol * MD_VOLUME * 128 / 16384;
            if (v.vol as i32 - vol as i32).abs() > 32 {
                v.rampvol = RAMP;
            }
            v.vol = vol;
            v.pan = v.main.panning;

            if v.main.fadevol == 0 {
                v.active = false;
            } else {
                v.frq = get_frequency(v.main.period);
            }
        }
        // md_bpm clamp
        self.bpm = self.bpm.clamp(32, 255);
    }

    // ---------------------------------------------------------------- mixing

    /// Renders interleaved stereo f32 frames into `out` (len must be even). After the song ends,
    /// the rest is silence; check [`ModPlayer::finished`].
    pub fn render(&mut self, out: &mut [f32]) {
        let mut frames = out.len() / 2;
        let mut at = 0;
        let mut acc = std::mem::take(&mut self.acc);
        while frames > 0 {
            if self.tickleft == 0 {
                self.handle_tick();
                self.tickleft = (self.rate * 125) / (self.bpm as u32 * 50);
            }
            let n = (self.tickleft as usize).min(frames);
            acc.clear();
            acc.resize(n * 2, 0);
            self.mix(&mut acc);
            // Mix32To16: drop the 9-bit volume headroom and clip.
            for (o, &x) in out[at * 2..(at + n) * 2].iter_mut().zip(acc.iter()) {
                *o = (x >> 9).clamp(-32768, 32767) as f32 / 32768.0;
            }
            self.tickleft -= n as u32;
            frames -= n;
            at += n;
        }
        self.acc = acc;
    }

    fn mix(&mut self, acc: &mut [i32]) {
        let rate = self.rate as i64;
        let todo_all = (acc.len() / 2) as i64;
        for v in self.voices.iter_mut() {
            if v.kick {
                v.current = (v.start as i64) << FRACBITS;
                v.kick = false;
                v.active = true;
            }
            if v.frq == 0 {
                v.active = false;
            }
            if !v.active {
                continue;
            }
            v.increment = ((v.frq as i64) << FRACBITS) / rate;
            if v.flags & SF_REVERSE != 0 {
                v.increment = -v.increment;
            }
            v.oldlvol = v.lvolsel;
            v.oldrvol = v.rvolsel;
            let vol = v.vol as i32;
            if v.pan == PAN_SURROUND {
                v.lvolsel = vol / 2;
                v.rvolsel = vol / 2;
            } else {
                let pan = v.pan as i32;
                v.lvolsel = (vol * (255 - pan)) >> 8;
                v.rvolsel = (vol * pan) >> 8;
            }
            let idxsize = if v.size != 0 {
                ((v.size as i64) << FRACBITS) - 1
            } else {
                0
            };
            let idxlend = if v.repend != 0 {
                ((v.repend as i64) << FRACBITS) - 1
            } else {
                0
            };
            let idxlpos = (v.reppos as i64) << FRACBITS;
            let data = &self.m.samples[v.sample].data;
            add_channel(v, data, acc, todo_all, idxsize, idxlend, idxlpos);
        }
    }
}

/// virtch `AddChannel`: advance one voice over `todo` frames, handling loops, and mix it.
fn add_channel(
    v: &mut Voice,
    s: &[i16],
    acc: &mut [i32],
    mut todo: i64,
    idxsize: i64,
    idxlend: i64,
    idxlpos: i64,
) {
    if s.is_empty() {
        v.current = 0;
        v.active = false;
        return;
    }
    let mut at = 0usize;
    while todo > 0 {
        if v.flags & SF_REVERSE != 0 {
            if v.flags & SF_LOOP != 0 && v.current < idxlpos {
                if v.flags & SF_BIDI != 0 {
                    v.current = idxlpos + (idxlpos - v.current);
                    v.flags &= !SF_REVERSE;
                    v.increment = -v.increment;
                } else {
                    v.current = idxlend - (idxlpos - v.current);
                }
            } else if v.current < 0 {
                v.current = 0;
                v.active = false;
                break;
            }
        } else if v.flags & SF_LOOP != 0 && v.current >= idxlend {
            if v.flags & SF_BIDI != 0 {
                v.flags |= SF_REVERSE;
                v.increment = -v.increment;
                v.current = idxlend - (v.current - idxlend);
            } else {
                v.current = idxlpos + (v.current - idxlend);
            }
        } else if v.current >= idxsize {
            v.current = 0;
            v.active = false;
            break;
        }

        let end = if v.flags & SF_REVERSE != 0 {
            if v.flags & SF_LOOP != 0 {
                idxlpos
            } else {
                0
            }
        } else if v.flags & SF_LOOP != 0 {
            idxlend
        } else {
            idxsize
        };
        let done = if end == v.current || v.increment == 0 {
            0
        } else {
            ((end - v.current) / v.increment + 1).clamp(0, todo)
        };
        if done == 0 {
            v.active = false;
            break;
        }
        let endpos = v.current + done * v.increment;
        if v.vol != 0 {
            v.current = mix_stereo_interp(v, s, &mut acc[at * 2..], v.current, done);
        } else {
            v.current = endpos;
        }
        todo -= done;
        at += done as usize;
    }
}

/// virtch `MixStereoInterp`.
fn mix_stereo_interp(v: &mut Voice, s: &[i16], dest: &mut [i32], mut idx: i64, todo: i64) -> i64 {
    let at = |i: i64| -> i32 { s.get(i as usize).copied().unwrap_or(0) as i32 };
    let sample_at = |idx: i64| -> i32 {
        let i = idx >> FRACBITS;
        let a = at(i);
        a + (((at(i + 1) - a) as i64 * (idx & FRACMASK)) >> FRACBITS) as i32
    };
    let (l, r) = (v.lvolsel, v.rvolsel);
    let mut n = 0usize;
    let mut todo = todo;
    if v.rampvol != 0 {
        let oldl = v.oldlvol - l;
        let oldr = v.oldrvol - r;
        while todo > 0 {
            todo -= 1;
            let sample = sample_at(idx);
            idx += v.increment;
            dest[n] += (((l << CLICK_SHIFT) + oldl * v.rampvol) * sample) >> CLICK_SHIFT;
            dest[n + 1] += (((r << CLICK_SHIFT) + oldr * v.rampvol) * sample) >> CLICK_SHIFT;
            n += 2;
            v.rampvol -= 1;
            if v.rampvol == 0 {
                break;
            }
        }
    }
    while todo > 0 {
        todo -= 1;
        let sample = sample_at(idx);
        idx += v.increment;
        dest[n] += l * sample;
        dest[n + 1] += r * sample;
        n += 2;
    }
    idx
}

fn vol_slide(a: &mut Control, dat: u8) {
    if dat & 0xf != 0 {
        a.tmpvolume = (a.tmpvolume - (dat & 0x0f) as i16).max(0);
    } else {
        a.tmpvolume = (a.tmpvolume + (dat >> 4) as i16).min(64);
    }
}

fn tone_slide(tick: u16, a: &mut Control) {
    if a.main.fadevol == 0 {
        a.main.kick = if a.main.kick == KICK_NOTE {
            KICK_NOTE
        } else {
            KICK_KEYOFF
        };
    } else {
        a.main.kick = if a.main.kick == KICK_NOTE {
            KICK_ENV
        } else {
            KICK_ABSENT
        };
    }
    if tick != 0 {
        let dist = a.main.period as i32 - a.wantedperiod as i32;
        if dist == 0 || a.portspeed as i32 > dist.abs() {
            a.tmpperiod = a.wantedperiod;
            a.main.period = a.wantedperiod;
        } else if dist > 0 {
            a.tmpperiod = a.tmpperiod.wrapping_sub(a.portspeed);
            a.main.period = a.main.period.wrapping_sub(a.portspeed);
        } else {
            a.tmpperiod = a.tmpperiod.wrapping_add(a.portspeed);
            a.main.period = a.main.period.wrapping_add(a.portspeed);
        }
    } else {
        a.tmpperiod = a.main.period;
    }
    a.ownper = true;
}

fn s3m_vol_slide(tick: u16, flags: u16, a: &mut Control, inf: u8) {
    let inf = if inf != 0 {
        a.s3mvolslide = inf;
        inf
    } else {
        a.s3mvolslide
    };
    let (lo, hi) = (inf & 0xf, inf >> 4);
    let s3mslides = flags & 0x0010 != 0;
    if lo == 0 {
        if tick != 0 || s3mslides {
            a.tmpvolume += hi as i16;
        }
    } else if hi == 0 {
        if tick != 0 || s3mslides {
            a.tmpvolume -= lo as i16;
        }
    } else if lo == 0xf {
        if tick == 0 {
            a.tmpvolume += if hi != 0 { hi as i16 } else { 0xf };
        }
    } else if hi == 0xf {
        if tick == 0 {
            a.tmpvolume -= if lo != 0 { lo as i16 } else { 0xf };
        }
    } else {
        return;
    }
    a.tmpvolume = a.tmpvolume.clamp(0, 64);
}
