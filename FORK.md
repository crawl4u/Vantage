# Cinnabar with native Vantage

A fork of [cinnabar-mc/cinnabar](https://github.com/cinnabar-mc/cinnabar) at `08446aa` that
runs Vantage natively inside the client (`crates/vantage`, `app/src/modding/vantage/`).
It is built and used exactly like upstream: same license (PolyForm Noncommercial 1.0.0, see
`LICENSE`), no game assets or binaries in the repository, and users generate or supply their
own compiled carriers. Branding (title and icon) is Vantage's own.

- Port status and what still needs testing: `docs/vantage-port-status.md`.
- The Vantage mod as a wasm component, for the upstream host: `vantage-mod/vantage.wasm`.
- Third-party licenses: `THIRD_PARTY_NOTICES.md`.
- Build: `make play` (or `cargo build --profile play -p cinnabar`).
