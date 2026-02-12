# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Build
cargo build --all-features

# Test all features
cargo test --all-features

# Test only sync
cargo test --features sync

# Test only async
cargo test --features async

# Run a single test
cargo test --all-features test_name

# Run tests in a specific file
cargo test --all-features --test sync
cargo test --all-features --test async

# Lint
cargo clippy --all-features

# Format
cargo fmt

# Run examples
cargo run --example simple --features sync
cargo run --example async_simple --features async
```

## Architecture

This is `cron_tab`, a Rust cron job scheduler library published on crates.io. It provides both synchronous (thread-based) and asynchronous (tokio-based) schedulers, gated behind feature flags.

### Feature Flags

- `sync` (default) — enables `Cron<Z>` and `Entry<Z>`
- `async` — enables `AsyncCron<Z>` and `AsyncEntry<Z>`, pulls in tokio/futures
- `all` — both

### Module Layout

The sync and async implementations are parallel in structure:

| Sync (`feature = "sync"`) | Async (`feature = "async"`) | Role |
|---|---|---|
| `cron.rs` → `Cron<Z>` | `async_cron.rs` → `AsyncCron<Z>` | Scheduler: manages entries, runs the select loop |
| `entry.rs` → `Entry<Z>` | `async_entry.rs` → `AsyncEntry<Z>` | Job entry: holds schedule, next fire time, callback |

`error.rs` and `lib.rs` are always compiled regardless of features.

### Key Design Patterns

- **Timezone-generic**: Both schedulers are generic over `Z: TimeZone`, accepting `Utc`, `FixedOffset`, `Local`, etc.
- **Two job types**: Recurring jobs have `schedule: Some(cron::Schedule)`, one-time jobs have `schedule: None`. The `is_once()` method on entries derives this from the schedule field.
- **Scheduler loop**: Uses `crossbeam_channel::select!` (sync) or `tokio::select!` (async) with three/four branches: timer expiry, add channel, remove channel (async only), stop channel.
- **Job dispatch**: Sync spawns `std::thread`, async spawns `tokio::task`. Jobs are `Arc`-wrapped for safe sharing.
- **Channel-based communication**: Both `add_fn`/`add_fn_once` and `remove` send through channels when the scheduler is running. The async version also supports direct push to entries when not running.

### Cron Expression Format

7-field format with second precision: `sec min hour day month weekday year`. The `cron` crate (v0.15) handles parsing. Both 6-field and 7-field formats are accepted.
