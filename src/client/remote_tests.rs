use super::tests::{linked_screen, selected_view, targets};
use super::*;

#[tokio::test]
async fn remote_locality_blocks_coincident_paths_pids_commands_forms_projects_and_hooks() {
    use crate::resources::{
        PaneSnapshot, Project, ProjectIdentity, SessionSnapshot, TabSnapshot, WorkspaceSnapshot,
    };
    let root = tempfile::tempdir().unwrap();
    let extension = root.path().join("extension");
    fs::create_dir(&extension).unwrap();
    let marker = root.path().join("executed");
    use std::os::unix::fs::PermissionsExt;
    let script = extension.join("run");
    fs::write(
        &script,
        format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(
        extension.join("fut-extension.toml"),
        r#"
api_version = 1
version = '1.0.0'
fut = '>=0.7.0, <1.0.0'
id = 'attack'
capabilities = ['commands', 'hooks']
[hooks]
'client.attached' = ['./run']
'client.detached' = ['./run']
[commands.interactive]
title = 'Attack interactive'
argv = ['./run']
[commands.background]
title = 'Attack background'
argv = ['./run']
mode = 'background'
[commands.form]
title = 'Attack form'
argv = ['./run']
fields = [{ name = 'value', label = 'Value' }]
"#,
    )
    .unwrap();
    fs::write(root.path().join("config.toml"), format!(
        "extensions = [{:?}]\n[trusted_commands.local]\ntitle = 'Trusted local'\nprogram = {:?}\n",
        extension.to_str().unwrap(), script.to_str().unwrap(),
    )).unwrap();
    let location = config::resolve_location(Some(root.path())).unwrap();
    let loaded = config::load_extensions_location(&location).unwrap();
    let catalog = crate::extensions::ExtensionRegistry::new(1, loaded.extensions, loaded.config)
        .unwrap()
        .catalog()
        .unwrap();
    // Simulate different catalogs on the two machines, at coincident local paths.
    fs::write(
        extension.join("fut-extension.toml"),
        "not a valid local manifest",
    )
    .unwrap();
    let mut ui = stage_ui_config(&location)
        .unwrap()
        .materialize(&catalog)
        .unwrap();
    assert!(Attachment::Remote.client_hooks(&ui).unwrap().is_none());
    assert!(Attachment::Remote.local_socket().is_err());
    assert_eq!(
        Attachment::Local(root.path()).local_socket().unwrap(),
        root.path()
    );

    let mut target = targets(1).remove(0);
    target.child_pid = std::process::id(); // Real, inspectable local process.
    let snapshot = ResourceSnapshot {
        revision: 1,
        sessions: vec![SessionSnapshot {
            id: target.session_id,
            name: "remote".into(),
            project: Project {
                identity: ProjectIdentity::CanonicalDirectory(root.path().to_owned()),
            },
            trusted_project_config: None,
            closing: false,
            tokens: Default::default(),
            workspaces: vec![WorkspaceSnapshot {
                id: target.workspace_id,
                name: "remote".into(),
                root: root.path().to_owned(),
                closing: false,
                tokens: Default::default(),
                tabs: vec![TabSnapshot {
                    id: target.tab_id,
                    name: "remote".into(),
                    closing: false,
                    tokens: Default::default(),
                    layout: SplitTree::leaf(target.pane_id),
                    panes: vec![PaneSnapshot {
                        id: target.pane_id,
                        terminal_id: target.terminal_id,
                        closing: false,
                        tokens: Default::default(),
                        activity: Default::default(),
                        cwd: None,
                        worktree: None,
                    }],
                }],
            }],
        }],
    };
    let mut resources = ResourceState::default();
    resources.accept(snapshot);
    let mut view = ViewState::new(
        Locality::Remote,
        selected_view(1, target.clone(), vec![target.clone()]),
    )
    .unwrap();
    let (stream, peer) = UnixStream::pair().unwrap();
    let mut framed = Framed::new(stream, codec());
    let mut peer = Framed::new(peer, codec());
    let (background, mut results) = mpsc::unbounded_channel();
    let mut surface = None;
    let mut temporary = None;
    let mut focus = FocusState::default();
    let mut reload = None;
    let mut project_reload = None;
    for action in [
        ClientAction::RunCommand(0),
        ClientAction::RunCommand(1),
        ClientAction::RunCommand(2),
        ClientAction::RunCommand(3),
        ClientAction::OpenProject,
        ClientAction::ReloadConfig,
        ClientAction::ReloadProjectConfig,
    ] {
        let toast = dispatch_client_action(
            action,
            &mut framed,
            &mut view,
            &resources,
            &mut surface,
            &NavigationHistory::default(),
            &mut CreateCoordinator::default(),
            &mut CloseTargetState::default(),
            &mut focus,
            &mut None,
            &mut None,
            Rect::new(0, 0, 80, 24),
            &mut ui,
            &mut temporary,
            Attachment::Remote,
            &background,
            &location,
            &mut reload,
            &mut project_reload,
            false,
        )
        .await
        .unwrap();
        assert!(
            matches!(toast, Some(Toast::Info(message)) if message.contains("unavailable during remote attachment"))
        );
        assert!(
            surface.is_none()
                && temporary.is_none()
                && reload.is_none()
                && project_reload.is_none()
        );
    }
    for command in ["interactive", "background", "form"] {
        let toast = dispatch_presentation_token_action(
            PresentationTokenInvocation {
                target: PresentationTokenTarget::Pane(target.pane_id),
                action: PresentationTokenAction::ExtensionCommand {
                    extension_id: "attack".into(),
                    command: command.into(),
                },
            },
            &mut framed,
            &mut view,
            &resources,
            &mut surface,
            &mut focus,
            Rect::new(0, 0, 80, 24),
            &ui,
            &mut temporary,
            Attachment::Remote,
            &background,
        )
        .await
        .unwrap();
        assert!(
            matches!(toast, Some(Toast::Info(message)) if message.contains("commands unavailable"))
        );
    }
    assert!(!marker.exists());
    assert!(results.try_recv().is_err());
    assert!(
        time::timeout(Duration::from_millis(20), peer.next())
            .await
            .is_err(),
        "blocked actions must not send protocol requests"
    );

    // Pane token actions still use the protocol without local process access.
    assert!(
        dispatch_presentation_token_action(
            PresentationTokenInvocation {
                target: PresentationTokenTarget::Pane(target.pane_id),
                action: PresentationTokenAction::Pane {
                    pane_id: target.pane_id
                }
            },
            &mut framed,
            &mut view,
            &resources,
            &mut surface,
            &mut focus,
            Rect::new(0, 0, 80, 24),
            &ui,
            &mut temporary,
            Attachment::Remote,
            &background,
        )
        .await
        .unwrap()
        .is_none()
    );
    let message: Envelope<ClientMessage> =
        decode_payload(&peer.next().await.unwrap().unwrap()).unwrap();
    assert!(
        matches!(message.message, ClientMessage::SelectTarget { selector: TargetSelector::Pane(id), .. } if id == target.pane_id)
    );
}

#[test]
fn remote_link_policy_applies_at_rendering_and_preserves_local_links() {
    for uri in [
        "file:///tmp/existing",
        "file://localhost/etc/passwd",
        "vscode://file/tmp/existing",
        "fut://pane/local",
        "/tmp/existing",
        "javascript:alert(1)",
        "https://example.com\x1b]8;;file:///tmp/x",
        "http://",
    ] {
        let screen = linked_screen("x", uri);
        let area = Rect::new(0, 0, 1, 1);
        let mut rendered = Buffer::empty(area);
        Screen(&screen, Locality::Remote).render(area, &mut rendered);
        assert_eq!(rendered[(0, 0)].symbol(), "x", "{uri}");
    }
    for uri in [
        "https://example.com",
        "http://example.com",
        "HTTPS://example.com",
    ] {
        let screen = linked_screen("x", uri);
        let area = Rect::new(0, 0, 1, 1);
        let mut rendered = Buffer::empty(area);
        Screen(&screen, Locality::Remote).render(area, &mut rendered);
        assert!(rendered[(0, 0)].symbol().contains(uri));
    }
    assert!(
        bench::render_snapshot(&linked_screen("x", "file:///tmp/existing"))[(0, 0)]
            .symbol()
            .contains("file:")
    );
}

#[tokio::test]
async fn remote_control_rejects_unsafe_daemon_version_text() {
    let (stream, peer) = UnixStream::pair().unwrap();
    let server = tokio::spawn(async move {
        let mut peer = Framed::new(peer, codec());
        let hello: Envelope<ClientMessage> =
            decode_payload(&peer.next().await.unwrap().unwrap()).unwrap();
        peer.send(Bytes::from(
            encode_payload(&Envelope {
                request_id: hello.request_id,
                message: ServerMessage::Welcome {
                    version: PROTOCOL_VERSION,
                    server_version: "0.22.0\u{1b}[2J".into(),
                    selected: None,
                    extension_catalog: crate::protocol::ExtensionCatalog {
                        generation: 1,
                        fingerprint: String::new(),
                        extensions: Vec::new(),
                        config: Default::default(),
                    },
                },
            })
            .unwrap(),
        ))
        .await
        .unwrap();
    });
    let error = hello_control(
        stream,
        PROTOCOL_VERSION,
        Duration::from_secs(1),
        Locality::Remote,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("invalid version string"));
    server.await.unwrap();
}

#[tokio::test]
async fn handshakes_reject_mismatch_with_locality_specific_guidance() {
    for locality in [Locality::Local, Locality::Remote] {
        for interactive in [false, true] {
            let (stream, peer) = UnixStream::pair().unwrap();
            let server = tokio::spawn(async move {
                let mut peer = Framed::new(peer, codec());
                let hello: Envelope<ClientMessage> =
                    decode_payload(&peer.next().await.unwrap().unwrap()).unwrap();
                assert!(matches!(
                    hello.message,
                    ClientMessage::Hello {
                        version: PROTOCOL_VERSION,
                        ..
                    }
                ));
                peer.send(Bytes::from(
                    encode_payload(&Envelope {
                        request_id: hello.request_id,
                        message: ServerMessage::IncompatibleProtocol {
                            client: PROTOCOL_VERSION,
                            server: PROTOCOL_VERSION - 1,
                        },
                    })
                    .unwrap(),
                ))
                .await
                .unwrap();
                assert!(peer.next().await.is_none());
            });
            let error = if interactive {
                handshake_interactive(
                    stream,
                    None,
                    TerminalSize {
                        columns: 80,
                        rows: 24,
                    },
                    crate::domain::ClientId::new(),
                    PROTOCOL_VERSION,
                    Duration::from_secs(1),
                    locality,
                )
                .await
                .unwrap_err()
            } else {
                handshake_navigator(stream, PROTOCOL_VERSION, Duration::from_secs(1), locality)
                    .await
                    .unwrap_err()
            };
            assert!(error.to_string().contains("incompatible protocol"));
            let message = error.to_string();
            match locality {
                Locality::Local => {
                    assert!(message.contains("fut daemon shutdown --force"), "{message}");
                    assert!(message.contains("local daemon"), "{message}");
                    assert!(!message.contains("install a matching"));
                }
                Locality::Remote => {
                    assert!(
                        message.contains("install a matching Fut version"),
                        "{message}"
                    );
                    assert!(message.contains("remote daemon"), "{message}");
                    assert!(!message.contains("shutdown"));
                }
            }
            server.await.unwrap();
        }
    }
}
