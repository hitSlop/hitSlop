# Conformance fixture fonts

`conformance-cjk.woff2` is a Noto Sans TC subset containing only 中 (U+4E2D)
and 文 (U+6587), the Chinese characters used by this fixture. It retains the
variable weight range 100–900. The packaged font lets the fixture exercise native
PNG and vector PDF capture without depending on installed system fonts.

The source was the copy of `NotoSansTC-VariableFont_wght.woff2` in
`_docs/takumi-master/assets/fonts/noto-sans/`, version
`2.004-H2;hotconv 1.0.118;makeotfexe 2.5.65603` (5,422,704 bytes).
Its SHA-256 is
`00dc2135d846a07b44dfa73e4e59d0f949200bd76b48bde8a417920bcb472e8f`.
The upstream font is [Noto Sans TC in Google Fonts](https://github.com/google/fonts/tree/main/ofl/notosanstc).
The matching SIL Open Font License is included as
[NotoSansTC-OFL.txt](NotoSansTC-OFL.txt), copied from that upstream directory.

The subset was generated from the repository root with fontTools 4.63.0:

```sh
uv run --with 'fonttools[woff]==4.63.0' python -m fontTools.subset \
  _docs/takumi-master/assets/fonts/noto-sans/NotoSansTC-VariableFont_wght.woff2 \
  --unicodes=U+4E2D,U+6587 --flavor=woff2 \
  '--layout-features=*' '--name-IDs=*' --name-legacy '--name-languages=*' \
  --notdef-glyph --notdef-outline \
  --output-file=tests/abi/owner-svelte/media/conformance-cjk.woff2
```

The resulting file is 1,836 bytes, with SHA-256
`db9c1f152495976434a162b18838aee9ddc0617daf1c985ac52c4cf2f61d4879`.
Builds use this checked-in subset directly; neither `_docs` nor fontTools is a
build dependency. This is fixture coverage for these two characters, not a general
CJK font distribution.

## Registry assets

`sample.*` files are minimal complete app-asset fixtures, retained by explicit
`?url` imports in `../media.ts`. Images are 16×16 solid RGB (51, 85, 119) patches
encoded with Pillow/HEIF; audio and videos were encoded with FFmpeg from the
existing tone or a matching solid-color frame. The TTF, WOFF and CFF OTF are
original empty-space fonts generated with fontTools FontBuilder; they have no
external font dependencies. The PDF is an original empty 16×16 page; WASM is an
empty version-1 module. Text/code/vector/data fixtures are original minimal files.
Both `.js` and `.mjs` exercise the registry's canonical `.js` asset output.
