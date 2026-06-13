# Retries in unreliable networks — a layered survey

Three layers, each making different assumptions about what's below.

## TCP — the transport

- **Sequence numbers per byte.** Receiver ACKs cumulative bytes. Sender keeps an unacked window. ([RFC 9293, §3.4](https://datatracker.ietf.org/doc/html/rfc9293#name-sequence-numbers))
- **RTO from RTT estimation.** Karn–Jacobson: smoothed RTT (`SRTT`) + variance (`RTTVAR`). RTO = SRTT + 4·RTTVAR. Recomputed on every ACK. Doubles on retransmit (Karn's rule excludes retransmitted samples to avoid skew). ([RFC 6298](https://datatracker.ietf.org/doc/html/rfc6298); Jacobson, [*Congestion Avoidance and Control*, SIGCOMM 1988](https://ee.lbl.gov/papers/congavoid.pdf))
- **Fast retransmit.** 3 duplicate ACKs → resend immediately, don't wait for RTO. Cuts tail latency under single loss. ([RFC 5681, §3.2](https://datatracker.ietf.org/doc/html/rfc5681#section-3.2))
- **Selective ACK (SACK).** Receiver tells sender exactly which segments arrived → sender only resends gaps, not from the loss forward. ([RFC 2018](https://datatracker.ietf.org/doc/html/rfc2018))
- **Congestion control** (Reno, CUBIC, BBR) is the loop that prevents retries from amplifying loss. Window shrinks on loss, grows back on success. (Reno: [RFC 5681](https://datatracker.ietf.org/doc/html/rfc5681); CUBIC: [RFC 9438](https://datatracker.ietf.org/doc/html/rfc9438); BBR: [Cardwell et al., ACM Queue 2016](https://queue.acm.org/detail.cfm?id=3022184))
- **Connection-scoped.** Lives and dies with the socket; no application semantics.

Key property: TCP retries **bytes**, not messages, and gives up the connection eventually (`tcp_retries2`, ~15 min default on Linux — see [`tcp(7)`](https://man7.org/linux/man-pages/man7/tcp.7.html)). Above the socket, you have no idea what made it.

## Application RPC frameworks (gRPC, Thrift, Finagle, Tonic)

- **Per-call deadline**, propagated through the call graph. A retry budget is bounded by the remaining deadline, not by attempt count alone. ([gRPC: Deadlines](https://grpc.io/blog/deadlines/))
- **Idempotency keys.** gRPC sends the same `request-id`; server-side **dedup tables** (often a bounded LRU keyed by client+request-id) return cached responses to duplicates instead of re-executing. ([Stripe: Designing robust and predictable APIs with idempotency](https://stripe.com/blog/idempotency))
- **Retry policy is per-method**, declared, not implicit. gRPC's retry config: max attempts, initial backoff, multiplier, max backoff, retryable status codes (`UNAVAILABLE`, `RESOURCE_EXHAUSTED` yes; `INVALID_ARGUMENT` no). ([gRPC proposal A6: client retries](https://github.com/grpc/proposal/blob/master/A6-client-retries.md))
- **Exponential backoff + full jitter.** AWS Architecture Blog convinced the industry: `sleep = random(0, base · 2^attempt)`. Plain exponential synchronizes retries across clients ("thundering herd"); jitter desynchronizes. ([Brooker, *Exponential Backoff And Jitter*, AWS Architecture Blog, 2015](https://aws.amazon.com/blogs/architecture/exponential-backoff-and-jitter/))
- **Hedging** (Google "Tail at Scale", 2013). Send the request, and if no reply in p95 of normal RTT, send a second copy to a different replica. Take whichever wins, cancel the loser. Cuts tail latency at modest extra load. ([Dean & Barroso, *The Tail at Scale*, CACM 2013](https://research.google/pubs/the-tail-at-scale/); [gRPC proposal A6 — hedging](https://github.com/grpc/proposal/blob/master/A6-client-retries.md#hedging-policy))
- **Circuit breakers** (Hystrix, Finagle). After N failures to a host, **stop trying** for some window. Prevents the retries-on-a-dead-peer leak. ([Hystrix wiki](https://github.com/Netflix/Hystrix/wiki); Fowler, [*CircuitBreaker*](https://martinfowler.com/bliki/CircuitBreaker.html))
- **Token-bucket retry budgets.** Finagle's "retry budget": you only get to retry if you have credit, refilled from successful calls. Keeps a 10× retry storm from happening when a backend is degraded. ([Finagle: retry budgets](https://twitter.github.io/finagle/guide/Clients.html#retries))

Key property: app-layer retries can be **semantic** — they know what the call meant, can pick a different replica, can mark the operation idempotent or not.

## Distributed storage / consensus

Same primitives, more careful contracts.

- **Idempotent state machines.** Raft AppendEntries is idempotent — applying the same log index twice is a no-op. The retry mechanism is dumb on purpose; the protocol absorbs duplicates by design. The broadcast's `BTreeSet::insert` is the same pattern. ([Ongaro & Ousterhout, *In Search of an Understandable Consensus Algorithm*, USENIX ATC 2014](https://raft.github.io/raft.pdf), §5.3)
- **Log-and-replay (outbox / WAL).** Spanner, Kafka, FoundationDB don't retry from memory — they retry from a durable log. Crash → restart → replay. The in-memory retry loop is just an optimization; the log is the source of truth. ([Spanner, OSDI 2012](https://research.google/pubs/spanner-googles-globally-distributed-database-2/); [FoundationDB, SIGMOD 2021](https://www.foundationdb.org/files/fdb-paper.pdf); [Kafka design](https://kafka.apache.org/documentation/#design); transactional-outbox pattern: [microservices.io](https://microservices.io/patterns/data/transactional-outbox.html))
- **Gossip with anti-entropy** (Dynamo, Cassandra, Serf). Don't retry individual messages forever. Periodically run a **Merkle-tree diff** with a peer and ship only what's missing. Replaces O(messages × failures) retries with O(state diff). Bandwidth bounded. ([DeCandia et al., *Dynamo*, SOSP 2007](https://www.allthingsdistributed.com/files/amazon-dynamo-sosp2007.pdf), §4.7; [Cassandra: repair / Merkle trees](https://cassandra.apache.org/doc/latest/cassandra/managing/operating/repair.html); [Serf gossip protocol](https://www.serf.io/docs/internals/gossip.html); foundational: [Demers et al., *Epidemic Algorithms*, PODC 1987](https://dl.acm.org/doi/10.1145/41840.41841))
- **Hinted handoff** (Dynamo). If peer X is down, write the data for X into a "hint" on peer Y. When X comes back, Y delivers the hint and deletes it. Retries become someone else's pending queue, with a clear handoff. ([Dynamo paper, §4.6](https://www.allthingsdistributed.com/files/amazon-dynamo-sosp2007.pdf); [Cassandra hinted handoff](https://cassandra.apache.org/doc/latest/cassandra/managing/operating/hints.html))
- **Read repair / quorum.** Don't retry forever to make sure every replica has every write; rely on the next read to catch up the laggards. Trade write-time work for read-time work. ([Dynamo paper, §4.5](https://www.allthingsdistributed.com/files/amazon-dynamo-sosp2007.pdf); [Cassandra read repair](https://cassandra.apache.org/doc/latest/cassandra/managing/operating/read_repair.html))
- **Per-peer streams, not per-message tasks.** Raft uses a single AppendEntries stream per follower with a `nextIndex` cursor. One leader-side task per peer, not one per log entry. This is the "coalescing" pattern — Raft does it by construction. ([Raft paper, §5.3](https://raft.github.io/raft.pdf))
- **Heartbeats + suspicion** (SWIM, phi-accrual failure detector). Keep one cheap heartbeat going per peer. Adapt your retry strategy based on whether the detector says the peer is "up", "suspect", or "down". Don't waste retries on a peer the detector has demoted. ([Das, Gupta, Motivala, *SWIM*, DSN 2002](https://en.wikipedia.org/wiki/SWIM_Protocol) ([PDF](https://www.cs.cornell.edu/projects/Quicksilver/public_pdfs/SWIM.pdf)); [Hayashibara et al., *The φ Accrual Failure Detector*, SRDS 2004](https://www.computer.org/csdl/proceedings-article/srds/2004/22390066/12OmNvT2phv))

Key property: storage systems treat retries as a **steady-state cost** to budget, not an exceptional case. They cap retries by changing the abstraction — durable log + per-peer cursor + periodic anti-entropy — rather than by tuning a timer.

## Patterns that show up at every layer

| Pattern | TCP | RPC framework | Storage |
|---|---|---|---|
| Idempotency by ID | seq numbers | request-id dedup | log indices |
| Adaptive timeout | Karn–Jacobson RTO | SRTT-based deadline | phi-accrual detector |
| Backoff + jitter | RTO doubling | exponential + jitter | gossip jitter |
| Cap on retries | `tcp_retries2` | retry budget / circuit breaker | failure detector demotes |
| Coalescing | Nagle, SACK | streaming RPC | per-peer cursor |
| Anti-entropy | (none) | (rare) | Merkle / read repair |

## For the Maelstrom node specifically

The current design is closest to TCP **without** RTO adaptation, SACK, or congestion control — and without TCP's cap. Useful upgrades, ordered by how much they buy you per unit complexity:

1. **Cap retries by deadline or attempt count + circuit-break dead peers.** Kills the task-leak failure mode.
2. **Per-peer outbox / cursor instead of per-message task.** Raft-style: one resend loop per destination carrying "everything you haven't acked." Drops msgs/op and memory cost together.
3. **Exponential backoff with jitter.** Stops the self-amplification when replies are slow.
4. **Anti-entropy tick.** Cheap "do you have everything up to seq N?" exchange between peers, independent of the per-message path. Belt to the suspenders.

(1) and (3) are local changes inside `RpcWriter`. (2) is a bigger rework — it changes the unit of retry from `MsgId` to `(NodeId, payload-stream)`. (4) lives in the broadcast crate, not the maelstrom crate.

Which of these you want depends on whether you're optimizing for "passes 3c under partition" (current code does) or "looks like a real system" (the per-peer cursor is the inflection point).
