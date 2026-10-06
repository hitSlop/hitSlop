# Artwork optimization levels (2026-10-05)

Measured on the development M1 with a release build (rustc 1.96.1) of `oxipng` 10.2.1,
default features off (one thread, libdeflate), with the options `file::optimize_png`
uses: the display chunks kept (oxipng's safe list plus gAMA and cHRM; these captures carry
only sRGB of them) and a 64 MiB decoded bound. Each image had one warm-up, then
30 samples per level, the three levels alternating each round. The inputs are the
artwork stored in every real capture available: Quick Checklist's template, Shape Lab's
template and one saved document.

| Image (SHA-256 prefix) | Bytes | Level 0 | p50 / p95 ms | Level 1 | p50 / p95 ms | Level 2 | p50 / p95 ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Quick Checklist preview, 960 × 1,240 (`d28af213`) | 88,579 | 60% | 25.0 / 26.0 | 55% | 107.7 / 109.9 | 53% | 203.5 / 207.2 |
| Quick Checklist icon, 512 × 512 (`4d288853`) | 18,474 | 72% | 7.5 / 7.8 | 68% | 34.3 / 36.2 | 64% | 81.5 / 84.4 |
| Saved document preview, 960 × 1,240 (`e32be2cc`) | 86,706 | 59% | 25.3 / 26.3 | 55% | 108.7 / 114.5 | 53% | 206.8 / 213.6 |
| Saved document icon (`4d288853`) | 18,474 | 72% | 7.6 / 9.1 | 68% | 34.9 / 38.1 | 64% | 82.9 / 88.2 |
| Shape Lab preview, 960 × 750 (`555c4553`) | 66,662 | 67% | 21.0 / 21.3 | 62% | 85.5 / 86.6 | 61% | 159.8 / 163.0 |
| Shape Lab icon, 512 × 512 (`09cb8244`) | 15,858 | 54% | 6.1 / 6.2 | 51% | 24.7 / 25.2 | 50% | 55.2 / 55.9 |

Percentages are the optimized size against the stored input. Every output, at every
level, decoded to the input's RGBA pixels, fully transparent ones included (compared
with Pillow).

Decision: `pack` uses level 2, once per build. A closing window's artwork uses level 0,
because the owner releases the writer lock only after writing it. At level 0, a preview
and its icon take at most 35 ms (p95), within the proposed 150 ms budget; level 1 reaches
153 ms for one document, and level 2 about 300 ms.

The release `slop-engine` grew from 10,052,096 to 10,339,872 bytes. The FFI static
library grew from 50,841,056 to 51,469,608 bytes, before linking strips it.
