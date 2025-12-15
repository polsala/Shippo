# Troubleshooting

- **Missing tool (cargo/go/python/npm)**: ensure language toolchains are installed and on PATH.
- **cosign not found**: install cosign or disable signing (`[sign].enabled = false`) locally.
- **gpg key issues**: import the signing key and trust it; set `GNUPGHOME` if needed.
- **pyinstaller build fails**: verify entrypoint path and hidden imports; switch to wheel mode if packaging libraries only.
- **node pkg errors**: set `node.binary.tool = "nexe"` or lock to supported Node version.
- **manifest verification fails**: check for missing files in `dist/`, regenerate with `shippo package`.
