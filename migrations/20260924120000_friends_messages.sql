-- Canonical pairs prevent reverse duplicates; all mutations lock users in UUID order.
CREATE TABLE friendships (
    user_low uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    user_high uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_low, user_high),
    CHECK (user_low < user_high)
);
CREATE TABLE friend_links (
    id uuid PRIMARY KEY,
    creator_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    target_id uuid REFERENCES users(id) ON DELETE CASCADE,
    label text NOT NULL CHECK (octet_length(label) BETWEEN 1 AND 64),
    token_hash text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,
    redeemed_at timestamptz,
    redeemed_by uuid REFERENCES users(id) ON DELETE CASCADE,
    receipt text,
    CHECK (target_id IS NULL OR target_id <> creator_id),
    CHECK ((redeemed_by IS NULL) = (redeemed_at IS NULL)),
    CHECK ((redeemed_by IS NULL) = (receipt IS NULL))
);
CREATE INDEX friend_links_creator ON friend_links(creator_id, created_at DESC);
CREATE TABLE secret_messages (
    id uuid PRIMARY KEY,
    sender_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    recipient_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    ciphertext text,
    ciphertext_hash text NOT NULL,
    sender_public_key text NOT NULL,
    recipient_public_key text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz,
    sender_deleted boolean NOT NULL DEFAULT false,
    recipient_deleted boolean NOT NULL DEFAULT false,
    CHECK(sender_id <> recipient_id),
    CHECK(ciphertext IS NULL OR octet_length(ciphertext) BETWEEN 1 AND 131072)
);
CREATE INDEX secret_messages_expiry ON secret_messages(expires_at) WHERE ciphertext IS NOT NULL AND expires_at IS NOT NULL;
CREATE INDEX secret_messages_sender ON secret_messages(sender_id, created_at DESC);
CREATE INDEX secret_messages_recipient ON secret_messages(recipient_id, created_at DESC);
-- Bounded daily counters survive message deletion and failed link claims.
CREATE TABLE social_daily_usage (
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    day date NOT NULL DEFAULT (now() AT TIME ZONE 'UTC')::date,
    kind text NOT NULL,
    count bigint NOT NULL,
    PRIMARY KEY (user_id, day, kind)
);
