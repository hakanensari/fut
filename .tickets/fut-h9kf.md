---
id: fut-h9kf
status: open
deps: []
links: []
created: 2026-09-15T13:21:56Z
type: epic
priority: 1
assignee: Mikkel Malmberg
tags: [remote, ssh, client, daemon]
---
# Remote SSH attachment and multi-machine federation

Let one local Fut client attach to Fut daemons on SSH hosts and eventually combine Local plus saved remote machines in one window. Each machine remains an independent authority for its sessions, processes, resources, and persistence; the client federates them. There is no main server and no daemon TCP listener.

## Design

Use outbound OpenSSH as authentication, encryption, host discovery, and routing. A fixed remote bridge command forwards framed Fut protocol bytes between SSH stdio and the remote private Unix socket. Start with one remote endpoint and exact protocol compatibility, then add saved endpoint profiles, background metadata watchers, independent reconnect supervision, machine-qualified navigation, and atomic active-endpoint switching. Never permit a remote path, PID, socket, extension, recipe, or trust decision to be mistaken for a local one.

## Acceptance Criteria

A local Fut UI can securely attach to a remote Fut daemon through SSH; saved Local and SSH endpoints can be shown together and fail independently; only the selected endpoint owns interactive input and terminal geometry; no TCP daemon is exposed; remote execution locality is explicit and tested.


## Notes

**2026-09-15T14:21:20Z**

Phase 1 complete: private SSH stdio bridge, explicit remote execution-locality policy, and attach-only fut --remote HOST shipped in the working tree. Closed fut-ewf4, fut-eigk, and fut-62nc after full local checks and a real SSH attach/detach validation against clonk. Remote daemon bootstrap remains deferred.
