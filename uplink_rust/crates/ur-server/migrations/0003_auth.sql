-- OAuth accounts replace the old "bearer token is the account" model.
ALTER TABLE accounts DROP COLUMN token_hash;
-- Public in-game name. Unique case-insensitively; deliberately not taken from any provider.
ALTER TABLE accounts ADD COLUMN handle text CHECK (handle ~ '^[A-Za-z0-9_]{3,20}$');
CREATE UNIQUE INDEX accounts_handle_ci ON accounts (lower(handle));

-- One row per linked login. Keyed on the provider's stable subject id, never on email.
CREATE TABLE identities (
    provider    text NOT NULL,
    subject     text NOT NULL,
    account_id  uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    email       text,
    created_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (provider, subject)
);
-- At most one identity per provider per account, so "unlink google" is unambiguous.
CREATE UNIQUE INDEX identities_account_provider ON identities (account_id, provider);

-- In-flight provider logins. Single use, 10 minute lifetime. Only the hash of `state` is stored.
CREATE TABLE oauth_states (
    state_hash        bytea PRIMARY KEY,
    provider          text NOT NULL,
    pkce_verifier     text NOT NULL,
    return_to         text NOT NULL,
    client_challenge  text NOT NULL,
    -- Set when a logged-in user is linking another provider (bound to their session, not a URL param).
    link_account      uuid,
    created_at        timestamptz NOT NULL DEFAULT now()
);

-- One-time code handed to the client after a successful login. Single use, 60 s lifetime,
-- redeemable only with the secret whose hash is `client_challenge`.
CREATE TABLE login_codes (
    code_hash         bytea PRIMARY KEY,
    account_id        uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    client_challenge  text NOT NULL,
    created_at        timestamptz NOT NULL DEFAULT now()
);

-- Opaque access + rotating refresh tokens. Only hashes are stored.
CREATE TABLE sessions (
    id                 uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id         uuid NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    access_hash        bytea NOT NULL UNIQUE,
    access_expires     timestamptz NOT NULL,
    refresh_hash       bytea NOT NULL UNIQUE,
    -- Previous refresh token, kept so reuse of a stolen/old one can be detected and the session killed.
    prev_refresh_hash  bytea,
    refresh_expires    timestamptz NOT NULL,
    created_at         timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX sessions_prev_refresh ON sessions (prev_refresh_hash);
CREATE INDEX sessions_account ON sessions (account_id);
