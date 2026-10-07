# Full PNG acceptance and 2× skins

2026-10-06. Resource acceptance foundation; no commit.

`images.rs` replaces the hand-written PNG header checker on writes and skin opens.
Pinned png 0.18.1 is now a normal dependency of the storage feature. The validator
uses strict CRC and Adler32 checks, an 8 MiB decoder allocation limit and bounded
row decoding. It accepts RGB/RGBA at 8/16 bits, including Adam7, requires RGBA for a
skin, and checks exact 1×/2× skin dimensions and 512×512 icons. It rejects missing
pixels, truncation, animation, invalid checksums and bytes after IEND.

Inspection of the pinned decoder found two defaults that would weaken this rule:
Adler32 checking is off and ancillary CRC failures are tolerated. Both are explicitly
disabled. The decoder also ignores some malformed/late animation controls, so a
bounded chunk-framing pass refuses every acTL/fcTL/fdAT. The library still owns the
PNG codec, checksums and pixel validation.

Every pack/capture artwork write uses full acceptance. Stored artwork opens retain
the cheap bounded header check through the PNG library. The optimizer preserves
accepted color types and bit depths and remains lossless. Swift's mask and backing
layers set `contentsScale` from pixel dimensions and declared point dimensions.

Regression evidence: `pack_refuses_a_png_header_without_pixels_and_keeps_the_previous_template`
failed before the fix because the old packer accepted a valid IHDR without pixels.
It now passes and proves that refusal preserves the previous output. Rust and CLI
fixtures now contain real image data. The SQLite erased-value test starts with a
legal PNG text chunk and confirms its marker exists before replacement; it no longer
uses invalid bytes after IEND.

Verification:

- Five focused PNG tests cover accepted color/depth/interlace combinations, scale,
  icon dimensions, every truncation of a small PNG, CRC/Adler32 failures, absent pixel
  rows, animation controls, oversized dimensions and bounded metadata allocation.
- The file suite passed, including lossless optimization, secure replacement and
  atomic refusal. The first full run exposed a grayscale shared CLI fixture; it was
  replaced with the same real RGBA fixture helper used by portable build tests.
- `bun run verify` then passed in 415.9 seconds: 257 Rust tests (4 skipped), 152 Bun
  tests, and static checks. Unchanged tiers used their existing receipts.
- Six focused native mask tests passed through `bun run verify swift --filter
  'imageMask|doubleResolutionSkin|ringMask|skinnedWindows'`, including the new 2×
  point-size/backing-scale/hit-region check. Native build and tests took 192.1 seconds.
  The existing libdeflate minimum-macOS linker warning remains; this run on the current
  Mac does not establish compatibility with the minimum supported OS.

The default source compiler and SQLite layout remain unchanged in this checkpoint.
