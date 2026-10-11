CREATE TABLE accounts (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    -- SHA-256 of the bearer token; the token itself is never stored.
    token_hash  bytea NOT NULL UNIQUE,
    created_at  timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE saves (
    account_id  uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    slot        text NOT NULL CHECK (slot ~ '^[a-z0-9_-]{1,32}$'),
    -- Compare-and-swap counter. Starts at 1 on first upload.
    revision    bigint NOT NULL CHECK (revision >= 1),
    -- Client-side encrypted blob (see gl-save). The server cannot read it.
    blob        bytea NOT NULL,
    updated_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (account_id, slot)
);
