# Gossip Glomers — Problems

Distributed-systems challenges from https://fly.io/dist-sys/. Each section names
the challenge, summarizes it, and gives the Maelstrom command(s) used for
validation.

Commands use `./maelstrom test ...` from inside a Maelstrom checkout. Replace
the `--bin` path with the binary you want to test — e.g. for the Rust workspace,
`./rust/target/release/<crate>` (`echo`, `unique-ids`, `broadcast`, etc.) after
`cargo build --release`.

---

## 1. Echo

A warm-up. Node reads JSON messages on stdin, replies with `echo_ok` carrying
the same payload. Exists to get familiar with Maelstrom plumbing.

```bash
./maelstrom test -w echo --bin ~/go/bin/maelstrom-echo \
  --node-count 1 --time-limit 10
```

---

## 2. Unique ID Generation

Globally unique IDs across a cluster, must hold under network partitions.
Workload: `unique-ids`.

```bash
./maelstrom test -w unique-ids --bin ~/go/bin/maelstrom-unique-ids \
  --time-limit 30 --rate 1000 --node-count 3 \
  --availability total --nemesis partition
```

---

## 3. Broadcast

Gossip a value to every node in the cluster; `read` returns all values seen.

### 3a — Single-Node Broadcast

```bash
./maelstrom test -w broadcast --bin ~/go/bin/maelstrom-broadcast \
  --node-count 1 --time-limit 20 --rate 10
```

### 3b — Multi-Node Broadcast

```bash
./maelstrom test -w broadcast --bin ~/go/bin/maelstrom-broadcast \
  --node-count 5 --time-limit 20 --rate 10
```

### 3c — Fault-Tolerant Broadcast

Same as 3b plus partitions. No values may be lost.

```bash
./maelstrom test -w broadcast --bin ~/go/bin/maelstrom-broadcast \
  --node-count 5 --time-limit 20 --rate 10 --nemesis partition
```

### 3d — Efficient Broadcast, Part I

Targets: msgs-per-op < 30, median latency < 400ms, max latency < 600ms.

```bash
./maelstrom test -w broadcast --bin ~/go/bin/maelstrom-broadcast \
  --node-count 25 --time-limit 20 --rate 100 --latency 100
```

Re-run with `--nemesis partition` to confirm correctness still holds.

### 3e — Efficient Broadcast, Part II

Tighter: msgs-per-op < 20, median latency < 1s, max latency < 2s.

```bash
./maelstrom test -w broadcast --bin ~/go/bin/maelstrom-broadcast \
  --node-count 25 --time-limit 20 --rate 100 --latency 100
```

---

## 4. Grow-Only Counter

Stateless g-counter against `g-counter` workload. Holds under partitions.

```bash
./maelstrom test -w g-counter --bin ~/go/bin/maelstrom-counter \
  --node-count 3 --rate 100 --time-limit 20 --nemesis partition
```

---

## 5. Kafka-Style Log

Replicated append-only log: `send`, `poll`, `commit_offsets`, `list_committed_offsets`.

### 5a — Single-Node

```bash
./maelstrom test -w kafka --bin ~/go/bin/maelstrom-kafka \
  --node-count 1 --concurrency 2n --time-limit 20 --rate 1000
```

### 5b — Multi-Node

```bash
./maelstrom test -w kafka --bin ~/go/bin/maelstrom-kafka \
  --node-count 2 --concurrency 2n --time-limit 20 --rate 1000
```

### 5c — Efficient

Same workload, focus on fewer CaS retries and fewer messages per op.

```bash
./maelstrom test -w kafka --bin ~/go/bin/maelstrom-kafka \
  --node-count 2 --concurrency 2n --time-limit 20 --rate 1000
```

---

## 6. Totally-Available Transactions

KV store with multi-op transactions on the `txn-rw-register` workload.

### 6a — Single-Node, Read Uncommitted

```bash
./maelstrom test -w txn-rw-register --bin ~/go/bin/maelstrom-txn \
  --node-count 1 --time-limit 20 --rate 1000 --concurrency 2n \
  --consistency-models read-uncommitted --availability total
```

### 6b — Totally-Available, Read Uncommitted

Baseline:

```bash
./maelstrom test -w txn-rw-register --bin ~/go/bin/maelstrom-txn \
  --node-count 2 --concurrency 2n --time-limit 20 --rate 1000 \
  --consistency-models read-uncommitted
```

Under partitions:

```bash
./maelstrom test -w txn-rw-register --bin ~/go/bin/maelstrom-txn \
  --node-count 2 --concurrency 2n --time-limit 20 --rate 1000 \
  --consistency-models read-uncommitted --availability total --nemesis partition
```

### 6c — Totally-Available, Read Committed

Stronger model: forbid G1a/G1b/G1c anomalies.

```bash
./maelstrom test -w txn-rw-register --bin ~/go/bin/maelstrom-txn \
  --node-count 2 --concurrency 2n --time-limit 20 --rate 1000 \
  --consistency-models read-committed --availability total --nemesis partition
```
