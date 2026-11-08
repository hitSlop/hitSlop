# Foundation rebuild

The foundation rebuild was merged in PR #84 for pop 2.0.0, document lineage 2. It
removed earlier-product readers and adapters, CloudKit sync, sharing epochs, the second
Firebase app and the account document cap. It established one SQLite lineage check,
persistent cloud documents, fold-only compaction, one Firebase identity and shared
Rust contracts with event-driven transport. Historical published release evidence remains
unchanged.

This page is a historical summary, not an implementation or release checklist. Current
behavior belongs in [architecture](architecture.md), the [engineering contract](engineering-contract.md)
and [cloud documents](cloud-documents.md). Open work is tracked in the
[roadmap](roadmap.md); deployment, release qualification and retained compatibility
evidence are governed by [releasing](guides/releasing.md) and the
[sharing operator checklist](../apps/sharing/README.md#operator-checklist).
