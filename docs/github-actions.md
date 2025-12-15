# GitHub Actions

Use Shippo in CI with the provided workflows or embed in your own pipeline.

```yaml
name: release
on:
  push:
    tags: ["v*.*.*"]
permissions:
  contents: write
  id-token: write
jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - run: cargo install --path crates/shippo
      - run: shippo --config .shippo.toml release --dry-run
      - run: shippo --config .shippo.toml release
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

See `.github/workflows/release.yml` for a full dogfooding example building and publishing Shippo itself.
