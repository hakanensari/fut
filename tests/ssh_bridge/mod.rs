mod remote;

use super::*;
use std::os::fd::OwnedFd;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn spawn_bridge(socket: &std::path::Path) -> (Connection, tokio::process::Child) {
    let (local, remote) = std::os::unix::net::UnixStream::pair().unwrap();
    local.set_nonblocking(true).unwrap();
    let child = tokio::process::Command::new(env!("CARGO_BIN_EXE_fut"))
        .args(["--socket"])
        .arg(socket)
        .arg("__stdio-bridge")
        .env("FUT_SOCKET", "/ignored/explicit-socket-wins")
        .env("FUT_CONFIG", "relative-invalid-config-is-irrelevant")
        .stdin(Stdio::from(OwnedFd::from(remote.try_clone().unwrap())))
        .stdout(Stdio::from(OwnedFd::from(remote)))
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    (Connection::new(UnixStream::from_std(local).unwrap()), child)
}

#[tokio::test]
async fn stdio_bridge_preserves_protocol_and_daemon_across_detach_and_eof() {
    let mut harness = Harness::start("printf 'BRIDGE_READY\\r\\n'; while IFS= read -r line; do printf 'ANSWER:%s\\r\\n' \"$line\"; done").await;
    let mut identity = None;
    for abrupt in [false, true] {
        let (mut connection, mut bridge) = spawn_bridge(&harness.socket);
        let welcome = hello(&mut connection, interactive_mode(None), PROTOCOL_VERSION)
            .await
            .unwrap();
        let ServerMessage::Welcome {
            selected: Some(selected),
            ..
        } = welcome
        else {
            panic!("{welcome:?}")
        };
        let terminal = selected.focused.terminal_id;
        let pid = selected.focused.child_pid;
        harness.terminal_pid = Some(pid);
        if let Some(expected) = identity {
            assert_eq!((terminal, pid), expected);
        }
        identity = Some((terminal, pid));
        snapshot_containing(&mut connection, terminal, "BRIDGE_READY").await;
        send(
            &mut connection,
            ClientMessage::Input {
                bytes: b"hello\n".to_vec(),
            },
        )
        .await;
        snapshot_containing(&mut connection, terminal, "ANSWER:hello").await;
        let size = TerminalSize {
            columns: 93,
            rows: 31,
        };
        send(
            &mut connection,
            ClientMessage::Resize {
                terminal_id: terminal,
                size,
            },
        )
        .await;
        snapshot_with_size(&mut connection, terminal, size).await;
        if !abrupt {
            harness.detach(&mut connection).await;
        }
        drop(connection);
        assert!(
            time::timeout(DEADLINE, bridge.wait())
                .await
                .unwrap()
                .unwrap()
                .success()
        );
        let mut diagnostics = String::new();
        bridge
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut diagnostics)
            .await
            .unwrap();
        assert!(diagnostics.is_empty(), "{diagnostics}");
        assert!(process_alive(pid));
        assert!(matches!(
            harness.control_command(ClientMessage::Ping).await,
            ServerMessage::Pong { .. }
        ));
        assert!(harness.socket.exists());
    }
    let (mut connection, terminal, pid) = harness.interactive().await;
    assert_eq!(Some((terminal, pid)), identity);
    harness.detach(&mut connection).await;
    drop(connection);
    harness.shutdown().await;
}

#[tokio::test]
async fn stdio_bridge_forwards_arbitrary_bytes_with_backpressure_and_exits_on_socket_eof() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("raw.sock");
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    let mut bridge = tokio::process::Command::new(env!("CARGO_BIN_EXE_fut"))
        .arg("__stdio-bridge")
        .env("FUT_SOCKET", &path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let (socket, _) = time::timeout(DEADLINE, listener.accept())
        .await
        .unwrap()
        .unwrap();
    let (mut socket_read, mut socket_write) = socket.into_split();
    let mut input = bridge.stdin.take().unwrap();
    let mut output = bridge.stdout.take().unwrap();
    // Includes invalid frame prefixes, NUL, all byte values, and many pump buffers.
    let upstream: Vec<u8> = (0..512 * 1024).map(|i| (i % 256) as u8).collect();
    let downstream: Vec<u8> = upstream.iter().rev().copied().collect();
    time::timeout(DEADLINE, async {
        tokio::join!(
            async {
                input.write_all(&upstream).await.unwrap();
            },
            async {
                socket_write.write_all(&downstream).await.unwrap();
            },
            async {
                let mut received = vec![0; upstream.len()];
                socket_read.read_exact(&mut received).await.unwrap();
                assert_eq!(received, upstream);
            },
            async {
                let mut received = vec![0; downstream.len()];
                output.read_exact(&mut received).await.unwrap();
                assert_eq!(received, downstream);
            },
        );
    })
    .await
    .unwrap();
    drop(socket_read);
    drop(socket_write);
    // Stdin remains open: socket EOF must not strand a blocking stdin reader.
    assert!(
        time::timeout(DEADLINE, bridge.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
    let mut extra = Vec::new();
    output.read_to_end(&mut extra).await.unwrap();
    assert!(extra.is_empty());
    assert!(path.exists(), "bridge must never unlink the daemon socket");
    drop(input);
}

#[tokio::test]
async fn stdio_bridge_errors_only_on_stderr_and_does_not_autostart() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("absent.sock");
    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_fut"))
        .arg("--socket")
        .arg(&path)
        .args(["--json", "__stdio-bridge"])
        .output()
        .await
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("connect bridge"));
    assert!(!path.exists());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    let help = tokio::process::Command::new(env!("CARGO_BIN_EXE_fut"))
        .arg("--help")
        .output()
        .await
        .unwrap();
    assert!(!String::from_utf8_lossy(&help.stdout).contains("__stdio-bridge"));
}

#[tokio::test]
async fn stdio_bridge_stdin_eof_closes_only_the_connection() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("eof.sock");
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    let mut bridge = tokio::process::Command::new(env!("CARGO_BIN_EXE_fut"))
        .arg("--socket")
        .arg(&path)
        .arg("__stdio-bridge")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let (mut socket, _) = time::timeout(DEADLINE, listener.accept())
        .await
        .unwrap()
        .unwrap();
    drop(bridge.stdin.take());
    time::timeout(DEADLINE, async {
        assert_eq!(socket.read(&mut [0]).await.unwrap(), 0);
        assert!(bridge.wait().await.unwrap().success());
    })
    .await
    .unwrap();
    assert!(path.exists());
    // The server's listening socket is still usable after the bridge exits.
    let connection = UnixStream::connect(&path).await.unwrap();
    assert!(listener.accept().await.is_ok());
    drop(connection);
}
