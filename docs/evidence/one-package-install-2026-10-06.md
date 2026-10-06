# One package, installed (2026-10-06)

Spike B of [the versioning plan](../../plans/versioning.md#spike-b-one-package-installed-gates-steps-3-and-6).
Does a single `hitslop` package, holding the CLI, the SDK and the schema, work when installed
the way authors and agents install it? Run on the development M1 with Bun 1.4.2.

A staging script (`spikes/one-package/stage.ts`, local only) builds the package from today's
sources without moving the repo:

- `src/cli`, `src/sdk`, `src/schema` and `generated/`;
- the built shell, templates, skills, `.crust/root/skills`, and the darwin-arm64 engine;
- package imports rewritten to relative paths (25 sites);
- exports `.`, `./svelte`, `./embed` and `./package.json`.

It also adds two prototype rules: delegation in `src/cli/cli.ts`, and skills managed by a
project's own copy. Two versions were staged, `3.0.0-spike.1` and `3.0.0-spike.2`, at about
5.8 MB each. Installs used `file:` tarballs, and the global install went into a scratch
`BUN_INSTALL_GLOBAL_DIR`.

## Result

The package works. One defect, in how project skill links are made, has a confirmed fix.

| Check | Result |
| --- | --- |
| `slop check` in a project pinned to the package (svelte-check over the SDK's TypeScript and Svelte source in `node_modules`) | 0 errors, 0 warnings |
| `slop build`: Vite compiles `hitslop` and `hitslop/svelte` from `node_modules`, and the package's engine packs | `dist/proj.slop` written |
| `slop create` and `slop get` on the result | Document created; state read |
| `slop dev` | Server and preview pages answer 200. The app wasn't rendered in a browser |
| `Bun.resolveSync("hitslop/package.json", project)` with strict exports, hoisted and isolated linkers | Resolves |
| Global 3.0.0-spike.2 running `slop build <project pinned to spike.1>` from outside the project | Hands off to the project's spike.1 copy (plain project, hoisted workspace and isolated workspace) |
| A project's own copy | Runs without handing off (no recursion) |
| `slop skills install --all --scope project` from the project's copy | Links `.agents/skills`, plus `.claude/skills`, Crush, Kiro and Antigravity, which it found on `PATH` |
| Those links after `bun add -d` of another version, plain project | Follow it: `../../node_modules/hitslop/.crust/root/skills/<name>` |
| Those links after `bun add -d` of another version, **isolated linker** | **Stale.** Crust links the running copy's resolved path (`node_modules/.bun/hitslop@<hash>/…`). Bun keeps the old store directory, so the links still resolve, to the old guides |
| Crust's `installSkill` given the unresolved `<project>/node_modules/hitslop/.crust/root/skills/<name>` | Keeps it as `../../node_modules/hitslop/…`, and the links follow a version change under the isolated linker |

## What this changes in the plan

- **Step 6.** Project links are made by calling Crust's `installSkill` with the unresolved
  `node_modules/hitslop` path (scope `project`), not by the `skill()` extension's install,
  which uses the running copy's resolved path. `slop skills repair --scope project` fixes
  links made the other way, but nothing prompts anyone to run it.
- **Step 3.** The paths that depend on the CLI's depth are wider than listed:
  - `../package.json` imports in `app.ts` and `init.ts`;
  - the stage worker's path in `build.ts`;
  - the shell URLs in `core.ts`;
  - `skills-build.ts` and the skill extras in `app.ts`;
  - `cliRoot` in `paths.ts`.

  The import `entry.ts` writes into each app's generated entry stays a package import
  (`hitslop/svelte`). `build.ts` reads the ABI from the package's own `abi` module.
- **Delegation's pre-parser** takes the directory from the argument after the command. A
  flag placed before the directory (`slop dev --port 5174 ./app`) defeats it, so the real
  pre-parser must know which flags take values, or the directory must come first.

## Not checked

- Agents loading the linked skills in a live session (Claude Code, Codex). Linking agent
  folders is Crust's documented install route for these agents.
- An install from the npm registry rather than a `file:` tarball, and `bunx hitslop`.
- Linux, and a tarball carrying every platform's engine (the plan estimates about 11 MB
  compressed).
