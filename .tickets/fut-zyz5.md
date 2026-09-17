---
id: fut-zyz5
status: closed
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

## Notes

**2026-09-17T10:30:00Z**

Implemented independent metadata supervisors for Local and every enabled saved SSH machine. Machine-scoped generations gate all resource, presence, alert, extension-catalog, and health updates; disconnects retain stale snapshots without any input path, while new connections reset revision authority. Transient failures use capped exponential backoff and correlated health probes; host-key, authentication, installation, and compatibility failures stop for attention. Saved-catalog changes reconcile live without switching the active machine, and background SSH is non-interactive, captures bounded diagnostics, and never decodes terminal screens.

Added lease-free `control-alerts.v1`, reference-counted alert subscriptions, cancellation-safe SSH ownership, and focused coverage for stalled and partial startup, EOF and health timeout, stale generations, revision reset, catalog changes, missing optional streams, classification, frame cancellation, probe correlation, and screen rejection. The full `mise run check` suite passes with 685 library tests, 144 E2E tests, and all integration and extension checks.
