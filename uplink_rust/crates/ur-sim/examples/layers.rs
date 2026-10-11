// Prints the defences of every machine in a seed's world. Handy for picking a test target.
fn main() {
    let seed: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(42);
    let w = ur_sim::World::new(seed);
    let mut v: Vec<_> = w
        .servers
        .values()
        .filter(|s| s.kind == ur_sim::ServerKind::Corp)
        .collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    for s in v {
        println!(
            "{}  {:?}",
            s.name,
            s.layers
                .iter()
                .map(|l| (l.kind, l.strength))
                .collect::<Vec<_>>()
        );
    }
}
