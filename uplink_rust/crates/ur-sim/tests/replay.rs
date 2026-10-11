use slotmap::KeyData;
use ur_sim::replay::*;
use ur_sim::*;

fn bytes(w: &World) -> Vec<u8> {
    postcard::to_allocvec(w).unwrap()
}

/// A short, always-legal session on any seed.
fn play(run: &mut Run) {
    let mut ev = vec![];
    let relays: Vec<_> = run
        .world
        .servers
        .iter()
        .filter(|(_, s)| s.kind == ServerKind::Relay)
        .map(|(i, _)| i)
        .collect();
    let corp = run
        .world
        .servers
        .iter()
        .find(|(_, s)| s.kind == ServerKind::Corp)
        .map(|(i, _)| i)
        .unwrap();
    run.apply(Command::AcceptMission(1), &mut ev).unwrap();
    run.apply(Command::BuyTool(Tool::Decrypter), &mut ev)
        .unwrap();
    assert!(run
        .apply(Command::BuyTool(Tool::Decrypter), &mut ev)
        .is_err());
    run.apply(
        Command::Connect {
            chain: relays,
            target: corp,
        },
        &mut ev,
    )
    .unwrap();
    for _ in 0..50 {
        run.tick(&mut ev);
    }
    // May or may not be legal depending on the seed's layers; either way the log stays honest.
    let _ = run.apply(
        Command::Run {
            tool: Tool::PasswordBreaker,
            server: corp,
            file: None,
        },
        &mut ev,
    );
    for _ in 0..400 {
        run.tick(&mut ev);
    }
}

#[test]
fn replay_reproduces_live_state_exactly() {
    for seed in [1, 2, 3, 99, 12345] {
        let mut run = Run::new(seed);
        play(&mut run);
        let seg = run.segment();
        let mut w = World::new(seed);
        replay(&mut w, &seg, Limits::default()).unwrap();
        assert_eq!(bytes(&w), bytes(&run.world), "seed {seed}");
    }
}

#[test]
fn memory_edit_never_reaches_the_replayed_state() {
    let mut run = Run::new(7);
    play(&mut run);
    let honest = run.world.player.credits;
    run.world.player.credits += 1_000_000; // what Cheat Engine does
    run.world.player.rating = 9999;

    let mut server_world = World::new(7);
    replay(&mut server_world, &run.segment(), Limits::default()).unwrap();
    assert_eq!(server_world.player.credits, honest);
    assert_eq!(server_world.player.rating, 0);
}

#[test]
fn segments_chain_and_acknowledge_survives_concurrent_play() {
    let mut run = Run::new(5);
    play(&mut run);
    let seg1 = run.segment();
    // The player keeps playing while the upload is in flight.
    run.apply(Command::Disconnect, &mut vec![]).unwrap();
    for _ in 0..20 {
        run.tick(&mut vec![]);
    }
    run.acknowledge(&seg1);
    let seg2 = run.segment();
    assert_eq!(seg2.start_tick, seg1.end_tick);
    assert_eq!(seg2.entries.len(), 1);

    let mut server = World::new(5);
    replay(&mut server, &seg1, Limits::default()).unwrap();
    replay(&mut server, &seg2, Limits::default()).unwrap();
    assert_eq!(bytes(&server), bytes(&run.world));
}

#[test]
fn resume_from_server_state_continues_the_chain() {
    let mut run = Run::new(5);
    play(&mut run);
    let mut server = World::new(5);
    replay(&mut server, &run.segment(), Limits::default()).unwrap();
    // Second device downloads the verified state and keeps going.
    let mut dev2 = Run::resume(5, server.clone());
    dev2.apply(Command::Disconnect, &mut vec![]).unwrap();
    dev2.tick(&mut vec![]);
    replay(&mut server, &dev2.segment(), Limits::default()).unwrap();
    assert_eq!(bytes(&server), bytes(&dev2.world));
}

#[test]
fn rejects_forged_and_malformed_logs() {
    let mut run = Run::new(3);
    play(&mut run);
    let good = run.segment();
    let fresh = || World::new(3);
    let l = Limits::default();

    // Replaying onto the wrong starting point.
    let mut shifted = good.clone();
    shifted.start_tick = 10;
    assert!(matches!(
        replay(&mut fresh(), &shifted, l),
        Err(ReplayError::StartTick { .. })
    ));

    // Wrong rules version.
    let mut old = good.clone();
    old.sim_version = 0;
    assert!(matches!(
        replay(&mut fresh(), &old, l),
        Err(ReplayError::SimVersion { .. })
    ));

    // Ticks going backwards.
    let mut unordered = good.clone();
    unordered.entries.push(CommandEntry {
        tick: 5,
        cmd: Command::Disconnect,
    });
    unordered.entries.push(CommandEntry {
        tick: 3,
        cmd: Command::Disconnect,
    });
    assert!(matches!(
        replay(&mut fresh(), &unordered, l),
        Err(ReplayError::Unordered { .. })
    ));

    // A command stamped after the segment ends.
    let mut late = good.clone();
    late.entries.push(CommandEntry {
        tick: good.end_tick + 1,
        cmd: Command::Disconnect,
    });
    assert!(matches!(
        replay(&mut fresh(), &late, l),
        Err(ReplayError::OutOfRange { .. })
    ));

    // A command that was never legal: buying with no money.
    let mut broke = good.clone();
    broke.entries.insert(
        0,
        CommandEntry {
            tick: 0,
            cmd: Command::BuyTool(Tool::Decrypter),
        },
    );
    broke.entries.insert(
        1,
        CommandEntry {
            tick: 0,
            cmd: Command::BuyTool(Tool::Decrypter),
        },
    );
    assert!(matches!(
        replay(&mut fresh(), &broke, l),
        Err(ReplayError::Rejected { .. })
    ));

    // Duplicate-injected command.
    let mut dup = good.clone();
    let connect = dup
        .entries
        .iter()
        .find(|e| matches!(e.cmd, Command::Connect { .. }))
        .unwrap()
        .clone();
    dup.entries.push(CommandEntry {
        tick: good.end_tick,
        cmd: connect.cmd,
    });
    assert_eq!(
        replay(&mut fresh(), &dup, l),
        Err(ReplayError::Rejected {
            index: dup.entries.len() - 1,
            error: CmdError::AlreadyConnected
        })
    );

    // Resource limits.
    let mut long = good.clone();
    long.end_tick = 10_000_000;
    assert_eq!(replay(&mut fresh(), &long, l), Err(ReplayError::TooLong));
    let tight = Limits {
        max_ticks: 1000,
        max_entries: 1,
    };
    assert_eq!(
        replay(&mut fresh(), &good, tight),
        Err(ReplayError::TooManyEntries)
    );
}

/// Hostile logs must never panic: the server replays untrusted input.
#[test]
fn random_hostile_commands_never_panic() {
    let mut rng = Rng::new(0xC0FFEE);
    for round in 0..200 {
        let mut w = World::new(round);
        let real: Vec<ServerId> = w.servers.keys().collect();
        let mut ev = vec![];
        for _ in 0..300 {
            let pick = |rng: &mut Rng| -> ServerId {
                if rng.range(0, 3) == 0 {
                    ServerId::from(KeyData::from_ffi(rng.next_u64()))
                } else {
                    real[rng.index(real.len())]
                }
            };
            let tool = [
                Tool::PasswordBreaker,
                Tool::FirewallBypass,
                Tool::ProxyBypass,
                Tool::Decrypter,
                Tool::LogDeleter,
                Tool::FileCopier,
            ][rng.index(6)];
            let cmd = match rng.range(0, 7) {
                0 => Command::Connect {
                    chain: (0..rng.range(0, 10)).map(|_| pick(&mut rng)).collect(),
                    target: pick(&mut rng),
                },
                1 => Command::Disconnect,
                2 | 3 => Command::Run {
                    tool,
                    server: pick(&mut rng),
                    file: if rng.range(0, 1) == 0 {
                        None
                    } else {
                        Some(rng.range(0, u32::MAX) as usize)
                    },
                },
                4 => Command::Cancel(TaskId::from(KeyData::from_ffi(rng.next_u64()))),
                5 => Command::AcceptMission(rng.range(0, 6)),
                _ => Command::BuyTool(tool),
            };
            let _ = w.apply(cmd, &mut ev);
            for _ in 0..rng.range(0, 40) {
                ev.clear();
                w.tick(&mut ev);
            }
        }
    }
}
