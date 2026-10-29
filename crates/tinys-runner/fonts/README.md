# Portable capture fonts

Only shared defaults, text symbols and the color-emoji fallback live in the helper. Tiny-specific
fonts remain document resources and register for that document's capture batch.

- Geist and Geist Mono: Vercel, SIL OFL 1.1; https://github.com/vercel/geist-font
- Newsreader: Production Type, SIL OFL 1.1; https://github.com/productiontype/Newsreader
- Noto Sans Symbols 2: SIL OFL 1.1; https://github.com/google/fonts/tree/main/ofl/notosanssymbols2
  Text symbols such as U+2713; registered after Apple emoji.
- Apple Color Emoji: Apple artwork converted to CBDT/CBLC PNG bitmaps with GSUB
  sequence shaping. The conversion code is MIT; Apple artwork is not covered by that
  license. See the Apple notices alongside the font. No Twemoji fallback is included.

The Apple font uses the locally installed macOS 26.6.2 font (SHA-256
`435ee0b75b26d671e089ddad38640473cc31febc4dbdfae27843a0a000d7ff62`).
Conversion: https://github.com/samuelngs/apple-emoji-ttf at
`484daf4e13942437e083d881cae39fbc92d837e1`, `configs/web.yaml` with `split.enabled: false`.
That recipe retains 96 px strikes, quantizes PNGs to 128 colors, and adds sequence
shaping for skin tones, flags and ZWJ emoji. The complete TTF is embedded; registering
it does not copy it through QuickJS or the capture request. WOFF2 saved only about
1 MB and required decompression, so the helper uses TTF. The font is deliberately
larger than the former fallback: 20,091,552 bytes.

License notices accompany these files. Font binaries are pinned by these SHA-256 hashes:

- `Geist.woff2`: `c46b00cf667277d22cc237e58149520daec19542edc3f05e7daff4581dc23d2a` (63,660 bytes)
- `GeistMono.woff2`: `05bea62ce9fcdb202ffb21f3d93c25e05cf6dfb7e14f636c942dbd72dea7e33f` (66,232 bytes)
- `Newsreader.ttf`: `8a08d13f8a6c0d51be379a60af84f945f65369a67e509ee3c3bdcc421254d7c1` (451,664 bytes)
- `AppleColorEmoji.ttf`: `289dfece852471725feffc7a3f1f18751121c2eb67f246f1606b91b91b9412d3` (20,091,552 bytes)

- `NotoSansSymbols2-Regular.ttf`: `7d5fb73b7ca67a6798101741f5d280a3d016a56a197afcd4199dbb57b4b82a21` (1,233,128 bytes)
