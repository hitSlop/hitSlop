# Capture renderer licenses

The native capture renderer uses [Takumi](https://github.com/kane50613/takumi)
and its experimental, unpublished vector PDF backend from pinned upstream revision
`e527358cbd9b4f5deb361f56ef2cb02a88759f2b` (Takumi 2.15.1, PDF 0.16.1).
Cargo fetches that source directly; no local source fork is required. Cargo.lock pins
the complete dependency graph.

These notices accompany the distributed evaluator in the macOS app and npm engine
package. `LICENSE-MIT`, `LICENSE-APACHE`, `THIRD-PARTY-NOTICES.md` and
`LICENSE-CHROMIUM` come from Takumi. The third-party notice's Chromium license link
is adjusted to this directory's flat layout.

The PDF backend includes a krilla fork. `LICENSE-KRILLA-MIT` and
`LICENSE-KRILLA-APACHE` retain its [krilla 0.8.2 upstream notices](https://github.com/LaurenzV/krilla).
`LICENSE-ICC` comes from `takumi-pdf/icc/LICENSE` and covers the bundled color
profiles. Font notices are packaged separately from `../fonts`.

When changing the pinned revision, review the upstream licenses and third-party
notices together with the dependency update.
