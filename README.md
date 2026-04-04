# portui

Interactive terminal UI for **listening TCP ports** on macOS (via `lsof`). Browse PIDs, processes, addresses, and users; filter and sort; send **SIGTERM** to a selected process.

## Requirements

- macOS (uses `lsof`)

## Install

**From npm** (Node.js 18+):

```bash
npm install -g portui
```

Or run without a global install:

```bash
npx portui
```

The first time you install a given version, the package downloads a prebuilt binary from [GitHub Releases](https://github.com/alexchen1337/port-cli/releases). Publish a matching `vX.Y.Z` tag and let the release workflow attach `portui-darwin-arm64.tar.gz` and `portui-darwin-x64.tar.gz` before publishing to npm.

**From source** (Rust 1.70+):

```bash
cargo install --path .
# or
cargo run --release
```

**Local npm development** (use the binary you built with Cargo instead of downloading):

```bash
cargo build --release
npm install
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
# binary: target/release/portui
```

