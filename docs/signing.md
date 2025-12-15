# Signing

Shippo signs artifacts when `[sign].enabled = true`.

## Cosign (default in CI)

- Keyless mode uses GitHub Actions OIDC (requires `permissions: id-token: write`).
- Provide `COSIGN_EXPERIMENTAL=1` and ensure `cosign` is installed.
- Shippo attempts `cosign sign-blob` for each archive, SBOM, and checksum file.

## GPG

- Requires a private key imported on the runner and `gpg` on PATH.
- Shippo uses detached signatures (`.sig`).

## Verification

`shippo verify` reads `dist/manifest.json`, asserts every file exists, hashes match, and signatures are present. When cosign/gpg tooling is missing, Shippo falls back to deterministic hash-based signatures so verification remains possible.
