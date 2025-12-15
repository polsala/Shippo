# Configuration reference

Shippo reads `.shippo.toml`. Single-project uses `[project]`; monorepo uses `[[packages]]`. Defaults align with the product requirements: CycloneDX SBOM, cosign keyless in CI, pkg for Node CLI binaries, and tar.gz/zip outputs with `name_template = "{name}-{version}-{target}"`.

## Core sections (all projects)

- `[project]` / `[[packages]]` – `name`, `type` (`rust|go|node|python`), `path` (default `.`).
- `[version]` – `source = tag|manual|git`, `manual = "1.2.3"` when manual.
- `[build]` – `targets = ["native", "linux-amd64"]`, `env = { KEY = "VALUE" }`.
- `[package]` – `formats = ["tar.gz", "zip"]`, `name_template = "{name}-{version}-{target}"`, `include`/`exclude` globs.
- `[sbom]` – `enabled`, `format = "cyclonedx"`, `mode = auto|native|fallback` (auto prefers native generators, then fallback).
- `[sign]` – `enabled`, `method = cosign|gpg`, `cosign_mode = keyless|key`.
- `[release]` – `provider = "github"`, `draft`, `prerelease`.
- `[release.github]` – `owner`, `repo`.
- `[changelog]` – `mode = auto|conventional|file`, `file = "CHANGELOG.md"` when using file.

## Complete examples by language

### Rust binary
```toml
[project]
name = "rust-cli"
type = "rust"
path = "."

[version]
source = "git" # or "tag" / "manual"

[build]
targets = ["native", "x86_64-unknown-linux-gnu"]

[package]
formats = ["tar.gz", "zip"]
name_template = "{name}-{version}-{target}"

[sbom]
enabled = true
format = "cyclonedx"
mode = "auto" # uses cargo-cyclonedx if available, else fallback

[sign]
enabled = true
method = "cosign"
cosign_mode = "keyless" # ideal in GitHub Actions

[release]
provider = "github"
draft = true
prerelease = false
[release.github]
owner = "acme"
repo = "rust-cli"
```

### Go binary
```toml
[project]
name = "go-svc"
type = "go"
path = "."

[build]
targets = ["linux-amd64", "darwin-arm64"]

[package]
formats = ["tar.gz"]

[sign]
enabled = true
method = "gpg"

[sbom]
mode = "fallback" # uses gomod lock-derived SBOM when cyclonedx-gomod not present
```

### Node frontend
```toml
[project]
name = "frontend"
type = "node"
path = "."

[node]
mode = "frontend"
[node.frontend]
build_dir = "dist"
build_cmd = "npm run build" # default if script exists

[build]
targets = ["native"] # kept for naming consistency

[package]
formats = ["zip"]
include = ["dist/**"]
```

### Node CLI binary (pkg default)
```toml
[project]
name = "node-tool"
type = "node"
path = "."

[node]
mode = "cli-binary"
[node.binary]
tool = "pkg" # or "nexe"
entry = "src/index.js"
targets = ["linux-x64", "macos-arm64", "win-x64"]

[sign]
enabled = true
method = "cosign"
cosign_mode = "keyless"
```

### Python wheel library
```toml
[project]
name = "py-lib"
type = "python"
path = "."

[python]
mode = "wheel" # uses python -m build (wheel + sdist)

[sbom]
mode = "auto" # cyclonedx-py if available, else fallback from lockfiles
```

### Python PyInstaller app
```toml
[project]
name = "py-app"
type = "python"
path = "."

[python]
mode = "pyinstaller"
[python.pyinstaller]
mode = "onefile" # or "onedir"
entry = "app/main.py"
hidden_imports = ["pkg_resources.py2_warn"]
data = ["static/**"]

[package]
formats = ["tar.gz", "zip"]
```

## Monorepo patterns

### Mixed languages
```toml
[[packages]]
name = "rust-cli"
type = "rust"
path = "apps/rust-cli"
[packages.build]
targets = ["native"]

[[packages]]
name = "go-svc"
type = "go"
path = "services/go-svc"
[packages.build]
targets = ["linux-amd64"]

[[packages]]
name = "web"
type = "node"
path = "apps/web"
[packages.node]
mode = "frontend"
[packages.node.frontend]
build_dir = "dist"

[[packages]]
name = "py-tool"
type = "python"
path = "tools/py-tool"
[packages.python]
mode = "pyinstaller"
[packages.python.pyinstaller]
mode = "onefile"
entry = "cli.py"

[version]
source = "tag"

[release]
provider = "github"
draft = true
[release.github]
owner = "acme"
repo = "super-repo"
```

### Per-package overrides (SBOM/sign/build)
```toml
[[packages]]
name = "node-cli"
type = "node"
path = "apps/cli"
[packages.node]
mode = "cli-binary"
[packages.node.binary]
entry = "bin/index.js"
targets = ["linux-x64"]
[packages.sign]
enabled = true
method = "cosign"

[[packages]]
name = "python-lib"
type = "python"
path = "libs/py"
[packages.sbom]
mode = "native" # require cyclonedx-py
```

## Signing and SBOM defaults

- Cosign keyless is assumed in CI; set `[sign].enabled = true` to turn on signing.
- GPG is supported when keys are available on the runner.
- SBOMs default to CycloneDX; `mode = auto` uses native generators when present.

## Versioning and changelog

- `source = "git"` uses the latest tag (or v0.1.0 fallback).
- `source = "tag"` strictly uses the latest tag; errors if none exist.
- `source = "manual"` requires `manual = "x.y.z"`.
- `changelog.mode = "auto"` uses git log; `"conventional"` groups by feat/fix/breaking; `"file"` reads a provided file.
