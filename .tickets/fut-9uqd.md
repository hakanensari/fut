---
id: fut-9uqd
status: in_progress
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
