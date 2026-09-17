use super::*;

fn fake_ssh(root: &std::path::Path) -> PathBuf {
    let bin = root.join("fake-bin");
    fs::create_dir(&bin).unwrap();
    let ssh = bin.join("ssh");
    fs::write(
        &ssh,
        r#"#!/bin/sh
printf '%s\n' "$@" >> "$SSH_ARGS"
printf '%s\n' "$$" >> "$SSH_PIDS"
if [ -n "$SSH_STALL" ]; then exec /bin/sleep 60; fi
if [ -n "$SSH_FAILURE" ]; then
    printf '%s\n' "$SSH_FAILURE" >&2
    exit 255
fi
exec "$FUT_BIN" --socket "$REMOTE_SOCKET" __stdio-bridge
"#,
    )
    .unwrap();
    fs::set_permissions(&ssh, fs::Permissions::from_mode(0o755)).unwrap();
    bin
}

fn remote_env(
    command: &mut Command,
    root: &std::path::Path,
    bin: &std::path::Path,
    socket: &std::path::Path,
) {
    command
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("HOME", root)
        .env("TERM", "xterm-256color")
        .env("FUT_BIN", env!("CARGO_BIN_EXE_fut"))
        .env("REMOTE_SOCKET", socket)
        .env("SSH_ARGS", root.join("ssh-args"))
        .env("SSH_PIDS", root.join("ssh-pids"))
        // These local values must never select the remote endpoint or cause IO.
        .env("FUT_SOCKET", root.join("must-not-be-used.sock"))
        .env("FUT_RUNTIME_DIR", root.join("must-not-be-created"));
}

async fn assert_ssh_reaped(root: &std::path::Path) {
    let pids = fs::read_to_string(root.join("ssh-pids")).unwrap();
    time::timeout(DEADLINE, async {
        for pid in pids.lines().map(|pid| pid.parse::<u32>().unwrap()) {
            while process_alive(pid) {
                time::sleep(POLL_INTERVAL).await;
            }
        }
    })
    .await
    .expect("SSH bridge survived client exit");
}

#[tokio::test]
async fn remote_cli_navigator_attach_detach_preserves_daemon_and_reaps_ssh() {
    let harness = Harness::start_with("printf 'REMOTE_READY\\r\\n'; while IFS= read -r line; do printf 'REMOTE:%s\\r\\n' \"$line\"; done", |root| {
        let extension = root.join("remote-extension");
        fs::create_dir(&extension).unwrap();
        fs::write(extension.join("fut-extension.toml"), r#"
api_version = 1
version = "1.0.0"
fut = ">=0.7.0, <1.0.0"
id = "remote-only"
capabilities = ["hooks", "commands"]
[hooks]
"client.attached" = ["./run"]
"client.session_changed" = ["./run"]
"client.detached" = ["./run"]
[commands.attack]
title = "Remote attack"
argv = ["./run"]
"#).unwrap();
        let script = extension.join("run");
        fs::write(&script, format!("#!/bin/sh\ntouch '{}'\n", root.join("client-hook-ran").display())).unwrap();
        fs::set_permissions(script, fs::Permissions::from_mode(0o755)).unwrap();
        fs::create_dir_all(root.join("home/.config/fut")).unwrap();
        fs::write(root.join("home/.config/fut/config.toml"), format!("extensions = [{:?}]\n", extension.to_str().unwrap())).unwrap();
    }).await;
    let root = tempfile::tempdir().unwrap();
    let bin = fake_ssh(root.path());
    let before = harness.resources().await;
    for tail in ["", "attach"] {
        let mut command = Command::new("/usr/bin/script");
        remote_env(&mut command, root.path(), &bin, &harness.socket);
        command.args(script_command_args()).arg(format!(
            "stty cols 80 rows 24; exec \"$FUT_BIN\" --no-config --remote clonk {tail}"
        ));
        let mut client = PtyChild::spawn(command);
        client.wait_for("navigator").await;
        client.send(b"\r");
        client.wait_for("REMOTE_READY").await;
        client.send(b"ping\r");
        client.wait_for("REMOTE:ping").await;
        // All of these used to resolve workspace roots/config in the client.
        client.send(b"\x02S");
        client.wait_for("project opener unavailable").await;
        client.send(b"\x02:");
        client.send(b"Remote attack");
        client.send(b"\r");
        client.wait_for("commands unavailable").await;
        client.send(b"\x02d");
        client.wait_success().await;
        assert_ssh_reaped(root.path()).await;
        assert!(!harness.root.path().join("client-hook-ran").exists());
        assert_eq!(
            without_observations(harness.resources().await),
            without_observations(before.clone())
        );
        assert!(!root.path().join("must-not-be-created").exists());
    }
    let argv = fs::read_to_string(root.path().join("ssh-args")).unwrap();
    assert_eq!(argv, "-T\n--\nclonk\nfut __stdio-bridge\n".repeat(4));
    assert!(matches!(
        harness.control_command(ClientMessage::Ping).await,
        ServerMessage::Pong { .. }
    ));
    harness.shutdown().await;
}

#[tokio::test]
async fn remote_failures_leave_terminal_untouched_and_reap_ssh() {
    for failure in [
        "Permission denied (publickey).",
        "Host key verification failed.",
        "fut: command not found",
    ] {
        assert_remote_failure(Some(failure)).await;
    }
}

#[tokio::test]
async fn remote_missing_daemon_fails_concisely_without_terminal_setup_and_reaps_ssh() {
    assert_remote_failure(None).await;
}

async fn assert_remote_failure(failure: Option<&str>) {
    let root = tempfile::tempdir().unwrap();
    let bin = fake_ssh(root.path());
    let socket = root.path().join("missing.sock");
    let mut command = Command::new("/usr/bin/script");
    remote_env(&mut command, root.path(), &bin, &socket);
    command
        .env("SSH_FAILURE", failure.unwrap_or_default())
        .args(script_command_args())
        .arg(
            r#"
before=$(stty -g)
"$FUT_BIN" --no-config --remote clonk
code=$?
after=$(stty -g)
[ "$before" = "$after" ] && printf 'TERM_UNCHANGED\n'
printf 'REMOTE_EXIT:%s\n' "$code"
"#,
        );
    let mut client = PtyChild::spawn(command);
    client.wait_success().await;
    // Wait for the asynchronous reader to drain the final pipe bytes.
    time::timeout(DEADLINE, async {
        while !client.text().contains("REMOTE_EXIT:1") {
            time::sleep(POLL_INTERVAL).await;
        }
    })
    .await
    .unwrap();
    assert!(
        client.text().contains("TERM_UNCHANGED"),
        "{}",
        client.text()
    );
    assert!(
        !client.text().contains('\x1b'),
        "terminal setup occurred before failed handshake"
    );
    let output = client.text();
    assert!(output.contains("remote attachment failed"), "{output}");
    if let Some(failure) = failure {
        assert!(output.contains(failure), "{output}");
    } else {
        assert!(output.contains("connect bridge to"), "{output}");
        assert!(output.contains("missing.sock"), "{output}");
        assert!(output.contains("No such file or directory"), "{output}");
        assert!(
            output.lines().count() <= 6,
            "failure was not concise: {output}"
        );
        assert!(!output.contains("panicked"), "{output}");
        assert!(!output.contains("shutdown"), "{output}");
    }
    assert!(!socket.exists());
    assert!(!root.path().join("must-not-be-created").exists());
    assert_ssh_reaped(root.path()).await;
}

#[tokio::test]
async fn remote_cli_protocol_mismatch_never_retries_or_changes_terminal() {
    let root = tempfile::tempdir().unwrap();
    let bin = fake_ssh(root.path());
    let socket = root.path().join("daemon.sock");
    let listener = tokio::net::UnixListener::bind(&socket).unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut connection = Connection::new(stream);
        let request = connection.next().await.unwrap().unwrap();
        let hello: Envelope<ClientMessage> = decode_payload(&request).unwrap();
        assert!(matches!(
            hello.message,
            ClientMessage::Hello {
                version: PROTOCOL_VERSION,
                mode: ClientMode::Control,
                ..
            }
        ));
        connection
            .send(Bytes::from(
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
        assert!(connection.next().await.is_none());
        assert!(
            time::timeout(Duration::from_millis(100), listener.accept())
                .await
                .is_err()
        );
    });
    let mut command = Command::new(env!("CARGO_BIN_EXE_fut"));
    remote_env(&mut command, root.path(), &bin, &socket);
    command.args(["--no-config", "--remote", "clonk"]);
    let output = tokio::task::spawn_blocking(move || command.output().unwrap())
        .await
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("incompatible protocol"), "{error}");
    assert!(error.contains("install a matching Fut version"), "{error}");
    assert!(error.contains("remote daemon"), "{error}");
    assert!(!error.contains("shutdown"));
    assert!(!error.contains('\x1b'));
    assert_ssh_reaped(root.path()).await;
    assert_eq!(
        fs::read_to_string(root.path().join("ssh-pids"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    server.await.unwrap();
}

#[test]
fn remote_cli_restrictions_precede_all_local_special_cases() {
    let root = tempfile::tempdir().unwrap();
    let bin = fake_ssh(root.path());
    for tail in [
        vec!["doctor"],
        vec!["trust", "status"],
        vec!["open", "/tmp"],
        vec!["daemon", "run"],
        vec!["attach", "--ignore-protocol-mismatch"],
        vec!["__stdio-bridge"],
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_fut"));
        remote_env(
            &mut command,
            root.path(),
            &bin,
            &root.path().join("missing.sock"),
        );
        let output = command
            .args(["--remote", "clonk"])
            .args(tail)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("--remote"));
    }
    assert!(!root.path().join("ssh-pids").exists());
    assert!(!root.path().join("must-not-be-created").exists());
}

#[tokio::test]
async fn remote_cancelled_handshake_reaps_ssh_without_terminal_setup() {
    let root = tempfile::tempdir().unwrap();
    let bin = fake_ssh(root.path());
    let mut command = Command::new(env!("CARGO_BIN_EXE_fut"));
    remote_env(
        &mut command,
        root.path(),
        &bin,
        &root.path().join("unused.sock"),
    );
    command
        .env("SSH_STALL", "1")
        .args(["--no-config", "--remote", "clonk"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = command.spawn().unwrap();
    wait_for_nonempty_file(&root.path().join("ssh-pids")).await;
    // SAFETY: this PID belongs to the child created above; only the client is
    // signalled, so SSH cleanup must be performed by the client itself.
    assert_eq!(unsafe { libc::kill(child.id() as _, libc::SIGTERM) }, 0);
    let output = time::timeout(
        DEADLINE,
        tokio::task::spawn_blocking(move || child.wait_with_output().unwrap()),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("terminated by SIGTERM"));
    assert_ssh_reaped(root.path()).await;
}

#[test]
fn remote_nested_client_guard_precedes_ssh_attachment() {
    let root = tempfile::tempdir().unwrap();
    let bin = fake_ssh(root.path());
    for tail in [vec![], vec!["attach"]] {
        for allow_nested in [false, true] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_fut"));
            remote_env(
                &mut command,
                root.path(),
                &bin,
                &root.path().join("missing.sock"),
            );
            command
                .env("FUT_TERMINAL_ID", "existing-terminal")
                .env("SSH_FAILURE", "nested override reached SSH");
            if allow_nested {
                command.env("FUT_ALLOW_NESTED", "1");
            }
            let output = command
                .args(["--no-config", "--remote", "clonk"])
                .args(&tail)
                .output()
                .unwrap();
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
            let error = String::from_utf8_lossy(&output.stderr);
            if allow_nested {
                assert!(error.contains("nested override reached SSH"), "{error}");
                assert!(!error.contains("clients should be nested"), "{error}");
                fs::remove_file(root.path().join("ssh-pids")).unwrap();
                fs::remove_file(root.path().join("ssh-args")).unwrap();
            } else {
                assert!(
                    error.contains("clients should be nested with care"),
                    "{error}"
                );
                assert!(error.contains("FUT_ALLOW_NESTED"), "{error}");
                assert!(!root.path().join("ssh-pids").exists());
                assert!(!root.path().join("ssh-args").exists());
            }
            assert!(!root.path().join("must-not-be-created").exists());
        }
    }
}
