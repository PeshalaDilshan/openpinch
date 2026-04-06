# Ownership

This document defines repository ownership for reviews, escalation, and future maintainer growth.

The source of truth for path-based review ownership is [`../.github/CODEOWNERS`](../.github/CODEOWNERS).
Human maintainer responsibilities and escalation paths live in [`../MAINTAINERS.md`](../MAINTAINERS.md).

## Owner Of Record

- Repository owner and primary maintainer: `@PeshalaDilshan`

Until more maintainers are added, all major areas route to the same owner. The ownership map is still split by subsystem so the repository can scale without redesigning the review model later.

## Area Ownership

- Repository governance, release flow, CI, and top-level build files:
  - `.github/`
  - `README.md`
  - `CONTRIBUTING.md`
  - `Makefile`
  - `docker-compose.yml`
  - `rust-toolchain.toml`
- Rust runtime:
  - `cli/`
  - `core/crates/common/`
  - `core/crates/engine/`
  - `core/crates/tools/`
  - `core/crates/sandbox/`
  - `proto/`
- Go gateway and operator:
  - `gateway/`
  - `deploy/operator/`
- Product documentation, skills, scripts, and future UI:
  - `docs/`
  - `skills/`
  - `scripts/`
  - `ui/`

## Review Expectations

- Normal feature work should target `dev`.
- Pull requests that affect one owned area should request review from that area owner.
- Cross-cutting pull requests should be reviewed as if they touched every affected ownership area, even if GitHub collapses ownership to the same user today.
- Changes to `proto/`, CI workflows, sandbox policy, or branch automation should be treated as high-sensitivity reviews.

## Triage Labels

Issue triage should use the repository label catalog in `../.github/labels.json`.

Core ownership labels:

- `area/core-rust`
- `area/gateway-go`
- `area/security`
- `area/docs`
- `area/ui`

Supporting triage labels:

- `type/bug`
- `type/feature`
- `type/docs`
- `type/connector`
- `status/needs-triage`
- `status/needs-owner`
- `status/blocked`
- `priority/p0`
- `priority/p1`
- `priority/p2`

The goal is simple:

- `area/*` shows who should look first.
- `type/*` shows what kind of work it is.
- `status/*` shows where it is in triage.
- `priority/*` shows urgency.

## Branch Ownership Model

- `main`:
  - release and stability branch
  - should require pull requests
  - should require code owner review in GitHub branch protection
- `dev`:
  - integration branch
  - should remain the default target for normal feature work
- `test`:
  - experimental branch
  - can be used for risky work without changing the stable promotion model

## GitHub Settings To Pair With CODEOWNERS

Maintainers should enable the following on `main` branch protection:

- Require a pull request before merging
- Require approval from code owners
- Require the Rust, Go, and Flutter checks to pass
- Block direct pushes except where repository administration explicitly allows them

For `dev`, requiring code owner review is optional, but recommended once more maintainers are added.

## Future Expansion

When new maintainers or teams are added:

1. Update `.github/CODEOWNERS`.
2. Update `MAINTAINERS.md`.
3. Update this document.
4. Update `CONTRIBUTING.md` if the review path changes.
5. Update branch protection rules to require the new owner set where needed.
