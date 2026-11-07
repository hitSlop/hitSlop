# pop transition

dotpop is the product that grew out of hitSlop. The CLI is `pop`; the combined SDK/CLI package is `@hitslop/pop`; documents use `.pop`; the repository is https://github.com/hitSlop/dotpop and release tags are `pop-vX.Y.Z`. It starts document lineage 2 and ships as 2.0.0 on top of the existing Mac and iPhone apps.

## What is not carried over

Nothing written before lineage 2 is supported. hitSlop `.slop` files, experimental `.tiny` files, earlier `.pop` files and the old authoring APIs are refused at open ("made by an older version"), never converted and never modified; see [compatibility](../engineering-contract.md#compatibility). The earlier product's cloud data is not read or migrated: the Worker (`dotpop`), its R2 bucket, analytics dataset and Durable Object migration tag use fresh names, so old and new infrastructure never share state.

## What stays the same

The Apple bundle IDs (`com.hitslop.app` and its extensions), the Firebase project, the Apple Services ID (`com.hitslop.app.signin`) and `dotslop.com`, the isolated browser rendering domain.

## Updates

The Mac app updates from `dotpop-appcast.xml` on the latest GitHub release. Every release also carries a valid, empty `appcast.xml` for installations that still request the old filename; it never contains an `<item>`. Packaging, retained candidate validation and publication verify both feeds. Release steps are in [releasing](releasing.md).
