# Sandbox

OpenPinch exposes a platform-aware sandbox contract:

- Linux: Firecracker + `jailer` + seccomp + containerd-in-guest expectation
- macOS: virtualization backend contract + Seatbelt-oriented restriction path
- Windows: Hyper-V backend contract + Job Object restriction path

## Policy Enforcement

- Tool and skill execution is deny-by-default.
- Capability rules are loaded from `skills/policies/default.yaml`.
- The executor currently models capabilities such as `tool.local`, `tool.inspect`, `shell.execute`, `sandbox.execute`, `skill.execute`, `agent.message`, and `network.egress`.
- `builtin.command` is intentionally denied outbound network access by policy unless the capability matrix is changed locally.

## Linux Firecracker Flow

The Linux backend expects:

- `firecracker` on `PATH` or configured explicitly
- `jailer` on `PATH` or configured explicitly
- A guest kernel image
- A guest root filesystem image
- An optional seccomp JSON policy

For each execution request the backend:

1. Creates an ephemeral workspace
2. Stages a task payload
3. Starts `jailer`
4. Configures the microVM through the Firecracker API socket
5. Boots the guest and waits for completion
6. Collects result artifacts and destroys the workspace

Use `openpinch start` or `openpinch execute` to surface sandbox doctor errors early. The runtime health surface also reports degraded states when Firecracker, `jailer`, guest assets, or `containerd` are missing.
