# Security policy

## Reporting a vulnerability

Please do not open a public issue for a suspected vulnerability. Use GitHub's
private vulnerability reporting for the `hitslop/hitslop` repository. Include
the affected component, reproduction steps, impact, and any suggested fix.

We will acknowledge a complete report as soon as practical and coordinate a
fix and disclosure timeline with the reporter.

## Supported versions

Security fixes target the latest shared release of the macOS app and the `hitslop`
npm package. The first shared release is 1.0.0. Saved-document compatibility follows
the [engineering contract](docs/engineering-contract.md#compatibility).

## Secret handling

Never commit `.env*`, `.dev.vars`, Apple `AuthKey_*.p8` files, signing keys,
provisioning profiles, or Sparkle private keys. Commit only the documented
`*.example` files with placeholders.
