//! The UI mode end to end: the binary hosting the editor over its own
//! linked nodes. A launch with a file seeds the held definition through
//! the story 03 loader, and a launch without one starts empty and
//! untitled; a file the launch cannot read or load ends the process with
//! the fault on the error stream and no server. A run started from the
//! browser prints the values its graph leaves unconnected — the shipped
//! graph's classic sequence, one line per value, as they arrive — and
//! printing stops with the run. The exit guard walks its paths: a clean
//! quit answers the first interrupt, unsaved changes stand down with a
//! warning while the session carries on, and the second interrupt quits.
//!
//! The tests take turns over the one loopback address the binary serves:
//! the turn is a std lock guarding the *process*, not the runtime, so its
//! guard rides the whole session across every await.
#![allow(clippy::await_holding_lock)]

mod common;

use std::io::{BufRead, Read};
use std::net::SocketAddr;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use common::{fizzbuzz_line, long_counter};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::mpsc::UnboundedReceiver;

use nodetool::server::DEFAULT_ADDRESS;

const GRAPH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/graphs/fizzbuzz.yml");

/// How long the tests wait on a value they expect to arrive: the shipped
/// run is a burst long past finished inside it, so only a run printing as
/// values arrive can meet the bound.
const ARRIVAL: Duration = Duration::from_secs(10);

/// How long a quit may take once the guard decides, and how long a
/// silence must hold before a test reads it as the printing's end.
const QUIT: Duration = Duration::from_secs(10);
const SILENCE: Duration = Duration::from_millis(500);

/// One editor server at a time: the binary serves its loopback default
/// address, so the tests that launch it take turns.
static SERVER: Mutex<()> = Mutex::new(());

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_nodetool-fizzbuzz"))
}

/// The handshake one browser sends: the RFC 6455 example's fixed key, the
/// answer to which the server gives in the upgrade head.
const HANDSHAKE: &[u8] =
    b"GET /ws HTTP/1.1\r\nHost: editor\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n";

/// One launched editor: the process, its served address, the protocol
/// over one websocket, and the lines it prints. The turn rides along —
/// the binary serves its fixed address, so the tests take turns — and the
/// child is killed on the way out, so no test leaves a server behind.
struct Session {
    child: Child,
    address: SocketAddr,
    writer: Option<WriteHalf<TcpStream>>,
    frames: Option<UnboundedReceiver<String>>,
    lines: UnboundedReceiver<String>,
    _turn: MutexGuard<'static, ()>,
}

impl Session {
    /// Launch `--ui` and join its announcement: the printed address is
    /// both the launch's word and the proof the server is up.
    async fn launch(args: &[&str]) -> Session {
        let turn = SERVER
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut child = binary()
            .arg("--ui")
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the binary runs");
        let stdout = child.stdout.take().expect("stdout is piped");
        let (lines_tx, mut lines) = tokio::sync::mpsc::unbounded_channel();
        std::thread::spawn(move || {
            let reader = std::io::BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        if lines_tx.send(line).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        let announcement = tokio::time::timeout(ARRIVAL, lines.recv())
            .await
            .expect("the launch announces itself")
            .expect("the reader lives");
        let address: SocketAddr = announcement
            .strip_prefix("fizzbuzz editor on http://")
            .expect("the announcement names the served address")
            .parse()
            .expect("the announcement carries the address");
        Session {
            child,
            address,
            writer: None,
            frames: None,
            lines,
            _turn: turn,
        }
    }

    /// One websocket: the handshake, then the frame pump running beside
    /// the test, so every push the server sends is gathered as it comes
    /// and no connection drowns in its own broadcast.
    async fn browser(&mut self) {
        let mut stream = TcpStream::connect(self.address)
            .await
            .expect("the editor is serving");
        stream
            .write_all(HANDSHAKE)
            .await
            .expect("the handshake is written");
        let mut buffer = Vec::new();
        loop {
            if let Some(end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&buffer[..end]).into_owned();
                assert!(
                    head.starts_with("HTTP/1.1 101"),
                    "the upgrade answer: {head}"
                );
                buffer.drain(..end + 4);
                break;
            }
            let mut chunk = [0u8; 1024];
            let read = stream.read(&mut chunk).await.expect("the socket reads");
            assert!(read > 0, "the connection closed before the upgrade");
            buffer.extend_from_slice(&chunk[..read]);
        }
        let (reader, writer) = tokio::io::split(stream);
        let (frames_tx, frames) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(frame_pump(FrameSource { reader, buffer }, frames_tx));
        self.writer = Some(writer);
        self.frames = Some(frames);
    }

    /// Send one protocol message and await its reply, the pushes riding
    /// past — the greeting first of all.
    async fn tell(&mut self, id: u64, message: Value) -> Value {
        let writer = self.writer.as_mut().expect("the browser is open");
        write_frame(writer, &message.to_string()).await;
        let frames = self.frames.as_mut().expect("the browser is open");
        loop {
            let frame = tokio::time::timeout(ARRIVAL, frames.recv())
                .await
                .expect("the reply arrives")
                .expect("the pump lives");
            let reply: Value = serde_json::from_str(&frame).expect("every message is JSON");
            if reply["id"] == json!(id) {
                return reply;
            }
        }
    }

    /// The next run-finished push: the run state gone idle with its
    /// outcome.
    async fn run_ending(&mut self) -> Value {
        let frames = self.frames.as_mut().expect("the browser is open");
        loop {
            let frame = tokio::time::timeout(ARRIVAL, frames.recv())
                .await
                .expect("the ending arrives")
                .expect("the pump lives");
            let pushed: Value = serde_json::from_str(&frame).expect("a push is JSON");
            if pushed["type"] == "run" && pushed["running"] == json!(false) {
                return pushed;
            }
        }
    }

    /// The next printed line.
    async fn line(&mut self) -> String {
        tokio::time::timeout(ARRIVAL, self.lines.recv())
            .await
            .expect("a line arrives")
            .expect("the reader lives")
    }

    /// The next printed line, if one arrives inside `limit`.
    async fn line_within(&mut self, limit: Duration) -> Option<String> {
        tokio::time::timeout(limit, self.lines.recv())
            .await
            .ok()
            .flatten()
    }

    /// The lines already printed, before a silence is judged.
    fn drain_lines(&mut self) {
        while self.lines.try_recv().is_ok() {}
    }

    /// The user's Ctrl-C: a real SIGINT to the process, as a terminal
    /// delivers it.
    fn interrupt(&mut self) {
        Command::new("kill")
            .args(["-INT", &self.child.id().to_string()])
            .output()
            .expect("the interrupt is sent");
    }

    /// Whether the process is still serving, a pause after the interrupt
    /// giving a prompt quit the time to happen first.
    async fn still_running(&mut self) -> bool {
        tokio::time::sleep(Duration::from_millis(400)).await;
        self.child
            .try_wait()
            .expect("the exit status is readable")
            .is_none()
    }

    /// The exit status once the process ends, bounded — a quit the guard
    /// decided arrives promptly; a hung one fails the test, not the suite.
    async fn exited(&mut self) -> std::process::ExitStatus {
        let deadline = Instant::now() + QUIT;
        loop {
            if let Some(status) = self.child.try_wait().expect("the exit status is readable") {
                return status;
            }
            assert!(Instant::now() < deadline, "the editor did not quit");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// Whatever reached the error stream, read after the process ended.
    fn stderr_text(&mut self) -> String {
        let mut stderr = String::new();
        self.child
            .stderr
            .take()
            .expect("stderr is piped")
            .read_to_string(&mut stderr)
            .expect("stderr is readable");
        stderr
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The server's half of the socket read one text frame at a time: the
/// greeting, the replies, and the pushes ride unmasked text frames, a
/// close ending the pump.
struct FrameSource {
    reader: ReadHalf<TcpStream>,
    buffer: Vec<u8>,
}

impl FrameSource {
    /// More bytes from the socket, or nothing once the server closes it.
    async fn fill(&mut self) -> Option<()> {
        let mut chunk = [0u8; 8192];
        let read = self.reader.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        self.buffer.extend_from_slice(&chunk[..read]);
        Some(())
    }

    async fn next_text(&mut self) -> Option<String> {
        loop {
            if self.buffer.len() < 2 {
                self.fill().await?;
                continue;
            }
            let (header, payload) = match self.buffer[1] & 0x7f {
                length @ 0..=125 => (2usize, length as usize),
                126 if self.buffer.len() >= 4 => (
                    4,
                    u16::from_be_bytes([self.buffer[2], self.buffer[3]]) as usize,
                ),
                127 if self.buffer.len() >= 10 => (
                    10,
                    usize::try_from(u64::from_be_bytes(
                        self.buffer[2..10].try_into().expect("eight bytes"),
                    ))
                    .expect("the frame fits memory"),
                ),
                _ => {
                    self.fill().await?;
                    continue;
                }
            };
            if self.buffer.len() < header + payload {
                self.fill().await?;
                continue;
            }
            let frame: Vec<u8> = self.buffer.drain(..header + payload).collect();
            match frame[0] & 0x0f {
                0x1 => {
                    return Some(
                        String::from_utf8(frame[header..].to_vec())
                            .expect("the protocol carries utf-8"),
                    )
                }
                0x8 => return None,
                _ => {}
            }
        }
    }
}

async fn frame_pump(mut source: FrameSource, frames: tokio::sync::mpsc::UnboundedSender<String>) {
    while let Some(text) = source.next_text().await {
        if frames.send(text).is_err() {
            break;
        }
    }
}

/// One masked client frame: the mask is zeros, so the payload rides as
/// itself — the server unmasks to the same bytes.
async fn write_frame(writer: &mut WriteHalf<TcpStream>, message: &str) {
    let payload = message.as_bytes();
    let mut frame = vec![0x81];
    match payload.len() {
        length @ 0..=125 => frame.push(0x80 | length as u8),
        length @ 126..=65_535 => {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(length as u16).to_be_bytes());
        }
        length => {
            frame.push(0x80 | 127);
            frame.extend_from_slice(&(length as u64).to_be_bytes());
        }
    }
    frame.extend_from_slice(&[0, 0, 0, 0]);
    frame.extend_from_slice(payload);
    writer
        .write_all(&frame)
        .await
        .expect("the frame is written");
}

#[tokio::test]
async fn a_launch_with_a_file_seeds_the_held_definition() {
    let mut session = Session::launch(&[GRAPH]).await;
    session.browser().await;

    let definition = session
        .tell(1, json!({ "id": 1, "type": "get_definition" }))
        .await;
    assert_eq!(
        definition["file"],
        json!({ "path": GRAPH, "dirty": false }),
        "the launch names the file and starts clean"
    );
    let nodes = definition["graph"]["nodes"].as_array().expect("nodes");
    assert_eq!(nodes.len(), 4, "the shipped graph's four nodes");
    let types: Vec<&str> = nodes
        .iter()
        .map(|node| node["type_ref"].as_str().expect("a type_ref"))
        .collect();
    assert!(
        types.contains(&"fizzbuzz/counter") && types.contains(&"fizzbuzz/case"),
        "the graph loaded through the story 03 loader: {types:?}"
    );

    session.interrupt();
    assert_eq!(session.exited().await.code(), Some(0), "a clean quit");
}

#[tokio::test]
async fn a_launch_without_a_file_starts_empty_and_untitled() {
    let mut session = Session::launch(&[]).await;
    session.browser().await;

    let definition = session
        .tell(1, json!({ "id": 1, "type": "get_definition" }))
        .await;
    assert_eq!(definition["file"], json!({ "path": null, "dirty": false }));
    assert_eq!(definition["graph"]["nodes"], json!([]));

    session.interrupt();
    assert_eq!(session.exited().await.code(), Some(0));
}

#[tokio::test]
async fn a_launch_load_error_ends_non_zero_naming_the_fault_without_a_server() {
    let _turn = SERVER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let output = binary()
        .arg("--ui")
        .arg("/no/such/graph.yml")
        .output()
        .expect("the binary runs");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("/no/such/graph.yml"), "{stderr}");
    assert!(
        TcpStream::connect(DEFAULT_ADDRESS).await.is_err(),
        "no server started: the loopback default answers nothing"
    );

    let path = std::env::temp_dir().join("nodetool-fizzbuzz-ui-broken.yml");
    std::fs::write(&path, "schema_version: 1\nsurprise: true\n").expect("the file is written");
    let output = binary()
        .arg("--ui")
        .arg(&path)
        .output()
        .expect("the binary runs");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot open"), "{stderr}");
    assert!(stderr.contains("unknown field `surprise`"), "{stderr}");
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn an_editor_started_run_prints_the_shipped_sequence_as_it_arrives() {
    let mut session = Session::launch(&[GRAPH]).await;
    session.browser().await;

    let started = session
        .tell(1, json!({ "id": 1, "type": "start_run" }))
        .await;
    assert_eq!(started["type"], "run_started");

    let expected: Vec<String> = (1..=100).map(fizzbuzz_line).collect();
    for want in &expected {
        assert_eq!(
            &session.line().await,
            want,
            "one line per value, in count order"
        );
    }
    assert!(
        session.line_within(SILENCE).await.is_none(),
        "exactly one line per value, nothing held back to the end"
    );
    let ended = session.run_ending().await;
    assert_eq!(ended["outcome"], json!("completed"));

    session.interrupt();
    assert_eq!(session.exited().await.code(), Some(0));
    assert!(
        session.stderr_text().is_empty(),
        "nothing on the error stream"
    );
}

#[tokio::test]
async fn the_next_start_runs_what_was_last_edited() {
    let mut session = Session::launch(&[GRAPH]).await;
    session.browser().await;

    let edited = session
        .tell(
            1,
            json!({
                "id": 1,
                "type": "set_parameter",
                "uuid": "00000000-0000-0000-0000-100000000001",
                "input": "stop",
                "value": "20",
            }),
        )
        .await;
    assert_eq!(edited["type"], "parameter_set");
    session
        .tell(2, json!({ "id": 2, "type": "start_run" }))
        .await;

    let expected: Vec<String> = (1..=20).map(fizzbuzz_line).collect();
    for want in &expected {
        assert_eq!(&session.line().await, want, "the edited sequence");
    }
    assert!(
        session.line_within(SILENCE).await.is_none(),
        "twenty lines, no more"
    );
    let ended = session.run_ending().await;
    assert_eq!(ended["outcome"], json!("completed"));

    // The edit was never saved, so the quit is saved elsewhere first —
    // the save-as gesture — leaving nothing unsaved for the guard.
    let saved_elsewhere = std::env::temp_dir().join("nodetool-fizzbuzz-ui-twenty.yml");
    let saved = session
        .tell(
            3,
            json!({
                "id": 3,
                "type": "save_file",
                "path": saved_elsewhere.to_string_lossy(),
            }),
        )
        .await;
    assert_eq!(saved["type"], "file_saved");
    session.interrupt();
    assert_eq!(session.exited().await.code(), Some(0));
    let _ = std::fs::remove_file(&saved_elsewhere);
}

#[tokio::test]
async fn printing_starts_with_the_run_and_stops_with_it() {
    let path = long_counter("1000000000");
    let mut session = Session::launch(&[path.to_str().expect("a path")]).await;
    session.browser().await;

    let started = session
        .tell(1, json!({ "id": 1, "type": "start_run" }))
        .await;
    assert_eq!(started["type"], "run_started");
    assert_eq!(
        session.line_within(ARRIVAL).await.as_deref(),
        Some("1"),
        "the first line arrives as the value does — the run cannot end inside the bound"
    );

    session
        .tell(2, json!({ "id": 2, "type": "stop_run" }))
        .await;
    let ended = session.run_ending().await;
    assert_eq!(ended["outcome"], json!("stopped"));
    session.drain_lines();
    assert!(
        session.line_within(SILENCE).await.is_none(),
        "printing stopped with the run"
    );

    session.interrupt();
    assert_eq!(session.exited().await.code(), Some(0));
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn a_clean_quit_answers_the_first_interrupt() {
    let mut session = Session::launch(&[GRAPH]).await;

    session.interrupt();
    assert_eq!(session.exited().await.code(), Some(0));
    assert!(
        session.stderr_text().is_empty(),
        "nothing unsaved, nothing to warn about"
    );
}

#[tokio::test]
async fn unsaved_changes_stand_down_and_the_second_interrupt_quits() {
    let mut session = Session::launch(&[GRAPH]).await;
    session.browser().await;

    let edited = session
        .tell(
            1,
            json!({
                "id": 1,
                "type": "set_parameter",
                "uuid": "00000000-0000-0000-0000-100000000001",
                "input": "stop",
                "value": "25",
            }),
        )
        .await;
    assert_eq!(edited["type"], "parameter_set");

    session.interrupt();
    assert!(
        session.still_running().await,
        "the first interrupt stands down over unsaved changes"
    );
    let held = session
        .tell(2, json!({ "id": 2, "type": "get_definition" }))
        .await;
    assert_eq!(
        held["file"]["dirty"],
        json!(true),
        "the session carries on: the state is the server-known dirty one"
    );

    session.interrupt();
    assert_eq!(
        session.exited().await.code(),
        Some(0),
        "the second interrupt quits"
    );
    let stderr = session.stderr_text();
    assert!(stderr.contains("unsaved changes"), "{stderr}");
}
