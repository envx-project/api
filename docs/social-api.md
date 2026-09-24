# Friends and encrypted handoffs

All routes are authenticated under `/v2`. Stable UUIDs identify users; usernames
are display text, not unique addresses. Public-key fingerprints are lowercase hex.
The server treats message bodies as opaque ciphertext. Clients must sign and
encrypt an envelope binding the message ID, both identities and fingerprints,
expiry, and payload, and verify the envelope before displaying or importing.

## Friends and links

- `GET /friends`: `{user: Identity, created_at, receipts: string[]}[]`.
- `DELETE /friends/{user_id}`: remove the mutual relationship, returning 204.
  Existing mailbox copies remain accessible. New sends are forbidden.
- `POST /friend-links`: `{label, target_id?, expires_at?}` returns
  `{link: Link, token: string}`. The 256-bit hex token is returned only here; only
  its SHA-256 digest is stored. Label is 1–64 lowercase ASCII letters/hyphens and
  has no security meaning. Expiry defaults to 24 hours, maximum 30 days.
- `GET /friend-links`: creator-only history, `Link[]`, including redeemed-by
  identity and signed receipt. This endpoint never returns the token or hash.
- `DELETE /friend-links/{id}`: creator-only revocation of an unused link, 204.
- `POST /friend-links/preview`: `{id, token}` returns `{link, creator: Identity}`
  without consuming the link. Wrong targets and bad tokens return 404.
- `POST /friend-links/redeem`: `{id, token, receipt}` returns the same shape as
  preview. `receipt` is an uncompressed armored OpenPGP signed message containing
  this JSON object (field order is irrelevant):

```json
{"version":1,"id":"link UUID","creator_id":"UUID","creator_fingerprint":"hex","redeemer_id":"UUID","redeemer_fingerprint":"hex"}
```

The server verifies the receipt using the redeemer's registered key and compares
all fields before committing friendship and consumption in one transaction.
Self-friending is rejected. Existing friendships make redemption a no-op for the
relationship but still consume the link. Concurrent redeemers produce one winner.
The original redeemer can recover a response after expiry; recovery does not
restore a friendship subsequently removed. Clients must verify receipts before
pinning a creator's new friend. First-use identity still depends on the link's
out-of-band fingerprint and the authenticated server directory.

`Identity` is `{id, username, public_key, fingerprint}`.
`Link` is `{id, creator_id, target_id, label, created_at, expires_at, revoked_at,
redeemed_at, redeemed_by, receipt}`; optional fields are JSON null. `redeemed_by`
is an `Identity`. Timestamps are RFC3339.

## Messages

- `POST /messages`: `{id, recipient_id, ciphertext, expires_at?}`. `id` is a
  client-generated durable UUID. An identical retry returns the original
  metadata, including after deletion or expiry; it never restores ciphertext.
  Reusing the ID with different content, recipient, or expiry returns 409.
- `GET /messages`: both incoming and outgoing visible messages, metadata only.
- `GET /messages/{id}`: participant-only metadata plus ciphertext.
- `DELETE /messages/{id}`: delete the caller's mailbox copy, 204. Once both copies
  are deleted, ciphertext and public-key snapshots are erased; minimal retry state remains.

Message fields: `{id, sender_id, recipient_id, created_at, expires_at,
sender_public_key, recipient_public_key, ciphertext}`. Send and list responses
set ciphertext to null and public-key fields to empty strings. Full keys are
returned only by the message detail endpoint. Keys are verified and canonically
armored at send time, discarding arbitrary armor headers. Legacy stored keys
larger than 1 MiB and canonical keys larger than 128 KiB are rejected when
sending; this intentionally bounds previously uncapped registrations. Never trust a server
snapshot in place of a locally pinned fingerprint.

Expiry makes both copies inaccessible immediately. A bounded cleanup during
subsequent sends erases up to 1,000 expired ciphertexts and their key snapshots. This is request-driven
cleanup, not a promise of physical erasure at the expiry instant; backups may
also retain ciphertext. Database maintenance can clear the remaining expired
ciphertext independently. The message ID and digest remain tombstones so retries
cannot resurrect messages.

All three lists accept `limit` (1–100, default 50) and `before` (UUID). Messages and links sort
newest first, with UUID descending breaking creation-time ties. Pass the last
item's ID as the next cursor; cursors must belong to the caller. The friends list
sorts by friend UUID descending and uses the friend user ID as its cursor.

## Limits and concurrency

- 100 active links and 100 new links per account per UTC day.
- 1,000 preview/redemption attempts per account per UTC day, including failures.
- 1,000 mutual friends per account.
- 128 KiB ciphertext per message; 1,000 sends per account per UTC day.
- 1,000 live messages and 20 MiB of ciphertext plus both canonical key snapshots
  per participant's mailbox.

Daily counters are independent of message deletion. User rows are locked in UUID
order before friendship mutations or quota-sensitive writes. PostgreSQL tests
cover claim races, retry recovery, target/signature/token rejection, revocation,
expiry, authorization, deletion, pagination, send-rate and mailbox-capacity races, creation-time pagination ties, unrelated cursors, and
legacy padded-key canonicalization/accounting/cleanup.
Run them only against a disposable PostgreSQL instance; `sqlx::test` creates
isolated test databases and applies migrations automatically.
