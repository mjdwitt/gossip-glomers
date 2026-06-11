# Gossip Glomers — Problem Set

Distributed systems challenges from [fly.io/dist-sys](https://fly.io/dist-sys/), built on top of [Maelstrom](https://github.com/jepsen-io/maelstrom) — a test harness by Kyle Kingsbury (Jepsen) that simulates a network of nodes, injects faults, and verifies correctness properties.

Each challenge is implemented as a binary that reads Maelstrom protocol messages from stdin and writes responses to stdout.

---

## Challenge 1 — Echo

**Goal:** Get familiar with the Maelstrom protocol and node lifecycle.

A client sends an `echo` message with a string payload; the node must reply with an `echo_ok` containing the same string. Completely stateless.

```json
// request
{ "type": "echo", "echo": "hello" }

// response
{ "type": "echo_ok", "echo": "hello" }
```

---

## Challenge 2 — Unique ID Generation

**Goal:** Build a globally unique ID generator that is totally available and coordination-free.

Nodes must respond to `generate` requests with an `id` that is guaranteed unique across the entire cluster for all time, even under partition. No two nodes may ever return the same ID. Ordering is a nice-to-have but not required.

```json
// request
{ "type": "generate" }

// response
{ "type": "generate_ok", "id": <unique-value> }
```

Common approaches: UUID v4 (random), or a Snowflake-style ID encoding `(timestamp | node_id | counter)` into a 64-bit integer.

---

## Challenge 3 — Broadcast

A multi-part challenge building toward an efficient, fault-tolerant gossip system.

### 3a — Single-Node Broadcast

Implement broadcast, read, and topology handlers on a single node. No replication needed; just store messages locally and return them on read.

```json
{ "type": "broadcast", "message": 1000 }
{ "type": "read" }          // → { "type": "read_ok", "messages": [1000, ...] }
{ "type": "topology", "topology": { "n1": ["n2", "n3"], ... } }
```

### 3b — Multi-Node Broadcast

Extend to a multi-node cluster. When a node receives a broadcast, it must propagate the value to all other nodes so every node eventually holds every message. Network is reliable (no partitions).

### 3c — Fault-Tolerant Broadcast

Same as 3b but the network now drops and delays messages. The broadcast protocol must retry until acknowledgment so that no messages are lost under partition.

### 3d — Efficient Broadcast, Part I

Optimize topology and fanout to reduce traffic. A flat tree (one root, all others as leaves) limits message hops to 2, keeping latency low while cutting redundant retransmissions.

- **Target:** messages-per-operation < 24, median latency < 400 ms, max latency < 600 ms

### 3e — Efficient Broadcast, Part II

Batch multiple messages together per gossip round instead of sending one message at a time. Nodes accumulate unseen messages and send them in bulk on a periodic timer.

- **Target:** messages-per-operation < 20, median latency < 1 s, max latency < 2 s

---

## Challenge 4 — Grow-Only Counter

**Goal:** Build a distributed, sequentially-consistent grow-only counter (G-counter) on top of Maelstrom's sequentially-consistent key/value store (`seq-kv`).

Each node owns a partition of the counter (keyed by node ID). `add` increments the local partition; `read` sums all partitions across nodes. Because the underlying store is sequentially consistent, reads may be slightly stale — that's acceptable as long as the counter never decreases.

```json
{ "type": "add", "delta": 5 }   // → { "type": "add_ok" }
{ "type": "read" }               // → { "type": "read_ok", "value": 42 }
```

---

## Challenge 5 — Kafka-Style Log

Build a replicated, append-only log similar to Kafka. Each key is an independent log; clients append records and poll by offset.

### 5a — Single-Node Kafka

In-memory log per key. `send` appends and returns the assigned offset; `poll` returns records at-or-after a given offset; `commit_offsets` / `list_committed_offsets` track consumer progress.

```json
{ "type": "send", "key": "k1", "msg": 123 }
// → { "type": "send_ok", "offset": 7 }

{ "type": "poll", "offsets": { "k1": 7 } }
// → { "type": "poll_ok", "msgs": { "k1": [[7, 123], [8, 456]] } }

{ "type": "commit_offsets", "offsets": { "k1": 8 } }
{ "type": "list_committed_offsets", "keys": ["k1"] }
// → { "type": "list_committed_offsets_ok", "offsets": { "k1": 8 } }
```

### 5b — Multi-Node Kafka

Persist logs in Maelstrom's linearizable key/value store (`lin-kv`) so data survives node failures. Requires careful use of compare-and-swap to avoid races when multiple nodes append to the same key.

- **Target:** ~12 messages-per-operation, availability ≥ 0.999

### 5c — Efficient Kafka-Style Log

Reduce CAS contention by sharding keys to specific owner nodes based on a hash. Each node handles appends for its own keys locally, forwarding foreign keys to the correct owner.

- **Target:** < 7 messages-per-operation

---

## Challenge 6 — Totally-Available Transactions

Build a transactional key/value store. Transactions are lists of operations: reads (`["r", key, null]`) and writes (`["w", key, value]`). The node executes the transaction and returns results.

```json
{ "type": "txn", "txn": [["r", "x", null], ["w", "y", 3]] }
// → { "type": "txn_ok", "txn": [["r", "x", 1],   ["w", "y", 3]] }
```

### 6a — Single-Node Transactions

In-memory map with no concurrency concerns. Establishes the baseline protocol.

### 6b — Read Uncommitted (Multi-Node)

Distribute transactions across nodes. The only required isolation guarantee is **no dirty writes** — two concurrent transactions must not interleave writes to the same key. Read uncommitted allows a transaction to observe another's uncommitted writes.

### 6c — Read Committed (Multi-Node)

Strengthen to **read committed**: transactions must not observe values written by transactions that have not yet committed (no dirty reads). Writes are still not required to be serializable.
