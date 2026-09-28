# Native candidate coverage

This inventory is deliberately stricter than "the demo opens." The adapter reads
prepared snapshots and implements enough semantics to compare placement. It is not
a complete port of `packages/document`.

| Capability | Implemented/proved here | Remaining before production |
|---|---|---|
| Plain text | UTF-16 splices, Unicode replacement, local draft queue, exact sustained typing result | Remote draft ancestry, composition/selection, remote deletion, real keyboard tests |
| Rich text | Root formatting delta projection and mark operation | General nested rich-text projection, all style expansion policies, concurrent formatting tests |
| Scalar registers | Typed operations; string/boolean/number/integer/enum validation | Complete constraint issue parity and arbitrary anomalous scalar projections |
| Objects | Descendant writes; refusal to assign over present composites | Native descriptor-based creation and correct mergeable composite initialization |
| Object lists | Valid IDs, indexed lookup, insertion, deletion and moves; identity survives reopen | Derived IDs and duplicate ownership; stable-ID structural intent based on stale frames |
| Scalar lists | Basic projection, insert/remove/move | Indexed scalar writes and complete conformance |
| Records / optionals | Entry writes/clear and absent-field assignment | Full initialization, merge and invalid-value matrix |
| Counters | Finite increments and exact local checkpoint/reopen owner test | Upstream replay divergence; overflow/constraint issue parity; shared counter policy |
| Trees | Empty tree in mixed seed can be projected | Nonempty tree projection, identity and editing |
| Atomic batches | Late rejection leaves state and version unchanged; sensitivity demonstrated | Complete operation surface, optimized staging and integrated undo |
| Local persistence | Incremental writes, capacity, writer lock, failed save/close, retry and ambiguous commit recovery | Full native UI error/retry flow, process-death/power-loss integration, attachment parity |
| Remote imports | In-memory reorder/duplicate convergence; native/WASM durable-log component replay | Durable client outbox/receive cursor recovery, persisted missing-dependency buffers, room binding, discard rules, full transport integration |
| Renderer independence | Core tests and replay use native documents without creating WebViews | Live renderer destruction/remount and WebContent-crash tests |
| Author SDK | Typed scalar/text/counter intents, asynchronous acceptance, callback failure, flush ordering | Complete collection APIs, subscriptions, request deduplication/unknown-outcome handling |
| Platform contracts | TypeBox-generated operation union, Swift JSONValue, typed request fields | Full generated request/reply envelope and runtime validation parity |
| Host isolation | Local harness with bounded JSON requests and loopback-only transport diagnostic | Remove harness storage verbs from authored content; capability-scoped commands, production package validation and credential isolation |

`audit.ts` compares identical seed bytes against the current document projection and
returns exit code 2 while capability gaps remain, even when its small set of probes
passes. The current runtime's broader contract tests remain the required specification.
The performance matrix only supports conclusions for the measured checklist workloads.
The harness's raw storage verbs exist to feed the WASM control. Its scheme handler
and storage opener are test infrastructure, not replacements for production package
isolation or path validation. The native session also lacks a complete closed/error
state machine and native retry UI; successful focused close tests do not establish
all lifecycle behavior.
