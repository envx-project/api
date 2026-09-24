# Version 1 project invitations

`POST /v2/project/{id}/snapshot` returns all encrypted rows, members, and a SHA-256 snapshot token. Optional `add_user_ids` includes the proposed recipients' public keys in that token. Rows and users are sorted before hashing. The token covers project ID, variable IDs/content/metadata, current membership, and the complete recipient set.

`POST /v2/invite/new` requires `protocol_version: 1`, the snapshot token, project ID, and a client-encrypted invitation payload. It saves the token only if the snapshot is still current. `POST /v2/invite/prepare` checks the verifier, expiry, unclaimed status, author's membership, and current source snapshot, then returns the payload and a token that also binds the invitee's key. Preparation does not claim the invite or grant membership.

`POST /v2/invite/accept` requires protocol version, code, verifier, the prepared token, and every `{id, value}` rewrapped row exactly once. In one transaction it checks the source and prepared snapshots, updates all values, claims the invitation, clears its payload, and adds membership. Any failure rolls everything back. Empty projects are valid. `POST /v2/project/{id}/rewrap` provides the same conditional complete rewrap and membership transaction for direct add-users.

All membership and variable writers acquire the project row lock before variable/membership locks. Cross-project update batches acquire sorted project IDs first. Creating invitations also serializes each author's quota after acquiring the project lock. Payload limits are 1 MiB each, 32 active invitations per author, and 16 MiB active ciphertext per author. Invites expire after one hour; new invitations clear that author's expired payloads.

Legacy create/accept/add-user requests fail with `invite_upgrade_required`; existing memberships remain intact. Old invitations must be regenerated. Stale snapshots return HTTP 409 with code `project_snapshot_stale`. Flat membership permissions are unchanged: every current member can write and manage membership. Server-side validation treats rewrapped ciphertext as opaque, as ordinary authorized writes already do.

## Remaining ordinary-write concurrency contract

Ordinary variable endpoints retain their existing API and do not require a recipient snapshot. This patch protects invitation/add-users rewraps, not all future writes. An exact reproduction of the remaining limitation is:

1. Existing member A fetches a variable and the project's public keys, and encrypts a replacement for the current recipients.
2. Before A submits it, B successfully accepts a version 1 invitation. The server atomically rewraps the variables and adds B.
3. A submits the previously prepared replacement through `/variables/update-many` or `/v2/variables/update-many` with the same variable/project IDs.
4. The existing API accepts A's authorized write. The replacement lacks B's recipient key even though B is now a member.

A complete remedy requires snapshot preconditions on every variable write plus migration of ordinary clients; rejecting older variable clients would change the compatibility contract. The invitation transaction does not claim to prevent later authorized writes from replacing its result. The same broader issue affects separately prepared removal rewraps.

Regression coverage lives in `src/routes/v2/invite/accept/tests.rs` and `src/routes/v2/project/snapshot.rs`. It covers expired/legacy/removed-author failures, changed/added/deleted/replaced rows, membership changes, incomplete/duplicate/failed rewraps, empty projects, concurrent redemption, waiting for a project writer, stale create, storage quotas, expiry cleanup, and conditional direct add-users. Run with a disposable `DATABASE_URL` via `cargo test`.
