# Maintainers

This file defines the human ownership model for OpenPinch beyond the path-based rules in `.github/CODEOWNERS`.

## Current Maintainer

- `@PeshalaDilshan`
  - Role: repository owner and primary maintainer
  - Scope: repository-wide
  - Responsibilities:
    - release approval for `main`
    - branch strategy and CI policy
    - protobuf and public API stability
    - sandbox and security-sensitive review
    - final arbitration for cross-cutting architectural changes

## Review Model

- Path-based review routing is enforced through `.github/CODEOWNERS`.
- Maintainers are responsible for final approval on changes that affect:
  - `.github/`
  - `proto/`
  - `core/crates/sandbox/`
  - security, attestation, audit, or release automation behavior
- Feature work should normally be reviewed and merged into `dev`.
- `main` should only receive the automated `dev -> main` promotion PR or an explicitly approved stabilization PR.

## Maintainer Expectations

- Keep the project local-first and privacy-first.
- Protect `main` with required status checks and code-owner review.
- Keep CI, CODEOWNERS, and branch protection aligned.
- Require documentation updates when behavior, policy, or public interfaces change.
- Treat security, sandbox, protocol, and release changes as high-sensitivity reviews.

## Escalation Areas

The following should always receive maintainer attention:

- protobuf contract changes
- branch or release automation changes
- sandbox policy or capability matrix changes
- encryption, attestation, or audit pipeline changes
- operator deployment model changes

## Future Team Expansion

When more maintainers or GitHub teams are added:

1. Update `.github/CODEOWNERS`.
2. Update `docs/ownership.md`.
3. Update this file with maintainers, scopes, and escalation paths.
4. Update GitHub branch protection to require the appropriate owner or team reviews.

## Suggested Future Team Map

These are placeholders for future GitHub teams, not active owners yet:

- `@openpinch/core-rust`
- `@openpinch/gateway-go`
- `@openpinch/security`
- `@openpinch/docs`
- `@openpinch/ui`
