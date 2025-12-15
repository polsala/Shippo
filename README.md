# Shippo

Shippo is a polyglot release orchestrator for Rust, Go, Node, and Python. It builds, packages, signs, and publishes GitHub Releases with deterministic artifacts, SBOMs (CycloneDX by default), and manifest-driven verification.

## Quickstart

```bash
cargo install --path crates/shippo
shippo init
shippo plan
shippo build
shippo package
shippo release --dry-run
```

### Example configs

- Rust: see `tests/fixtures/rust-hello/.shippo.toml` (generate via `shippo init`).
- Monorepo: use `[[packages]]` entries per project.

### Command overview

- `shippo init` – detect projects and scaffold `.shippo.toml`.
- `shippo plan` – render build plan (`--json` available).
- `shippo build` – run language-specific builders for configured targets.
- `shippo package` – create archives, SBOMs, `SHA256SUMS`, `manifest.json`, signatures, and provenance.
- `shippo release` – build + package + publish a GitHub Release (draft by default, `--dry-run` to skip publish).
- `shippo verify` – validate manifest, checksums, signatures, and SBOM presence.

## Features

- Native builders for Rust (cargo), Go (`go build` with ldflags), Node (frontend builds or CLI binaries via `pkg`/`nexe`), and Python (wheel or PyInstaller).
- SBOM generation (CycloneDX) with fallback lockfile-derived metadata.
- Signing support: cosign keyless (preferred in CI) or GPG; verification via manifest references.
- Deterministic packaging: archive naming templates, `manifest.json`, `SHA256SUMS`, and `provenance.json`.
- GitHub Release publishing with changelog generation and asset uploads.

## CI usage

See `.github/workflows/ci.yml` and `docs/github-actions.md` for recommended workflows. `release.yml` dogfoods Shippo to publish tagged releases.

## Documentation

- `docs/config.md` – full configuration reference.
- `docs/signing.md` – cosign keyless & GPG guidance.
- `docs/sbom.md` – SBOM generation and fallback behavior.
- `docs/github-actions.md` – CI examples.
- `docs/troubleshooting.md` – common issues and fixes.
