@AGENTS.md

**Do not create an automated test or test suite for each slop.** Creating or changing
a slop calls for existing checks/builds and hands-on preview/export review, not a new
example-specific test. Put shared SDK, storage, or host regression coverage at its
owning boundary. Follow the Testing rules in AGENTS.md.
Ordinary tests use minimal infrastructure fixtures, not live examples; generic
shipped-artifact smoke checks and frozen compatibility replay remain separate.
