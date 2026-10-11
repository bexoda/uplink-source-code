use ur_save::store::LocalStore;
use ur_save::*;
use ur_sim::World;

fn sample() -> World {
    let mut w = World::new(99);
    for _ in 0..50 {
        w.tick(&mut vec![]);
    }
    w
}

fn bytes_of(w: &World) -> Vec<u8> {
    postcard::to_allocvec(w).unwrap()
}

#[test]
fn key_roundtrip_and_compression() {
    let w = sample();
    let key = Key::generate();
    let sealed = seal(&w, Unlock::Key(&key)).unwrap();
    assert!(sealed.len() < bytes_of(&w).len() + 100);
    let back = open(&sealed, Unlock::Key(&key)).unwrap();
    assert_eq!(bytes_of(&w), bytes_of(&back));
}

#[test]
fn passphrase_roundtrip() {
    let w = sample();
    let sealed = seal(&w, Unlock::Passphrase("correct horse")).unwrap();
    assert!(peek(&sealed).unwrap().passphrase);
    assert!(open(&sealed, Unlock::Passphrase("correct horse")).is_ok());
    assert_eq!(
        open(&sealed, Unlock::Passphrase("wrong")).unwrap_err(),
        SaveError::Decrypt
    );
}

#[test]
fn wrong_key_and_mode_rejected() {
    let w = sample();
    let (k1, k2) = (Key::generate(), Key::generate());
    let sealed = seal(&w, Unlock::Key(&k1)).unwrap();
    assert_eq!(
        open(&sealed, Unlock::Key(&k2)).unwrap_err(),
        SaveError::Decrypt
    );
    assert_eq!(
        open(&sealed, Unlock::Passphrase("x")).unwrap_err(),
        SaveError::KeyModeMismatch
    );
}

#[test]
fn any_single_bit_flip_is_detected() {
    let w = World::new(1);
    let key = Key::generate();
    let sealed = seal(&w, Unlock::Key(&key)).unwrap();
    // Header bytes and a spread of ciphertext bytes (incl. the tag at the end).
    for i in (0..sealed.len()).step_by(7).chain([sealed.len() - 1]) {
        let mut t = sealed.clone();
        t[i] ^= 1;
        assert!(
            open(&t, Unlock::Key(&key)).is_err(),
            "flip at byte {i} was accepted"
        );
    }
}

#[test]
fn garbage_and_truncation_rejected() {
    let key = Key::generate();
    assert_eq!(
        open(b"nope", Unlock::Key(&key)).unwrap_err(),
        SaveError::Truncated
    );
    assert_eq!(
        open(&[0u8; 100], Unlock::Key(&key)).unwrap_err(),
        SaveError::BadMagic
    );
    let mut sealed = seal(&World::new(1), Unlock::Key(&key)).unwrap();
    sealed.truncate(sealed.len() - 20);
    assert!(open(&sealed, Unlock::Key(&key)).is_err());
}

#[test]
fn nonce_is_fresh_each_seal() {
    let (w, key) = (World::new(1), Key::generate());
    assert_ne!(
        seal(&w, Unlock::Key(&key)).unwrap(),
        seal(&w, Unlock::Key(&key)).unwrap()
    );
}

#[test]
fn local_store_is_atomic_keeps_backup_and_validates_names() {
    let dir = tempfile::tempdir().unwrap();
    let st = LocalStore::new(dir.path()).unwrap();
    st.write("main", b"one").unwrap();
    st.write("main", b"two").unwrap();
    assert_eq!(st.read("main").unwrap(), b"two");
    assert_eq!(st.read_backup("main").unwrap(), b"one");
    assert_eq!(st.slots().unwrap(), vec!["main"]);
    assert!(st.write("../evil", b"x").is_err());
    assert!(st.read("a/b").is_err());
    assert!(!dir.path().join("main.glsave.tmp").exists());

    assert_eq!(st.read_sync_state("main").unwrap(), None);
    let s = sync::SyncState {
        base: 4,
        dirty: true,
    };
    st.write_sync_state("main", s).unwrap();
    assert_eq!(st.read_sync_state("main").unwrap(), Some(s));
}

#[test]
fn slot_names() {
    assert!(slot::is_valid("main-1_a"));
    for bad in ["", "A", "a b", "../x", "a.b", &"x".repeat(33)] {
        assert!(!slot::is_valid(bad), "{bad}");
    }
}

#[test]
fn wire_roundtrips_and_bounds_hostile_input() {
    use ur_sim::replay::*;
    let mut run = Run::new(4);
    run.apply(ur_sim::Command::AcceptMission(1), &mut vec![])
        .unwrap();
    let seg = run.segment();
    assert_eq!(decode_segment(&encode_segment(&seg).unwrap()).unwrap(), seg);

    let w = sample();
    let back = decode_state(&encode_state(&w).unwrap()).unwrap();
    assert_eq!(digest(&w).unwrap(), digest(&back).unwrap());
    assert_ne!(digest(&w).unwrap(), digest(&World::new(1)).unwrap());

    // A "decompression bomb": claims a huge inflated size.
    let mut bomb = encode_segment(&seg).unwrap();
    bomb[..4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(decode_segment(&bomb).is_err());
    assert!(decode_segment(b"junk").is_err());
}

#[test]
fn run_save_keeps_world_seed_and_unsynced_log() {
    use ur_sim::replay::Run;
    let mut run = Run::new(77);
    run.apply(ur_sim::Command::AcceptMission(1), &mut vec![])
        .unwrap();
    for _ in 0..30 {
        run.tick(&mut vec![]);
    }
    let key = Key::generate();
    let sealed = seal_run(&run, Unlock::Key(&key)).unwrap();
    assert_eq!(peek(&sealed).unwrap().format, RUN_FORMAT_VERSION);
    let back = open_run(&sealed, Unlock::Key(&key)).unwrap();
    assert_eq!((back.seed, back.unsynced()), (77, 1));
    assert_eq!(back.segment(), run.segment());
    assert_eq!(bytes_of(&back.world), bytes_of(&run.world));
    // The two payload kinds can't be confused for each other.
    assert_eq!(
        open(&sealed, Unlock::Key(&key)).unwrap_err(),
        SaveError::UnsupportedFormat(RUN_FORMAT_VERSION)
    );
    let world_save = seal(&run.world, Unlock::Key(&key)).unwrap();
    assert_eq!(
        open_run(&world_save, Unlock::Key(&key)).unwrap_err(),
        SaveError::UnsupportedFormat(FORMAT_VERSION)
    );
}
