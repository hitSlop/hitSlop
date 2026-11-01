# pop transition

dotpop is a new product lineage, starting at 1.0.0 in https://github.com/hitSlop/dotpop. The CLI is `pop`; the combined SDK/CLI package is `@hitslop/pop`. Documents use `.pop`. Legacy `.slop` files and experimental `.tiny` files and authoring APIs are unsupported; existing files and cloud records must not be deleted or silently converted.

## Retained identities

Apple bundle IDs, the CloudKit `SlopDocument` record type and `Library` zone, widget kind, iCloud containers, Keychain/app groups, Firebase project and Apple Services ID remain unchanged. The stateful Cloudflare relay/host Worker names, Durable Object class identities/migration history, R2 buckets and analytics datasets retain their deployed names. The new stateless `dotpop-landing` Worker serves dotpop.click separately, leaving the original landing Worker intact. `dotslop.com` remains the isolated browser rendering domain. These names are intentional exceptions to the source rename.

Historical hitSlop protocol fixtures, release evidence, command prelude and `scripts/compat/required-history.json` remain unchanged. Active compatibility discovery reads only `pop-vX.Y.Z` releases in the renamed original repository and `scripts/compat/pop-required-history.json`. Pin the first pop corpus hash permanently after the first publication; every release still captures and qualifies its own corpus.

## Before publication

- Merge the rename PR into the original `hitSlop/hitSlop` repository, then rename that repository to `pop`. Preserve its history, tags, releases, secrets and settings. Use `pop-vX.Y.Z` tags for the new lineage; the app and npm versions remain `X.Y.Z`.
- Retain the existing CI/rulesets, protected release environment and signing, App Store, Sparkle and Cloudflare secrets. Verify tag protection covers `pop-v*` and the Cloudflare token covers `dotpop.click`. Keep `POP_DEPLOY_ENABLED` disabled until rollout qualification is complete.
- Verify publishing rights for `@hitslop/pop` and configure npm trusted publishing for the new repository.
- Add `share.dotpop.click` and its `/auth/apple` return URL to the existing Apple Services ID and Firebase authorized domains. Preserve the existing Apple primary-app grouping.
- Qualify dotpop.click, share.dotpop.click, wildcard sandbox routing, AASA, sign-in, create/join/stop, and browser downloads.
- Update the App Store Connect/TestFlight and Apple sign-in display names to pop while retaining existing identifiers. Confirm App Store Connect accepts marketing version 1.0.0; native build numbers must continue increasing.
- Capture the first pop corpus, complete release checks and review the signed candidate before setting `POP_DEPLOY_ENABLED=true`. Website and sharing deployment jobs remain gated until then.

Do not overwrite or delete original hitSlop tags, releases, corpora or hash pins. The renamed app uses its new update feed; transitioning old hitSlop installations through their existing updater is a separate rollout action.
