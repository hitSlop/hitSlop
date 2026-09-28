# hitSlop website and docs

The Astro landing page and Starlight documentation deploy together as the
`hitslop-landing` Cloudflare Worker. The landing page is served at
`https://hitslop.com/` and the documentation at `https://hitslop.com/docs/`.

## Local development

From the repository root:

```sh
bun install --cwd apps/landing --frozen-lockfile
bun run --cwd apps/landing dev
```

Run the production checks with:

```sh
bun run --cwd apps/landing check
bun run --cwd apps/landing build
```

The template wall reads committed data generated from the real templates. After
templates change, run `bun apps/landing/scripts/templates.ts` from the repository root.
Artwork, fonts, the share image and data are described in
[src/components/Landing.assets.md](src/components/Landing.assets.md).

The build uses Astro directly. Review edited documentation links in the local preview;
there is no custom link checker.

## Cloudflare Workers Builds

Use these configuration settings when connecting the `hitSlop/hitslop` repository
to the `hitslop-landing` Worker. Confirm the actual dashboard settings before deployment:

| Setting | Value |
| --- | --- |
| Production branch | `master` |
| Root directory | `/` |
| Build command | `bun install --cwd apps/landing --frozen-lockfile && bun run --cwd apps/landing check && bun run --cwd apps/landing build` |
| Deploy command | `cd apps/landing && npx --no-install wrangler deploy` |
| Build variable | `BUN_VERSION=1.4.2` |

The landing app has its own lockfile and needs its own dependency install.
Automatic production and branch preview deployment depend on the Worker dashboard
configuration; these settings are not verified by a local build. For branch previews, use
`bun run --cwd apps/landing preview:upload` as the preview deploy command.

For a manual production deploy, run:

```sh
bun run --cwd apps/landing deploy
```
