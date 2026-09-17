---
id: fut-62nc
status: closed
deps: [fut-ewf4, fut-eigk]
links: []
created: 2026-09-15T13:21:56Z
type: feature
priority: 1
assignee: Mikkel Malmberg
parent: fut-h9kf
tags: [remote, ssh, cli, client]
---
# Add standalone local-client remote attachment

Add a supported command such as fut --remote <ssh-target> that runs the Fut UI locally while attaching through the SSH bridge to one remote Fut daemon.

## Design

Accept SSH config host aliases first; URI syntax and named remote daemon selection may be additive. Preserve Fut's distinction between attach-only behavior and commands allowed to start a daemon. Initially require Fut to be installed remotely and require an exact protocol match. Never stop or replace an incompatible daemon automatically because doing so ends its panes. Keep terminal setup local and enter raw mode only after a successful remote handshake.

## Acceptance Criteria

fut --remote workbox attaches a local UI to an already-running compatible daemon; Phase 1 remains attach-only and never starts, stops, or replaces the remote daemon; authentication, host-key, missing-binary, missing-daemon, and protocol errors are concise and leave the terminal restored; detaching leaves remote panes alive; normal local attachment is unchanged. Remote daemon bootstrap is deferred to later work.
