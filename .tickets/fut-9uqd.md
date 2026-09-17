---
id: fut-9uqd
status: closed
deps: [fut-62nc]
links: []
created: 2026-09-15T13:21:56Z
type: feature
priority: 2
assignee: Mikkel Malmberg
parent: fut-h9kf
tags: [remote, protocol, compatibility]
---
# Define a stable capability-negotiated remote endpoint contract

Separate long-lived remote client compatibility from Fut's exact same-install private protocol so compatible client and daemon releases can remain connected during normal upgrades.

## Design

Keep the private protocol strict for internal same-install operations. Introduce a versioned remote endpoint handshake with named codecs, methods, and capabilities; additions must be optional or explicitly negotiated. Include capabilities needed by federation such as metadata watching, interactive attachment, health checks, and execution-locality operations. An incompatible endpoint must fail in isolation and must never trigger an automatic daemon restart.

## Acceptance Criteria

Compatible unequal Fut versions negotiate a documented remote contract; unsupported optional capabilities disable only their features; incompatible generations produce a typed endpoint error; frame bounds and malformed-handshake tests cover the network trust boundary; local exact-version safeguards remain intact.

## Notes

**2026-09-17T07:52:15Z**

Implemented remote generation 1 with explicit RemoteHello/RemoteWelcome DTOs, msgpack-map-v1, bounded required/optional capabilities, diagnostic-only package versions, typed endpoint errors, and daemon/client method gates. SSH remains protocol-blind; local Hello/Welcome, exact-version enforcement, mismatch escape hatch, and lifecycle code are preserved. Optional alerts/catalog/health omission disables only those features. Remote extension catalogs retain structural/fingerprint checks without requiring the local renderer package version. Added frozen contract documentation, usage guidance, and changelog entry.

Validation passed: the full `mise run check` suite with 666 library tests, 144 E2E tests, and all integration and extension checks. This includes unequal versions, optional omission, malformed/incompatible peers, failure isolation, and the local unsupported-protocol regression.
