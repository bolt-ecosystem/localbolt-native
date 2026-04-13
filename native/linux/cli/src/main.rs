//! localbolt-cli — Linux CLI helper for LocalBolt.
//!
//! Phase 1 Steam Deck / Linux validation artifact.
//! Manages bolt-daemon lifecycle, reads IPC events, and provides
//! interactive transfer control from the terminal.
//!
//! LOCALBOLT-LINUX-CLI-IMPL-1

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use bolt_app_core::ipc_client::{probe_readiness, ReadinessResult};
use bolt_app_core::ipc_transport::IpcStream;
use bolt_app_core::ipc_types::{
    Decision, IpcKind, IpcMessage, PairingDecisionPayload, PairingRequestPayload,
    TransferIncomingDecisionPayload, TransferIncomingRequestPayload, VersionHandshakePayload,
};
use bolt_app_core::platform;
use clap::{Parser, Subcommand};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DAEMON_VERSION: &str = "0.0.1";
const DEFAULT_WS_LISTEN: &str = "0.0.0.0:9557";

#[derive(Parser)]
#[command(
    name = "localbolt-cli",
    version,
    about = "LocalBolt CLI — Linux daemon companion"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to bolt-daemon binary
    #[arg(long, global = true)]
    daemon_path: Option<String>,

    /// IPC socket path
    #[arg(long, global = true)]
    socket_path: Option<String>,

    /// Data directory for daemon state
    #[arg(long, global = true)]
    data_dir: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start bolt-daemon as a background process
    Start {
        /// WS listen address
        #[arg(long, default_value = DEFAULT_WS_LISTEN)]
        ws_listen: String,

        /// Pairing policy (ask, allow, deny)
        #[arg(long, default_value = "ask")]
        pairing_policy: String,

        /// Stay in foreground and stream IPC events after starting
        #[arg(long)]
        watch: bool,
    },
    /// Check daemon readiness
    Status,
    /// Watch daemon IPC event stream
    Watch,
    /// Send a file to the connected peer
    Send {
        /// Path to the file to send
        path: String,
    },
    /// Stop a running daemon
    Stop,
}

fn resolve_socket_path(cli: &Cli) -> PathBuf {
    PathBuf::from(
        cli.socket_path
            .clone()
            .unwrap_or_else(platform::default_ipc_path),
    )
}

fn resolve_data_dir(cli: &Cli) -> String {
    cli.data_dir
        .clone()
        .unwrap_or_else(platform::default_data_dir)
}

fn resolve_daemon_path(cli: &Cli) -> String {
    if let Some(ref p) = cli.daemon_path {
        return p.clone();
    }
    // Search common locations
    let candidates = [
        // Adjacent to this binary
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("bolt-daemon")))
            .unwrap_or_default(),
        // System paths
        PathBuf::from("/usr/local/bin/bolt-daemon"),
        PathBuf::from("/usr/bin/bolt-daemon"),
        // User local
        dirs_home()
            .map(|h| h.join(".local/bin/bolt-daemon"))
            .unwrap_or_default(),
    ];
    for c in &candidates {
        if c.as_os_str().is_empty() {
            continue;
        }
        if c.exists() {
            return c.to_string_lossy().to_string();
        }
    }
    // Fallback: assume on PATH
    "bolt-daemon".to_string()
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Start {
            ws_listen,
            pairing_policy,
            watch,
        } => cmd_start(&cli, ws_listen, pairing_policy, *watch),
        Commands::Status => cmd_status(&cli),
        Commands::Watch => cmd_watch(&cli),
        Commands::Send { path } => cmd_send(&cli, path),
        Commands::Stop => cmd_stop(&cli),
    }
}

// ── Start ────────────────────────────────────────────────────

fn cmd_start(cli: &Cli, ws_listen: &str, pairing_policy: &str, watch: bool) {
    let socket_path = resolve_socket_path(cli);
    let data_dir = resolve_data_dir(cli);
    let daemon_bin = resolve_daemon_path(cli);

    // Check if daemon is already running
    if IpcStream::probe(&socket_path) {
        eprintln!(
            "[localbolt-cli] daemon already running at {}",
            socket_path.display()
        );
        if watch {
            do_watch(cli);
        }
        return;
    }

    // Ensure data directory exists
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        eprintln!("[localbolt-cli] failed to create data dir {data_dir}: {e}");
        std::process::exit(1);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&data_dir, std::fs::Permissions::from_mode(0o700));
    }

    println!("[localbolt-cli] starting bolt-daemon...");
    println!("  binary:    {daemon_bin}");
    println!("  ws-listen: {ws_listen}");
    println!("  socket:    {}", socket_path.display());
    println!("  data-dir:  {data_dir}");
    println!("  policy:    {pairing_policy}");

    let mut child = match Command::new(&daemon_bin)
        .args([
            "--mode",
            "ws-endpoint",
            "--ws-listen",
            ws_listen,
            "--socket-path",
            &socket_path.to_string_lossy(),
            "--data-dir",
            &data_dir,
            "--pairing-policy",
            pairing_policy,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[localbolt-cli] failed to start daemon: {e}");
            if e.kind() == std::io::ErrorKind::NotFound {
                eprintln!("[localbolt-cli] bolt-daemon not found at: {daemon_bin}");
                eprintln!("[localbolt-cli] install bolt-daemon or use --daemon-path");
            }
            std::process::exit(1);
        }
    };

    let pid = child.id();
    println!("[localbolt-cli] daemon started (PID {pid})");

    // Set up Ctrl+C handler
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();
    let _ = ctrlc::set_handler(move || {
        shutdown_clone.store(true, Ordering::SeqCst);
    });

    // Wait for daemon to become ready
    let start = Instant::now();
    let timeout = std::time::Duration::from_secs(10);
    loop {
        if shutdown.load(Ordering::SeqCst) {
            println!("\n[localbolt-cli] shutting down daemon (PID {pid})...");
            terminate_child(&mut child);
            return;
        }
        if start.elapsed() > timeout {
            eprintln!("[localbolt-cli] daemon did not become ready within 10s");
            terminate_child(&mut child);
            std::process::exit(1);
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
        if IpcStream::probe(&socket_path) {
            break;
        }
    }

    match probe_readiness(&socket_path, DAEMON_VERSION) {
        ReadinessResult::Ready {
            daemon_version,
            connected_peers,
        } => {
            println!(
                "[localbolt-cli] daemon ready (version {daemon_version}, peers: {connected_peers})"
            );
        }
        ReadinessResult::Incompatible { daemon_version } => {
            eprintln!(
                "[localbolt-cli] WARNING: daemon version {daemon_version} may be incompatible with CLI {VERSION}"
            );
        }
        ReadinessResult::Failed(reason) => {
            eprintln!("[localbolt-cli] readiness probe failed: {reason}");
            eprintln!("[localbolt-cli] daemon may still be starting — try `localbolt-cli status`");
        }
    }

    if watch {
        // Stay in foreground, stream events
        println!("[localbolt-cli] watching events (Ctrl+C to stop)...");
        println!("---");
        event_loop(&socket_path, &shutdown);
        println!("\n[localbolt-cli] shutting down daemon (PID {pid})...");
        terminate_child(&mut child);
    } else {
        println!("[localbolt-cli] daemon running in background (PID {pid})");
        println!("[localbolt-cli] use `localbolt-cli watch` to stream events");
        println!("[localbolt-cli] use `localbolt-cli stop` to shut down");
        // Detach — don't wait for child
        std::mem::forget(child);
    }
}

// ── Status ───────────────────────────────────────────────────

fn cmd_status(cli: &Cli) {
    let socket_path = resolve_socket_path(cli);

    if !IpcStream::probe(&socket_path) {
        println!(
            "[localbolt-cli] daemon not running (no socket at {})",
            socket_path.display()
        );
        std::process::exit(1);
    }

    match probe_readiness(&socket_path, DAEMON_VERSION) {
        ReadinessResult::Ready {
            daemon_version,
            connected_peers,
        } => {
            println!("[localbolt-cli] daemon ready");
            println!("  version: {daemon_version}");
            println!("  peers:   {connected_peers}");
            println!("  socket:  {}", socket_path.display());
        }
        ReadinessResult::Incompatible { daemon_version } => {
            println!("[localbolt-cli] daemon running but incompatible (version {daemon_version})");
            std::process::exit(1);
        }
        ReadinessResult::Failed(reason) => {
            println!("[localbolt-cli] daemon probe failed: {reason}");
            std::process::exit(1);
        }
    }
}

// ── Watch ────────────────────────────────────────────────────

fn cmd_watch(cli: &Cli) {
    let socket_path = resolve_socket_path(cli);

    if !IpcStream::probe(&socket_path) {
        eprintln!(
            "[localbolt-cli] daemon not running (no socket at {})",
            socket_path.display()
        );
        std::process::exit(1);
    }

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();
    let _ = ctrlc::set_handler(move || {
        shutdown_clone.store(true, Ordering::SeqCst);
    });

    println!("[localbolt-cli] connecting to daemon IPC...");
    event_loop(&socket_path, &shutdown);
    println!("\n[localbolt-cli] disconnected");
}

fn do_watch(cli: &Cli) {
    let socket_path = resolve_socket_path(cli);
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();
    let _ = ctrlc::set_handler(move || {
        shutdown_clone.store(true, Ordering::SeqCst);
    });
    println!("[localbolt-cli] watching events (Ctrl+C to stop)...");
    println!("---");
    event_loop(&socket_path, &shutdown);
}

fn event_loop(socket_path: &Path, shutdown: &Arc<AtomicBool>) {
    let stream = match IpcStream::connect(socket_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[localbolt-cli] IPC connect failed: {e}");
            return;
        }
    };

    // Set a short read timeout so we can check shutdown flag
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(1)));

    let mut writer = match stream.try_clone() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("[localbolt-cli] clone stream for writer: {e}");
            return;
        }
    };

    // Version handshake
    let handshake = IpcMessage::new_decision(
        "version.handshake",
        serde_json::to_value(VersionHandshakePayload {
            app_version: DAEMON_VERSION.to_string(),
        })
        .unwrap(),
    );
    if let Err(e) = write_message(&mut writer, &handshake) {
        eprintln!("[localbolt-cli] handshake write failed: {e}");
        return;
    }

    let reader = BufReader::new(stream);

    for line_result in reader.lines() {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut
                {
                    if shutdown.load(Ordering::SeqCst) {
                        break;
                    }
                    continue;
                }
                eprintln!("[localbolt-cli] IPC read error: {e}");
                break;
            }
        };

        if line.trim().is_empty() {
            continue;
        }

        let msg: IpcMessage = match serde_json::from_str(&line) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[localbolt-cli] parse error: {e}");
                continue;
            }
        };

        handle_event(&msg, &mut writer);
    }
}

fn handle_event(msg: &IpcMessage, writer: &mut IpcStream) {
    match msg.msg_type.as_str() {
        "version.status" => {
            let compatible = msg
                .payload
                .get("compatible")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let version = msg
                .payload
                .get("daemon_version")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            if compatible {
                println!("[daemon] version handshake OK (daemon {version})");
            } else {
                eprintln!("[daemon] version INCOMPATIBLE (daemon {version})");
            }
        }
        "daemon.status" => {
            let peers = msg
                .payload
                .get("connected_peers")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let version = msg
                .payload
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            println!("[daemon] status: version={version}, connected_peers={peers}");
        }
        "session.connected" => {
            let peer = msg
                .payload
                .get("remote_peer_id")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            println!("[session] peer connected: {}", &peer[..peer.len().min(12)]);
        }
        "session.sas" => {
            let sas = msg
                .payload
                .get("sas")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            println!("[session] SAS verification code: {sas}");
            println!("          Verify this code matches the other device.");
        }
        "session.ended" => {
            let reason = msg
                .payload
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            println!("[session] ended: {reason}");
        }
        "session.error" => {
            let reason = msg
                .payload
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            eprintln!("[session] ERROR: {reason}");
        }
        "pairing.request" => {
            if let Ok(req) = serde_json::from_value::<PairingRequestPayload>(msg.payload.clone()) {
                println!(
                    "[pairing] request from {} ({})",
                    req.remote_device_name, req.remote_device_type
                );
                println!("          SAS: {}", req.sas);
                println!("          capabilities: {:?}", req.capabilities_requested);

                // Interactive prompt
                eprint!("          Accept? [y/N]: ");
                let _ = std::io::stderr().flush();
                let mut input = String::new();
                let decision = if std::io::stdin().read_line(&mut input).is_ok()
                    && input.trim().eq_ignore_ascii_case("y")
                {
                    Decision::AllowOnce
                } else {
                    Decision::DenyOnce
                };

                let response = IpcMessage::new_decision(
                    "pairing.decision",
                    serde_json::to_value(PairingDecisionPayload {
                        request_id: req.request_id,
                        decision,
                        note: None,
                    })
                    .unwrap(),
                );
                if let Err(e) = write_message(writer, &response) {
                    eprintln!("[localbolt-cli] failed to send pairing decision: {e}");
                }
                let label = if matches!(decision, Decision::AllowOnce) {
                    "accepted"
                } else {
                    "denied"
                };
                println!("[pairing] {label}");
            }
        }
        "transfer.incoming.request" => {
            if let Ok(req) =
                serde_json::from_value::<TransferIncomingRequestPayload>(msg.payload.clone())
            {
                let size = format_bytes(req.file_size_bytes);
                println!(
                    "[transfer] incoming: {} ({}) from {}",
                    req.file_name, size, req.from_device_name
                );

                // Interactive prompt
                eprint!("           Accept? [y/N]: ");
                let _ = std::io::stderr().flush();
                let mut input = String::new();
                let decision = if std::io::stdin().read_line(&mut input).is_ok()
                    && input.trim().eq_ignore_ascii_case("y")
                {
                    Decision::AllowOnce
                } else {
                    Decision::DenyOnce
                };

                let response = IpcMessage::new_decision(
                    "transfer.incoming.decision",
                    serde_json::to_value(TransferIncomingDecisionPayload {
                        request_id: req.request_id,
                        decision,
                        note: None,
                    })
                    .unwrap(),
                );
                if let Err(e) = write_message(writer, &response) {
                    eprintln!("[localbolt-cli] failed to send transfer decision: {e}");
                }
                let label = if matches!(decision, Decision::AllowOnce) {
                    "accepted"
                } else {
                    "declined"
                };
                println!("[transfer] {label}");
            }
        }
        "transfer.started" => {
            let name = msg
                .payload
                .get("file_name")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let size = msg
                .payload
                .get("file_size_bytes")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let dir = msg
                .payload
                .get("direction")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            println!(
                "[transfer] started: {name} ({}) [{dir}]",
                format_bytes(size)
            );
        }
        "transfer.progress" => {
            let progress = msg
                .payload
                .get("progress")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let transferred = msg
                .payload
                .get("bytes_transferred")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let total = msg
                .payload
                .get("total_bytes")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let pct = (progress * 100.0) as u32;
            // Overwrite the same line for progress updates
            eprint!(
                "\r[transfer] {pct:>3}% ({} / {})",
                format_bytes(transferred),
                format_bytes(total)
            );
            let _ = std::io::stderr().flush();
            if pct >= 100 {
                eprintln!();
            }
        }
        "transfer.complete" => {
            let name = msg
                .payload
                .get("file_name")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let verified = msg
                .payload
                .get("verified")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let status = if verified { "verified" } else { "UNVERIFIED" };
            println!("[transfer] complete: {name} ({status})");
        }
        "transfer.paused" => {
            println!("[transfer] paused");
        }
        "transfer.resumed" => {
            println!("[transfer] resumed");
        }
        _ => {
            if msg.kind == IpcKind::Event {
                println!("[event] {}: {}", msg.msg_type, msg.payload);
            }
        }
    }
}

// ── Send ─────────────────────────────────────────────────────

fn cmd_send(cli: &Cli, file_path: &str) {
    let socket_path = resolve_socket_path(cli);
    let data_dir = resolve_data_dir(cli);

    let abs_path = match std::fs::canonicalize(file_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[localbolt-cli] cannot resolve path {file_path}: {e}");
            std::process::exit(1);
        }
    };

    if !abs_path.is_file() {
        eprintln!("[localbolt-cli] not a file: {}", abs_path.display());
        std::process::exit(1);
    }

    // Method 1: Try IPC file.send command
    if IpcStream::probe(&socket_path) {
        println!("[localbolt-cli] sending via IPC: {}", abs_path.display());
        let stream = match IpcStream::connect(&socket_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[localbolt-cli] IPC connect failed: {e}");
                std::process::exit(1);
            }
        };
        let mut writer = match stream.try_clone() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[localbolt-cli] clone stream: {e}");
                std::process::exit(1);
            }
        };

        // Handshake first
        let handshake = IpcMessage::new_decision(
            "version.handshake",
            serde_json::to_value(VersionHandshakePayload {
                app_version: DAEMON_VERSION.to_string(),
            })
            .unwrap(),
        );
        if let Err(e) = write_message(&mut writer, &handshake) {
            eprintln!("[localbolt-cli] handshake failed: {e}");
            std::process::exit(1);
        }

        // Send file.send command
        let send_cmd = IpcMessage::new_decision(
            "file.send",
            serde_json::json!({ "file_path": abs_path.to_string_lossy() }),
        );
        if let Err(e) = write_message(&mut writer, &send_cmd) {
            eprintln!("[localbolt-cli] IPC file.send failed: {e}");
            eprintln!("[localbolt-cli] falling back to signal file method");
        } else {
            println!("[localbolt-cli] file.send command sent");
            return;
        }
    }

    // Method 2: Signal file fallback
    let signal_path = PathBuf::from(&data_dir).join("send_file.signal");
    println!(
        "[localbolt-cli] writing signal file: {}",
        signal_path.display()
    );
    match std::fs::write(&signal_path, abs_path.to_string_lossy().as_bytes()) {
        Ok(()) => {
            println!("[localbolt-cli] send_file.signal written — daemon will pick up within 250ms");
        }
        Err(e) => {
            eprintln!("[localbolt-cli] failed to write signal file: {e}");
            std::process::exit(1);
        }
    }
}

// ── Stop ─────────────────────────────────────────────────────

fn cmd_stop(cli: &Cli) {
    let socket_path = resolve_socket_path(cli);

    if !IpcStream::probe(&socket_path) {
        println!("[localbolt-cli] daemon not running");
        return;
    }

    // Read PID file if available
    let pid_path = platform::default_pid_path();
    if let Ok(pid_str) = std::fs::read_to_string(&pid_path) {
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            if platform::process_alive(pid) {
                println!("[localbolt-cli] sending SIGTERM to daemon (PID {pid})...");
                platform::process_terminate(pid);

                // Wait for socket to disappear
                for _ in 0..20 {
                    std::thread::sleep(std::time::Duration::from_millis(250));
                    if !IpcStream::probe(&socket_path) {
                        println!("[localbolt-cli] daemon stopped");
                        let _ = std::fs::remove_file(&pid_path);
                        return;
                    }
                }

                eprintln!("[localbolt-cli] daemon did not stop gracefully, sending SIGKILL");
                platform::process_force_kill(pid);
                let _ = std::fs::remove_file(&pid_path);
                println!("[localbolt-cli] daemon killed");
                return;
            }
        }
    }

    eprintln!("[localbolt-cli] daemon socket exists but no PID file found");
    eprintln!(
        "[localbolt-cli] remove stale socket manually: {}",
        socket_path.display()
    );
    std::process::exit(1);
}

// ── Helpers ──────────────────────────────────────────────────

fn write_message(writer: &mut IpcStream, msg: &IpcMessage) -> std::io::Result<()> {
    let line = msg
        .to_ndjson()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    writer.write_all(line.as_bytes())?;
    writer.flush()
}

fn terminate_child(child: &mut Child) {
    #[cfg(unix)]
    {
        platform::process_terminate(child.id());
        // Give it 3 seconds to stop
        for _ in 0..12 {
            std::thread::sleep(std::time::Duration::from_millis(250));
            if let Ok(Some(_)) = child.try_wait() {
                return;
            }
        }
        platform::process_force_kill(child.id());
        let _ = child.wait();
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_units() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1_048_576), "1.0 MB");
        assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
    }

    #[test]
    fn resolve_socket_path_default() {
        let cli = Cli::parse_from(["localbolt-cli", "status"]);
        let path = resolve_socket_path(&cli);
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn resolve_socket_path_override() {
        let cli = Cli::parse_from([
            "localbolt-cli",
            "--socket-path",
            "/tmp/custom.sock",
            "status",
        ]);
        let path = resolve_socket_path(&cli);
        assert_eq!(path.to_string_lossy(), "/tmp/custom.sock");
    }

    #[test]
    fn resolve_data_dir_default() {
        let cli = Cli::parse_from(["localbolt-cli", "status"]);
        let dir = resolve_data_dir(&cli);
        assert!(!dir.is_empty());
    }

    #[test]
    fn resolve_data_dir_override() {
        let cli = Cli::parse_from(["localbolt-cli", "--data-dir", "/tmp/custom-data", "status"]);
        let dir = resolve_data_dir(&cli);
        assert_eq!(dir, "/tmp/custom-data");
    }
}
