# SBOMs

- Default format: CycloneDX JSON (`*.cdx.json`).
- Modes:
  - `auto` (default): try native generators (`cargo cyclonedx`, `cyclonedx-gomod`, `cyclonedx-npm`, `cyclonedx-py`).
  - `native`: require native tools.
  - `fallback`: generate a minimal SBOM derived from lockfiles/build metadata.
- SBOMs are created per target and listed in `manifest.json` and `SHA256SUMS`.
