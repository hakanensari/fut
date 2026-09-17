---
id: fut-68p1
status: open
deps: [fut-hpia]
links: []
created: 2026-09-15T13:21:57Z
type: task
priority: 2
assignee: Mikkel Malmberg
parent: fut-h9kf
tags: [remote, docs, testing, security]
---
# Harden and document remote and federated Fut workflows

Finish the remote feature with end-to-end coverage, diagnostics, documentation, and resource/performance validation across local and unreliable SSH transports.

## Design

Add deterministic bridge tests that do not require a public host plus opt-in real-SSH smoke tests. Exercise disconnects, backpressure, malformed frames, incompatible versions, daemon restarts, multiple clients, resizing, process cleanup, and terminal restoration. Document plain ssh -t host fut versus local-client fut --remote, machine federation, SSH-agent/known-host requirements, unsupported Windows/server combinations, and the absence of a central server or TCP listener. Measure metadata traffic and bound profile count/retry behavior.

## Acceptance Criteria

The full suite covers standalone remote attach and at least two independent simulated endpoints; security and failure behavior is documented; fut doctor diagnoses SSH binary/config, endpoint compatibility, and saved-profile errors without modifying hosts; no leaked SSH processes or bridge sockets remain after tested exits; CHANGELOG and user docs describe the shipped behavior.
