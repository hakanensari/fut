---
id: fut-hpia
status: closed
deps: [fut-7nfl]
links: []
created: 2026-09-15T13:21:57Z
type: feature
priority: 1
assignee: Mikkel Malmberg
parent: fut-h9kf
tags: [remote, client, navigation, input]
---
# Switch the active interactive attachment atomically between machines

Allow a user to choose a machine or one of its resources and transfer the local Fut surface from the current daemon to the target daemon without leaving the UI or disturbing background metadata connections.

## Design

Treat each rendered tab as wholly owned by one endpoint; cross-machine panes and pane moves stay out of scope. Freeze input during handoff, establish and validate the target interactive attachment and fresh SelectedView, apply geometry, reset endpoint-local modal and pending state, then commit and release the old attachment. On failure, restore the source if coherent or remain visibly frozen. Route every input, resize, selection, close, copy-mode, and action message through the active endpoint identity and connection generation.

## Acceptance Criteria

Switching never sends input or resize to the wrong daemon; the target is not shown until a matching fresh view/screen is ready; failure or disconnect during every handoff phase has deterministic recovery; background watchers survive switches; remote reconnection never steals focus; simultaneous panes from different machines are rejected explicitly.
