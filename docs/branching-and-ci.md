# Branching And CI

OpenPinch uses three long-lived branches:

- `main`: stable production branch. No direct pushes should be allowed. Changes should arrive through pull requests only.
- `dev`: active development branch. Day-to-day integration lands here first.
- `test`: experimental branch for trying patches, spikes, or risky features before they are ready for `dev`.

Only `main` and `dev` are part of the required CI path right now.

## Intended Flow

1. Contributors open feature branches from `dev`.
2. Work is reviewed and merged into `dev`.
3. A push to `dev` runs the branch CI workflows.
4. The release automation re-validates the repo on `dev` and automatically creates or updates a `dev -> main` pull request.
5. A pull request into `main` runs the CI workflows again before merge.
6. A merge into `main` builds release artifacts.

`test` is intentionally outside the required CI gate. It is available for experimentation, partial migrations, or unstable patches without changing the stable promotion path.

## Workflow Files

The repository keeps CI split by concern:

- `.github/workflows/ci-rust.yml`
- `.github/workflows/ci-go.yml`
- `.github/workflows/ci-flutter.yml`
- `.github/workflows/release.yml`

### `ci-rust.yml`

Runs on:

- push to `dev`
- pull request targeting `main`
- manual dispatch

Checks:

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`

### `ci-go.yml`

Runs on:

- push to `dev`
- pull request targeting `main`
- manual dispatch

Checks:

- `gofmt` verification
- `go test ./...`
- `go vet ./...`

### `ci-flutter.yml`

Runs on:

- push to `dev`
- pull request targeting `main`
- manual dispatch

Behavior:

- If `ui/pubspec.yaml` does not exist, the workflow exits cleanly in placeholder mode.
- Once the Flutter UI lands, the workflow runs `flutter pub get`, `flutter analyze`, `flutter test`, and `flutter build web --release`.

### `release.yml`

Runs on:

- push to `dev`
- push to `main`
- manual dispatch

Behavior on `dev`:

- Re-runs the aggregate validation gate.
- Creates or updates a `dev -> main` pull request automatically.
- Attempts to enable GitHub auto-merge for that PR.

Behavior on `main`:

- Builds the release CLI binary.
- Builds the Go gateway binary.
- Uploads both as GitHub Actions artifacts.

## Required GitHub Settings

Some rules cannot be enforced by workflow YAML alone. Repository maintainers should configure the following in GitHub settings:

### `main` branch protection

- Require a pull request before merging.
- Require review from code owners.
- Disable direct pushes for everyone except repository admins if needed.
- Require status checks to pass before merging.
- Require these checks at minimum:
  - `Rust Checks`
  - `Go Checks`
  - `Flutter Checks`
- Optionally require linear history and signed commits if your org policy wants them.

### `dev` branch protection

- Allow normal development pushes if that matches your team workflow.
- Optionally require pull requests for larger teams, but do not change the promotion model: `dev` remains the integration branch feeding `main`.

### Auto-merge

- Enable repository-level auto-merge in GitHub settings if you want the automation in `release.yml` to complete the promotion without manual intervention.

## Operational Notes

- The promotion workflow intentionally duplicates the high-level checks instead of assuming other workflows have already completed. That makes the `dev -> main` gate deterministic.
- If the Flutter UI is still absent, Flutter CI remains green without blocking Rust and Go work.
- Release artifact uploads on `main` are build artifacts, not tagged GitHub Releases yet.
- CODEOWNERS should be kept in sync with `docs/ownership.md` so review routing and written policy do not drift apart.
