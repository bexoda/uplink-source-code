use ur_sim::*;

fn target(w: &mut World, layers: &[(LayerKind, u32)], trace_ticks: u32) -> ServerId {
    w.servers.insert(Server {
        name: "corp-test".into(),
        ip: "10.0.0.1".into(),
        kind: ServerKind::Corp,
        layers: layers
            .iter()
            .map(|&(kind, strength)| Layer {
                kind,
                strength,
                defeated: false,
            })
            .collect(),
        files: vec![FileEntry {
            name: "secret.dat".into(),
            size: 100,
            value: 500,
        }],
        logs: vec![],
        trace_ticks,
    })
}

fn relay(w: &mut World, trace_ticks: u32) -> ServerId {
    w.servers.insert(Server {
        name: "relay".into(),
        ip: "10.0.0.2".into(),
        kind: ServerKind::Relay,
        layers: vec![],
        files: vec![],
        logs: vec![],
        trace_ticks,
    })
}

fn run_until(
    w: &mut World,
    ev: &mut Vec<Event>,
    max: u32,
    mut pred: impl FnMut(&Event) -> bool,
) -> bool {
    for _ in 0..max {
        ev.clear();
        w.tick(ev);
        if ev.iter().any(&mut pred) {
            return true;
        }
    }
    false
}

#[test]
fn full_heist_completes_mission_and_pays() {
    let mut w = World::empty(1);
    let t = target(&mut w, &[(LayerKind::Password, 80)], 1000);
    let r = relay(&mut w, 1000);
    w.missions.push(Mission {
        id: 1,
        title: "steal".into(),
        kind: MissionKind::StealFile {
            server: t,
            file: "secret.dat".into(),
        },
        reward: 1000,
        accepted: false,
        done: false,
    });
    let mut ev = vec![];
    let start = w.player.credits;
    w.apply(Command::AcceptMission(1), &mut ev).unwrap();
    w.apply(
        Command::Connect {
            chain: vec![r],
            target: t,
        },
        &mut ev,
    )
    .unwrap();
    w.apply(
        Command::Run {
            tool: Tool::PasswordBreaker,
            server: t,
            file: None,
        },
        &mut ev,
    )
    .unwrap();
    assert!(run_until(&mut w, &mut ev, 100, |e| matches!(
        e,
        Event::LayerDefeated { .. }
    )));
    w.apply(
        Command::Run {
            tool: Tool::FileCopier,
            server: t,
            file: Some(0),
        },
        &mut ev,
    )
    .unwrap();
    assert!(run_until(&mut w, &mut ev, 100, |e| matches!(
        e,
        Event::MissionComplete { reward: 1000, .. }
    )));
    assert_eq!(w.player.credits, start + 1000);
    assert_eq!(w.player.rating, 1);
    assert_eq!(w.player.gateway.files.len(), 1);
}

#[test]
fn trace_walks_every_hop_then_catches() {
    let mut w = World::empty(1);
    let t = target(&mut w, &[(LayerKind::Password, 1_000_000)], 10);
    let r = relay(&mut w, 5);
    let mut ev = vec![];
    w.apply(
        Command::Connect {
            chain: vec![r],
            target: t,
        },
        &mut ev,
    )
    .unwrap();
    w.apply(
        Command::Run {
            tool: Tool::PasswordBreaker,
            server: t,
            file: None,
        },
        &mut ev,
    )
    .unwrap();
    assert!(ev.contains(&Event::TraceStarted));
    let credits = w.player.credits;
    let mut seen = vec![];
    for _ in 0..100 {
        ev.clear();
        w.tick(&mut ev);
        seen.extend(ev.iter().cloned());
        if w.session.is_none() {
            break;
        }
    }
    assert!(seen.contains(&Event::TraceAdvanced { hop: 1 }));
    assert!(seen.iter().any(|e| matches!(e, Event::Caught { .. })));
    assert_eq!(w.player.heat, 1);
    assert!(w.player.credits < credits);
    assert!(w.tasks.is_empty());
    assert!(
        w.servers[t].layers.iter().all(|l| !l.defeated),
        "layers re-arm after disconnect"
    );
}

#[test]
fn deleting_log_before_trace_reaches_hop_loses_trace() {
    let mut w = World::empty(1);
    let t = target(&mut w, &[(LayerKind::Password, 40)], 200);
    let mut ev = vec![];
    w.apply(
        Command::Connect {
            chain: vec![],
            target: t,
        },
        &mut ev,
    )
    .unwrap();
    // Cannot erase target logs while locked.
    assert_eq!(
        w.apply(
            Command::Run {
                tool: Tool::LogDeleter,
                server: t,
                file: None
            },
            &mut ev
        ),
        Err(CmdError::Locked)
    );
    w.apply(
        Command::Run {
            tool: Tool::PasswordBreaker,
            server: t,
            file: None,
        },
        &mut ev,
    )
    .unwrap();
    assert!(run_until(&mut w, &mut ev, 100, |e| matches!(
        e,
        Event::LayerDefeated { .. }
    )));
    w.apply(
        Command::Run {
            tool: Tool::LogDeleter,
            server: t,
            file: None,
        },
        &mut ev,
    )
    .unwrap();
    assert!(run_until(&mut w, &mut ev, 100, |e| matches!(
        e,
        Event::LogErased { .. }
    )));
    assert!(run_until(&mut w, &mut ev, 300, |e| matches!(
        e,
        Event::TraceLost
    )));
    assert_eq!(w.player.heat, 0);
    assert!(w.session.is_some(), "still connected, just untraceable");
}

#[test]
fn tasks_share_cpu() {
    let mut w = World::empty(1);
    let t = target(&mut w, &[(LayerKind::Password, 1_000)], 10_000);
    let r = relay(&mut w, 10_000);
    let mut ev = vec![];
    w.apply(
        Command::Connect {
            chain: vec![r],
            target: t,
        },
        &mut ev,
    )
    .unwrap();
    w.apply(
        Command::Run {
            tool: Tool::PasswordBreaker,
            server: t,
            file: None,
        },
        &mut ev,
    )
    .unwrap();
    w.tick(&mut ev);
    let solo = w.tasks.values().next().unwrap().progress;
    w.apply(
        Command::Run {
            tool: Tool::LogDeleter,
            server: r,
            file: None,
        },
        &mut ev,
    )
    .unwrap();
    let before = w
        .tasks
        .values()
        .find(|t| t.tool == Tool::PasswordBreaker)
        .unwrap()
        .progress;
    w.tick(&mut ev);
    let after = w
        .tasks
        .values()
        .find(|t| t.tool == Tool::PasswordBreaker)
        .unwrap()
        .progress;
    assert_eq!(after - before, solo / 2);
}

#[test]
fn rejects_invalid_commands() {
    let mut w = World::new(7);
    let mut ev = vec![];
    let find = |w: &World, k: ServerKind| {
        w.servers
            .iter()
            .find(|(_, s)| s.kind == k)
            .map(|(i, _)| i)
            .unwrap()
    };
    let c = find(&w, ServerKind::Corp);
    let r = find(&w, ServerKind::Relay);
    assert_eq!(
        w.apply(Command::Disconnect, &mut ev),
        Err(CmdError::NotConnected)
    );
    assert_eq!(
        w.apply(
            Command::Connect {
                chain: vec![c],
                target: c
            },
            &mut ev
        ),
        Err(CmdError::BadChain)
    );
    assert_eq!(
        w.apply(
            Command::Connect {
                chain: vec![r, r],
                target: c
            },
            &mut ev
        ),
        Err(CmdError::BadChain)
    );
    assert_eq!(
        w.apply(
            Command::Run {
                tool: Tool::PasswordBreaker,
                server: c,
                file: None
            },
            &mut ev
        ),
        Err(CmdError::NotConnected)
    );
    assert_eq!(
        w.apply(Command::AcceptMission(99), &mut ev),
        Err(CmdError::NoSuchMission)
    );
    assert_eq!(w.apply(Command::BuyTool(Tool::Decrypter), &mut ev), Ok(()));
    assert_eq!(
        w.apply(Command::BuyTool(Tool::Decrypter), &mut ev),
        Err(CmdError::NotEnoughCredits)
    );
}

#[test]
fn three_strikes_ends_the_game() {
    let mut w = World::empty(1);
    let t = target(&mut w, &[(LayerKind::Password, 1_000_000)], 2);
    let mut ev = vec![];
    for _ in 0..3 {
        w.apply(
            Command::Connect {
                chain: vec![],
                target: t,
            },
            &mut ev,
        )
        .unwrap();
        w.apply(
            Command::Run {
                tool: Tool::PasswordBreaker,
                server: t,
                file: None,
            },
            &mut ev,
        )
        .unwrap();
        assert!(run_until(&mut w, &mut ev, 50, |e| matches!(
            e,
            Event::Caught { .. }
        )));
    }
    assert!(w.game_over);
    assert_eq!(
        w.apply(Command::Disconnect, &mut ev),
        Err(CmdError::GameOver)
    );
}

fn script(seed: u64) -> Vec<u8> {
    let mut w = World::new(seed);
    let mut ev = vec![];
    let relay_ids: Vec<_> = w
        .servers
        .iter()
        .filter(|(_, s)| s.kind == ServerKind::Relay)
        .map(|(i, _)| i)
        .collect();
    let corp = w
        .servers
        .iter()
        .find(|(_, s)| s.kind == ServerKind::Corp)
        .map(|(i, _)| i)
        .unwrap();
    w.apply(
        Command::Connect {
            chain: relay_ids,
            target: corp,
        },
        &mut ev,
    )
    .unwrap();
    w.apply(
        Command::Run {
            tool: Tool::PasswordBreaker,
            server: corp,
            file: None,
        },
        &mut ev,
    )
    .ok();
    for _ in 0..500 {
        w.tick(&mut ev);
    }
    postcard::to_allocvec(&w).unwrap()
}

#[test]
fn same_seed_same_bytes_different_seed_differs() {
    assert_eq!(script(42), script(42));
    assert_ne!(script(42), script(43));
}

#[test]
fn state_round_trips_through_postcard() {
    let w = World::new(5);
    let bytes = postcard::to_allocvec(&w).unwrap();
    let back: World = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(bytes, postcard::to_allocvec(&back).unwrap());
}

#[test]
fn contracts_are_never_duplicated() {
    for seed in 0..500 {
        let w = World::new(seed);
        assert_eq!(w.missions.len(), 3);
        let keys: Vec<_> = w.missions.iter().map(|m| m.title.clone()).collect();
        let mut dedup = keys.clone();
        dedup.sort();
        dedup.dedup();
        assert_eq!(dedup.len(), 3, "seed {seed}: {keys:?}");
    }
}
