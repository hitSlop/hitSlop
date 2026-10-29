# tinys transition

tinys is a new product lineage, starting at 1.0.0 in https://github.com/hitSlop/tinys. The CLI is `tiny`; the combined SDK/CLI package is `@hitslop/tinys`. Documents use `.tiny`. Old `.slop` files and authoring APIs are unsupported; existing files and cloud records must not be deleted or silently converted.

## Retained identities

Apple bundle IDs, the CloudKit `SlopDocument` record type and `Library` zone, widget kind, iCloud containers, Keychain/app groups, Firebase project and Apple Services ID remain unchanged. The stateful Cloudflare relay/host Worker names, Durable Object class identities/migration history, R2 buckets and analytics datasets retain their deployed names. The new stateless `tinys-landing` Worker serves tinys.click separately, leaving the original landing Worker intact. `dotslop.com` remains the isolated browser rendering domain. These names are intentional exceptions to the source rename.

Historical hitSlop protocol fixtures, release evidence, command prelude and `scripts/compat/required-history.json` remain unchanged. Active compatibility discovery reads only `tinys-vX.Y.Z` releases in the renamed original repository and `scripts/compat/tinys-required-history.json`. Pin the first tinys corpus hash permanently after the first publication; every release still captures and qualifies its own corpus.

## Before publication

- Merge the rename PR into the original `hitSlop/hitSlop` repository, then rename that repository to `tinys`. Preserve its history, tags, releases, secrets and settings. Use `tinys-vX.Y.Z` tags for the new lineage; the app and npm versions remain `X.Y.Z`.
- Retain the existing CI/rulesets, protected release environment and signing, App Store, Sparkle and Cloudflare secrets. Verify tag protection covers `tinys-v*` and the Cloudflare token covers `tinys.click`. Keep `TINYS_DEPLOY_ENABLED` disabled until rollout qualification is complete.
- Verify publishing rights for `@hitslop/tinys` and configure npm trusted publishing for the new repository.
- Add `share.tinys.click` and its `/auth/apple` return URL to the existing Apple Services ID and Firebase authorized domains. Preserve the existing Apple primary-app grouping.
- Qualify tinys.click, share.tinys.click, wildcard sandbox routing, AASA, sign-in, create/join/stop, and browser downloads.
- Update the App Store Connect/TestFlight and Apple sign-in display names to tinys while retaining existing identifiers. Confirm App Store Connect accepts marketing version 1.0.0; native build numbers must continue increasing.
- Capture the first tinys corpus, complete release checks and review the signed candidate before setting `TINYS_DEPLOY_ENABLED=true`. Website and sharing deployment jobs remain gated until then.

Do not overwrite or delete original hitSlop tags, releases, corpora or hash pins. The renamed app uses its new update feed; transitioning old hitSlop installations through their existing updater is a separate rollout action.
