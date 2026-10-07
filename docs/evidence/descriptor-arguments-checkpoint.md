# Descriptor arguments checkpoint

2026-10-06. Implementation step 3 foundation, staged without committing.

`hitslop-core::arguments::Arguments` accepts the command subset of document
descriptors. It delegates values to `Node::validate`; it does not implement another
value checker. Text, counters, records and object-row lists are refused recursively.
The original descriptor retains descriptions. Errors carry an escaped JSON pointer
for the owner to use in command refusals.

Its JSON Schema method is output only. Six tests compare the projection with the
descriptor checker, using a test dependency as the oracle. Cases include closed
nested objects, absent optionals, null, numeric edges, integral float spellings,
enumerations, code-point string bounds and scalar lists. The projection includes
implicit safe-integer and list-length limits; omitting these would advertise inputs
the owner refuses. Coverage of actual built templates follows their step-5 switch.

Document and argument strings now support `minLength` and count Unicode code points.
The SDK accepts both length options. Shared conformance fixtures, WASM/SDK tests and
the reference document reflect the deliberate pre-launch change. Record keys and
text/caret offsets retain UTF-16 units. No marker was raised and no legacy reader was
added.

The integer regression first failed on `1.0` with `type_mismatch`, before the fix.
Validation and conversion into Loro now agree on integral numeric values, including
`1e2` and negative zero. Creation, checkpoint save and reopening are covered.
String tests also verify that rejected writes leave the document unchanged.

Validation:

- Focused argument, validation and conformance tests: 19 passed.
- `bun run verify`: 238 Rust tests passed (4 skipped), 152 Bun tests passed,
  installed-package checks passed, types/contracts/hygiene passed.
- The native FFI signatures and Swift sources did not change at this checkpoint.

The production command storage and runner still use the previous contracts until
the coordinated definition/storage/SDK switch. This checkpoint provides their single
checker; it does not claim that TypeBox or production `jsonschema` are fully removed.
