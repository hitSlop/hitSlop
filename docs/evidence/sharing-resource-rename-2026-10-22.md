# Sharing resource rename — 2026-10-22

Production resources now use `hitslop-sharing-relay`, `hitslop-sharing-host` and the
private R2 bucket `hitslop-sharing`. Public domains remain `share.hitslop.com` and
`*.dotslop.com`. No document marker or v1 protocol change was made.

The relay was fenced with a temporary maintenance worker while files were copied.
Room WebSockets were closed with retry code 1013; cleanup alarms were deferred.
The `sharing-rename-v1` transfer preserved the existing SQLite Durable Object namespaces:

- Rooms: `ddce11b8ec3647f59d5c1227d100c666`
- Accounts: `0015d58e6dc64937a0a2e2a98dfced04`

All six R2 objects were copied with their original keys and metadata, downloaded from
the destination, and compared by SHA-256. Before retirement, namespace ownership,
public domain ownership, wildcard routing and every destination hash were checked again.
The old workers and bucket were then retired. A private local file backup and migration
records remain under `.hitslop/sharing-rename/`; no credentials are included here.
Automatic deployments were paused for cutover. They remain paused until the new
configuration lands on the default branch; then re-enable `sharing.yml`. This prevents
the old default-branch configuration from recreating the retired resources.

Public health and Apple App Site Association checks passed. Local sharing qualification
passed verified identities, creation, join, recovery, Stop, account deletion, quotas,
cleanup and socket revocation. Existing local invitation fixtures used an older link
encoding and could not qualify a live room with the current protocol; namespace identity
and file hashes are the data-preservation evidence, not an end-to-end old-client proof.

Historical source names remain only in the applied transfer migration and historical
records. Keep the applied migration in Wrangler configuration.
