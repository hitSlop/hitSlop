# @hitslop/shell

Private host-owned page runtime: immutable snapshots, handles, text bindings, ordered
publications, barriers, attachments, themes and capture. Depends on the author SDK
and the TypeBox platform contracts. The native app and CLI ship its built assets;
authored apps and the published document SDK do not contain the shell.

`bun run build` builds the shell for both consumers. `bun run test` includes the
shell tests over the WASM core; Swift integration covers the native protocol.

The runtime imports contract types and plain constants, never TypeBox: Rust validates
incoming requests, and the build fails when the shell exceeds its 64 KB budget. Boot
owns the `__slop` entry point and installs publication delivery before opening the
document.
