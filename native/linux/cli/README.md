# localbolt-cli — Linux CLI Helper

Phase 1 Linux/Steam Deck validation artifact for LocalBolt.

This is a terminal companion for `bolt-daemon` — it manages the daemon lifecycle,
streams IPC events, and provides interactive transfer control. It is **not** the
final Linux GUI shell (that is a future platform-native shell, TBD per
GOVERNANCE-NATIVE-SHELL-ALIGNMENT-1).

## Requirements

- Linux x86_64 (SteamOS, Ubuntu, Arch, Fedora, etc.)
- `bolt-daemon` binary (same machine or on PATH)

## Install (Steam Deck)

1. Switch to Desktop Mode.
2. Open Konsole.
3. Extract the tarball:

```bash
tar xzf localbolt-cli-0.1.0-x86_64-linux.tar.gz -C ~/.local/bin/
```

This places `localbolt-cli` and `bolt-daemon` in `~/.local/bin/`.

4. Verify:

```bash
localbolt-cli --version
```

## Usage

### Start daemon

```bash
# Start daemon and watch events interactively
localbolt-cli start --watch

# Start daemon in background
localbolt-cli start

# Custom WS listen address
localbolt-cli start --ws-listen 192.168.1.100:9557

# Auto-accept all pairing requests (less secure, useful for testing)
localbolt-cli start --pairing-policy allow --watch
```

### Check status

```bash
localbolt-cli status
```

### Watch events

```bash
# Attach to a running daemon's IPC stream
localbolt-cli watch
```

### Send a file

```bash
localbolt-cli send /path/to/file.txt
```

### Stop daemon

```bash
localbolt-cli stop
```

## Browser Transfer

1. Start the daemon: `localbolt-cli start --watch`
2. Open `localbolt.app` in a browser on another device (same network)
3. The browser and daemon discover each other via the rendezvous server
4. Accept the pairing request in the terminal when prompted
5. Verify the SAS code matches on both sides
6. Send or receive files

## File Locations

| Item | Path |
|------|------|
| Daemon data | `~/.local/share/localbolt/daemon/` |
| IPC socket | `/tmp/bolt-daemon.sock` |
| Crash logs | `~/.local/state/localbolt/` |
| Received files | `~/.local/share/localbolt/daemon/` (configurable via `--data-dir`) |

## Global Options

```
--daemon-path <path>    Path to bolt-daemon binary (default: auto-detect)
--socket-path <path>    IPC socket path (default: /tmp/bolt-daemon.sock)
--data-dir <path>       Data directory (default: ~/.local/share/localbolt/daemon)
```

## Build from Source

```bash
cd native/linux/cli
cargo build --release
```

Binary: `target/release/localbolt-cli`
