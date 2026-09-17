---
id: fut-7nfl
status: open
deps: [fut-zyz5]
links: []
created: 2026-09-15T13:21:57Z
type: feature
priority: 2
assignee: Mikkel Malmberg
parent: fut-h9kf
tags: [remote, ui, navigation, agents]
---
# Present machine-qualified navigation, agents, and notifications

Extend Fut's navigation surfaces to show Local and saved machines, their connection state, and their sessions/workspaces/tabs/panes and agents without confusing identical names or daemon-local revisions.

## Design

Add a machine level only when multiple endpoints are configured. Qualify navigator results, navigation history, alerts, pending actions, and selected targets with MachineId. Show Online, Reconnecting, Attention, Disabled, and stale states. Cached disconnected resources may be inspected as summaries but cannot be activated. Preserve the existing compact hierarchy when only Local is configured.

## Acceptance Criteria

Users can distinguish and filter resources and agents from multiple machines; labels identify machine ownership where ambiguous; stale/offline targets cannot receive navigation or commands; machine failures and required setup actions are visible; one-machine rendering and keybindings remain unchanged; narrow layouts remain usable.
