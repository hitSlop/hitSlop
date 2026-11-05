# pop transition

dotpop is a new product lineage, starting at 1.0.0 in https://github.com/hitSlop/dotpop. The CLI is `pop`; the combined SDK/CLI package is `@hitslop/pop`. Documents use `.pop`. Legacy `.slop` files and experimental `.tiny` files and authoring APIs are unsupported; existing files and cloud records must not be deleted or silently converted.

## Retained identities

Apple bundle IDs, widget kind, Keychain/app groups, Firebase project and Apple Services ID remain unchanged. These are intentional exceptions to the source rename. The active cloud service now uses persistent Cloudflare Document DOs/R2 and client-written Firestore references; see [cloud documents](../cloud-documents.md). CloudKit is removed from the client. Legacy CloudKit records and containers are not deleted or silently imported. Cloud-only documents require export with the previous app.

Deployed infrastructure identities and migration history are preserved where the current service uses them; retired relay Workers are listed in [deployment evidence](../evidence/cloud-documents-2026-11-04.md). The stateless `dotpop-landing` Worker serves dotpop.click separately. `dotslop.com` remains the isolated browser rendering domain.

Historical hitSlop protocol fixtures, release evidence, command prelude and `scripts/compat/required-history.json` remain unchanged. Active compatibility discovery reads only `pop-vX.Y.Z` releases in the renamed original repository and `scripts/compat/pop-required-history.json`. Pin the first pop corpus hash permanently after the first publication; every release still captures and qualifies its own corpus.

## Before publication

- Archive and verify all 55 original assets, release metadata and signed attestations from hitSlop `v1.0.0`, `v1.0.1`, `v1.1.0`, `v1.2.0` and `v1.3.0` outside Git. Check the original checksum manifests, GitHub asset digests and first corpus hash pin. Keep this archive when cleaning build caches.
- Merge the rename PR into the original `hitSlop/hitSlop` repository, then rename that repository to `dotpop`. Preserve its history, tags, secrets and settings; update the local origin to `https://github.com/hitSlop/dotpop.git`. Use `pop-vX.Y.Z` tags for the new lineage; the app and npm versions remain `X.Y.Z`.
- Retain the existing CI/rulesets, protected release environment and signing, App Store, Sparkle and Cloudflare secrets. Verify tag protection covers `pop-v*` and the Cloudflare token covers `dotpop.click`. Keep `POP_DEPLOY_ENABLED` disabled until rollout qualification is complete.
- Bootstrap `@hitslop/pop` with the metadata-only `0.0.0-bootstrap.0` version under the `bootstrap` tag. Configure npm trusted publishing for owner `hitSlop`, repository `dotpop`, workflow `macos-release.yml`, environment `release`, with direct publish and dist-tag permissions. It contains no runtime and creates no supported document format.
- Add `share.dotpop.click` and its `/auth/apple` return URL to the existing Apple Services ID and Firebase authorized domains. Preserve the existing Apple primary-app grouping.
- Qualify dotpop.click, share.dotpop.click, wildcard sandbox routing, AASA, sign-in, create/join/stop, and browser downloads.
- Update the App Store Connect/TestFlight and Apple sign-in display names to pop while retaining existing identifiers. Confirm App Store Connect accepts marketing version 1.0.0; native build numbers must continue increasing.
- For the current cloud cutover, preserve existing local files and legacy cloud records. Discover validated files only from known prior library directories; let the user opt into new sync. Download/export cloud-only files using the previous app. Pre-launch files remain unsupported; the released pop corpus is the acceptance baseline.
- Keep `POP_DEPLOY_ENABLED=false` while capturing the first pop corpus, completing release checks and testing the exact signed candidate DMG. Approve its protected `release` environment after acceptance; publication consumes those retained bytes.
- After `pop-v1.0.0` is published, verify npm integrity, downloads and both update feeds, and pin its original corpus hash. Then retire exactly the five archived legacy GitHub releases and their public assets. Keep their Git tags, the offline evidence, original hash pin, old npm versions and all user/cloud data. Public dotpop releases remain permanent compatibility commitments.
- Enable `POP_DEPLOY_ENABLED`, deploy sharing on the accepted `pop-v1.0.0` tag, then deploy the website from `master` to `dotpop.click`. Verify downloads, docs, sign-in and sharing on production.

The original repository keeps GitHub URL redirects after its rename. dotpop uses `latest/download/dotpop-appcast.xml`. Every dotpop release also carries a valid, empty `appcast.xml` for legacy hitSlop installations, which keep requesting that filename through the redirect. It must never contain an `<item>`: hitSlop receives no updates, while dotpop retains its own updater. Packaging, retained candidate validation and publication verify both feeds. Legacy `.slop` files are unsupported and are not migrated.
