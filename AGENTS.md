# AGENTS.md

Orientation guide for AI agents working in this repo.

## What this repo is

Solutions to the [Gossip Glomers](https://fly.io/dist-sys/) distributed systems challenge series from fly.io. Each challenge is a standalone binary that speaks the [Maelstrom](https://github.com/jepsen-io/maelstrom) protocol over stdin/stdout. See `problems.md` for a full description of each challenge.

The repo has parallel implementations in **Go** and **Rust**. Both are independent and self-contained.

---

## Repo layout

```
gossip-glomers/
├── problems.md          # Problem set descriptions (all 6 challenges)
├── AGENTS.md            # This file
├── go/
│   ├── go.mod           # Module: github.com/mjdwitt/gossip-glomers
│   ├── go.sum
│   ├── cmd/
│   │   ├── maelstrom-echo/         # Challenge 1
│   │   ├── maelstrom-unique-ids/   # Challenge 2
│   │   └── maelstrom-broadcast/    # Challenge 3
│   └── proc/
│       └── exit.go                 # os.Exit helper
└── rust/
    ├── Cargo.toml        # Workspace root
    ├── Cargo.lock
    ├── maelstrom/        # Protocol + node framework (internal library)
    ├── runtime/          # Tracing/error setup (internal library)
    ├── echo/             # Challenge 1
    ├── unique-ids/       # Challenge 2
    ├── broadcast/        # Challenge 3
    └── scratch/          # Throwaway experiments, not a challenge binary
```

---

## Current implementation state

| Challenge | Go | Rust |
|---|---|---|
| 1 — Echo | done | done |
| 2 — Unique IDs | done (UUID v4) | done (Snowflake-style 64-bit ID) |
| 3a — Single-node broadcast | done | done |
| 3b — Multi-node broadcast | done | done |
| 3c — Fault-tolerant broadcast | done (retry loop in `network/`) | done (retrying RPC in framework) |
| 3d — Efficient broadcast I | not started | not started |
| 3e — Efficient broadcast II | not started | not started |
| 4 — Grow-only counter | not started | not started |
| 5a — Single-node Kafka | not started | not started |
| 5b — Multi-node Kafka | not started | not started |
| 5c — Efficient Kafka | not started | not started |
| 6a — Single-node transactions | not started | not started |
| 6b — Read uncommitted | not started | not started |
| 6c — Read committed | not started | not started |

The broadcast implementation (3a–3c) is present in both languages but has **not yet been tuned** for the efficiency targets of 3d/3e.

---

## Go

```
go/
├── cmd/maelstrom-broadcast/
│   ├── main.go           # Handler registration; dispatches to state + network
│   ├── state/state.go    # Actor-pattern store (channel-based, single event loop)
│   └── network/network.go # Peer list; sendUntilAck retry loop (50 ms timeout)
```

- Uses `github.com/jepsen-io/maelstrom/demo/go` for the node/RPC abstraction.
- State is mutated only inside a single goroutine via an event channel — no mutexes.
- Reliable delivery: `sendUntilAck` retries with a 50 ms context timeout until the peer acks.
- Unique IDs use `github.com/google/uuid`.

**Build & test:**
```sh
cd go
go build ./...
go test ./...
```

---

## Rust

```
rust/
├── maelstrom/src/
│   ├── message.rs        # NodeId, MsgId, Message<B>, Request/Response traits
│   ├── node.rs           # NodeBuilder, handler registration, async run loop
│   ├── handler.rs        # Handler trait; impls for closures with 0–4 params
│   └── node/
│       ├── rpc.rs        # Rpc trait, RpcWriter, retrying send_rpc, pending map
│       ├── init.rs       # Auto-handles init; distributes Ids via oneshot
│       └── state.rs      # State<S> wrapper, FromRef<T> for flexible extraction
├── runtime/src/lib.rs    # setup() — tracing + eyre error handling
├── echo/src/main.rs
├── unique-ids/src/main.rs
└── broadcast/src/main.rs
```

- Handlers are registered with `.handle("type", fn)` where `fn` can be an async closure accepting any subset of `(Rpc, Ids, State<S>, Message<Req>)` — the framework dispatches by type.
- `send_rpc` spawns a background task that retries until an ack arrives.
- Unique IDs encode `(timestamp_ms << 14) | (node_id << 8) | counter` into a `u64`, using a custom epoch of 2023-03-29.
- Broadcast state is a `BTreeSet<u64>` behind an `Arc<RwLock<>>`.

**Build & test:**
```sh
cd rust
cargo build
cargo test
```

---

## Running with Maelstrom

Download the Maelstrom JAR from https://github.com/jepsen-io/maelstrom/releases, then run a workload against a built binary. Example for broadcast:

```sh
# Rust
./maelstrom test -w broadcast \
  --bin rust/target/debug/broadcast \
  --node-count 5 --time-limit 20 --rate 10 \
  --nemesis partition

# Go
./maelstrom test -w broadcast \
  --bin go/cmd/maelstrom-broadcast/maelstrom-broadcast \
  --node-count 5 --time-limit 20 --rate 10 \
  --nemesis partition
```

Maelstrom writes results (including a latency graph and violation log) to `store/`.

---

## Development branch

Active work happens on `claude/dist-sys-problems-md-shaxg1`.
