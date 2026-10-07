# Rust media registry

2026-10-06. Resource foundation for the packaging migration; no commit.

`hitslop-core/src/media.rs` owns asset extensions, canonical extensions, MIME types
and compression choices. The Rust exporter writes `wire/media.generated.ts`, used by
the new Vite compiler. The existing Rust asset reader uses the same registry while
the production layout still uses its old asset paths.

The passive attachment recognizer checks image, audio, video and PDF signatures.
It distinguishes ISO BMFF brands, verifies a bounded WebM document type, and serves
unknown bytes and active formats as `application/octet-stream`. Text assets require
UTF-8; binary app assets must match their declared media type. This is signature
recognition, not a replacement for the skin/artwork PNG decoder.

The new key checker permits fixed program/style keys and canonical content-addressed
media keys. Aliases, uppercase hashes and traversal are refused. Production attachment
columns and URL serving still need the storage switch; this does not claim they have
landed.

Verification: 13 focused app/media tests passed. `bun run verify` passed in 534.8
seconds: 251 Rust tests (4 skipped), 152 Bun tests, four installed-package checks
(1 skipped), and static checks. An initial clippy boolean-simplification failure was
fixed before that successful run.
