-- Only opaque Noise messages are stored; no identity keys or transfer passwords.
CREATE TABLE auth_pairings (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    client_network cidr NOT NULL,
    expires_at timestamptz NOT NULL DEFAULT (clock_timestamp() + interval '10 minutes'),
    phase smallint NOT NULL DEFAULT 0 CHECK (phase BETWEEN 0 AND 4),
    receiver_hash text,
    message text CHECK (octet_length(message) <= 131070)
);
CREATE INDEX auth_pairings_expiry ON auth_pairings(expires_at);
CREATE INDEX auth_pairings_owner ON auth_pairings(owner_id);
CREATE INDEX auth_pairings_client ON auth_pairings(client_network);
