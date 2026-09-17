---
id: fut-eigk
status: closed
deps: []
links: []
created: 2026-09-15T13:21:56Z
type: task
priority: 1
assignee: Mikkel Malmberg
parent: fut-h9kf
tags: [remote, security, extensions, projects]
---
# Make client actions safe across local and remote execution contexts

Audit and define execution locality for behavior currently assuming that client and daemon share a filesystem and process namespace. Prevent a local remote-attached client from executing daemon-advertised paths, PIDs, sockets, extensions, project recipes, or trust operations on the wrong machine.

## Design

Classify built-in UI/config/keybindings and desktop clipboard as client-local. Keep daemon hooks and terminal processes daemon-local. Route daemon-owned extension commands and project operations to their owning endpoint when supported; until then disable them explicitly with a useful explanation. Client lifecycle hooks must come from locally installed client extensions, not remote catalog paths. Treat workspace roots and child PIDs as display/remote-domain values unless an operation is sent to their endpoint.

## Acceptance Criteria

Remote attachment cannot spawn or inspect a local process using a remote executable path, PID, workspace root, or FUT_SOCKET; unsupported remote extension/project actions are visibly unavailable rather than mis-executed; local UI configuration and clipboard continue to work; focused tests cover malicious or coincidentally valid remote paths and differing extension catalogs.
