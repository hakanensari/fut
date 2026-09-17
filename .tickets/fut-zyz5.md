---
id: fut-zyz5
status: open
deps: [fut-7neq, fut-9uqd]
links: []
created: 2026-09-15T13:21:56Z
type: feature
priority: 1
assignee: Mikkel Malmberg
parent: fut-h9kf
tags: [remote, client, resources, reliability]
---
# Supervise independent endpoint connections and metadata snapshots

Maintain Local plus enabled SSH endpoint connections independently and keep machine-scoped resource, presence, agent, and alert summaries available without streaming every machine's terminal screens.

## Design

Give every endpoint a MachineId, connection generation, status, protocol negotiation, and independent revisions. Keep ResourceSnapshot and alert/presence state per endpoint rather than concatenating them into one synthetic daemon snapshot. Use control WatchResources connections for background metadata; reserve an interactive connection for the active endpoint. Reconnect transient failures with bounded exponential backoff and application-level health checks; classify host-key, authentication, installation, and compatibility failures as Attention requiring an interactive command.

## Acceptance Criteria

One stalled or disconnected endpoint cannot delay another endpoint's input or updates; stale events from prior connection generations are discarded; disconnected metadata remains clearly stale and cannot receive input; reconnecting never changes the active machine; background endpoints do not stream screen frames; Local can reconnect independently; tests cover sleep/EOF, revision reset, partial startup, and catalog changes.
