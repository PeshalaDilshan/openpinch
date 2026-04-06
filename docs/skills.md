# Skills

OpenPinch skills are signed local packages.

## Package Model

- Package format: `.tar.zst`
- Detached manifest signature: Ed25519
- Manifest contains file digests, metadata, entrypoint, and runtime requirements

## Registry Model

- Registry index lives under `skills/registry/`
- Trusted root keys live under `skills/trust/`
- Registry metadata is verified before skills are listed or installed

## CLI

```bash
openpinch skill list
openpinch skill verify skills/examples/echo.skill
openpinch skill install skills/examples/echo.skill
```

