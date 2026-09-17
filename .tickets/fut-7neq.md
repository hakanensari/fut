---
id: fut-7neq
status: open
deps: [fut-62nc]
links: []
created: 2026-09-15T13:21:56Z
type: feature
priority: 2
assignee: Mikkel Malmberg
parent: fut-h9kf
tags: [remote, ssh, cli, config]
---
# Add saved SSH machine profiles and management commands

Store a local catalog of SSH endpoints and provide machine add/list/rename/enable/disable/remove commands. A profile identifies one independently managed remote Fut endpoint and has a stable opaque ID.

## Design

Persist only opaque ID, label, SSH target, optional remote runtime/session selection, and enabled state. Never store passwords, private keys, agent sockets, or SSH control sockets. Validate and bound catalog size and fields, reject embedded passwords and control characters, write atomically with private permissions, and save a new profile only after interactive setup succeeds. Removing or disabling a profile disconnects it but never stops its daemon or panes.

## Acceptance Criteria

Machine commands have human and JSON output; duplicate targets may have distinct profile identities; invalid/private data is rejected; failed or cancelled setup leaves no saved profile; editing the catalog while Fut is open can be detected safely; profile removal never contacts the remote daemon to shut it down.
