---
id: fut-ewf4
status: closed
deps: []
links: []
created: 2026-09-15T13:21:56Z
type: feature
priority: 1
assignee: Mikkel Malmberg
parent: fut-h9kf
tags: [remote, ssh, protocol, daemon]
---
# Add a private SSH stdio bridge for the Fut protocol

Add an internal remote-host command that connects to the selected Fut Unix socket and copies bytes losslessly between that socket and stdin/stdout. Add the local bridge lifecycle needed to run it through ssh -T without exposing the daemon on TCP.

## Design

Keep the bridge protocol-blind: it must not decode MessagePack or trust frame lengths itself. Build SSH commands with direct argv and a fixed remote command, preserve normal SSH config aliases and ProxyJump behavior, reserve stdout exclusively for protocol bytes, and send diagnostics to stderr. Prefer a private temporary local Unix relay so the existing Framed<UnixStream> client can be reused for the first release. Clean up only bridge sockets owned by the current process and reliably terminate SSH children on detach or cancellation.

## Acceptance Criteria

A test can place the bridge between a Fut client and daemon and exercise handshake, input, screen frames, resize, and clean detach; arbitrary bytes are forwarded unchanged in both directions; bridge sockets are private and cleaned safely; abrupt EOF tears down only the attachment and leaves the remote daemon and panes running; the daemon still has no network listener.
