-- Agents: a named, server-issued seed. An account holds many agents (the login screen's roster);
-- each agent's verified progress is its row in `runs`. Retiring an agent deletes its seed, which
-- cascades to the run.
ALTER TABLE seeds ADD COLUMN agent_name text;
CREATE UNIQUE INDEX seeds_agent_name ON seeds (account_id, agent_name) WHERE agent_name IS NOT NULL;
