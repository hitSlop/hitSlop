# tinys transition

tinys is a new product lineage, starting at 1.0.0 in https://github.com/hitSlop/tinys. The CLI is `tiny`; the combined SDK/CLI package is `@hitslop/tinys`. Documents use `.tiny`. Old `.slop` files and authoring APIs are unsupported; existing files and cloud records must not be deleted or silently converted.

## Retained identities

Apple bundle IDs, the CloudKit `SlopDocument` record type and `Library` zone, widget kind, iCloud containers, Keychain/app groups, Firebase project and Apple Services ID remain unchanged. Cloudflare Worker names, Durable Object class identities/migration history, R2 buckets and analytics datasets retain their deployed names. `dotslop.com` remains the isolated browser rendering domain. These names are intentional exceptions to the source rename.

Historical hitSlop protocol fixtures, release evidence, command prelude and `scripts/compat/required-history.json` remain unchanged. Active compatibility discovery reads the new repository and `scripts/compat/tinys-required-history.json`. Pin the first tinys corpus hash permanently after the first publication; every release still captures and qualifies its own corpus.

## Before publication

- Publish renamed source and Git history to the new repository without old tags. Set its default branch to master.
- Recreate CI/rulesets and the protected release environment, and reprovision the signing, App Store, Sparkle and Cloudflare secrets from their secure sources. Secret values cannot be recovered from GitHub.
- Verify publishing rights for `@hitslop/tinys` and configure npm trusted publishing for the new repository.
- Add `share.tinys.click` and its `/auth/apple` return URL to the existing Apple Services ID and Firebase authorized domains. Preserve the existing Apple primary-app grouping.
- Qualify tinys.click, share.tinys.click, wildcard sandbox routing, AASA, sign-in, create/join/stop, and browser downloads.
- Update the App Store Connect/TestFlight and Apple sign-in display names to tinys while retaining existing identifiers. Confirm App Store Connect accepts marketing version 1.0.0; native build numbers must continue increasing.
- Capture the first tinys corpus, complete release checks and review the signed candidate before setting `TINYS_DEPLOY_ENABLED=true`. Website and sharing deployment jobs remain gated until then.

Do not overwrite or delete original hitSlop tags, releases, corpora or hash pins. The renamed app uses its new update feed; transitioning old hitSlop installations through their existing updater is a separate rollout action.
