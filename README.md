# port-cli

Interactive terminal UI for **listening TCP ports** on macOS (via `lsof`). Browse PIDs, processes, addresses, and users; filter and sort; send **SIGTERM** to a selected process.

## Requirements

- macOS (uses `lsof`)
- Rust 1.70+ (`cargo`)

## Run

```bash
cargo run --release
```

## Keys


| Key                       | Action                               |
| ------------------------- | ------------------------------------ |
| `q` / `Esc`               | Quit                                 |
| `j` / `↓`, `k` / `↑`      | Move selection                       |
| `Enter` / `K`             | Kill selected (confirm with `y`/`n`) |
| `r`                       | Refresh                              |
| `s`                       | Cycle sort: port → pid → name        |
| `/`                       | Filter (name, user, port, PID)       |
| `g` / `Home`, `G` / `End` | Jump top / bottom                    |
| `?`                       | Help overlay                         |


## Build

```bash
cargo build --release
# binary: target/release/port-cli
```

