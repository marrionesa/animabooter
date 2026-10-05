# AnimaBooter CLI

The command-line interface lives in `crates/animabooter-cli` and uses the
same Tauri-free engine as the desktop application.

## Local usage

```bash
cargo run -p animabooter-cli -- --help
cargo run -p animabooter-cli -- list
cargo run -p animabooter-cli -- list --json
cargo run -p animabooter-cli -- flash image.iso --device /dev/sdb --yes
cargo run -p animabooter-cli -- eject --device /dev/sdb
```

`flash` verifies the written data by default. Use `--no-verify` only when the
trade-off is understood. `--yes` is mandatory because flashing destroys all
data on the selected device. `--unsafe-mode` is required to include
non-removable devices in the safety checks.

Progress and diagnostics go to stderr so `--json` remains safe to pipe into
another program. A running flash can be cancelled with `Ctrl+C`; the command
returns a non-zero exit code when cancellation or any other error occurs.

## Architecture

```text
animabooter-cli ─┐
                 ├─ animabooter-core ─ platform backend
Tauri desktop ───┘                 └─ streaming flash pipeline
```

The CLI is not embedded into the desktop application. Both applications call
the shared Rust core directly, keeping safety and device behaviour identical.