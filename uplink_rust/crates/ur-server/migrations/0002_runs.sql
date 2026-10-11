-- Server-issued seeds: a run can only start from a seed the server handed out,
-- so players can't pick worlds with easy missions. Seeds are u64 stored as bigint (bit-cast).
CREATE TABLE seeds (
    account_id  uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    seed        bigint NOT NULL,
    issued_at   timestamptz NOT NULL DEFAULT now(),
    used        boolean NOT NULL DEFAULT false,
    PRIMARY KEY (account_id, seed)
);

-- Verified runs. `state` is the world the SERVER computed by replaying client command logs,
-- sealed with the server key. It is never taken from the client.
CREATE TABLE runs (
    account_id     uuid NOT NULL,
    seed           bigint NOT NULL,
    sim_version    integer NOT NULL,
    verified_tick  bigint NOT NULL CHECK (verified_tick >= 0),
    revision       bigint NOT NULL CHECK (revision >= 1),
    state          bytea NOT NULL,
    credits        bigint NOT NULL,
    rating         integer NOT NULL,
    heat           smallint NOT NULL,
    game_over      boolean NOT NULL,
    started_at     timestamptz NOT NULL DEFAULT now(),
    updated_at     timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (account_id, seed),
    FOREIGN KEY (account_id, seed) REFERENCES seeds(account_id, seed) ON DELETE CASCADE
);
