# Migration From OpenClaw

OpenPinch keeps the same broad monorepo shape as OpenClaw, but the internals differ in a few important ways:

- The execution core is Rust instead of a garbage-collected runtime.
- Messaging and scheduling are isolated in a Go gateway with a stable gRPC boundary.
- Skills are signed and verified locally before install or execution.
- Sandbox isolation is treated as a first-class runtime concern instead of an optional afterthought.

## Migration Notes

- Existing skill ideas can be ported, but package manifests must be signed with the OpenPinch trust model.
- Messaging connectors should target the gateway connector interface, not the Rust engine directly.
- Local model configuration is explicit and never defaults to a hosted provider.

