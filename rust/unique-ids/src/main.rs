//! A maelstrom unique-id service using a 53-bit unsigned integer id inspired by twitter's
//! snowflake ids.

use std::ops::DerefMut;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use maelstrom::prelude::*;
use runtime::prelude::*;

lazy_static! {
    /// A timestamp corresponding to the earliest valid id generation. This must remain constant
    /// across the time in which these ids are in use. It currently is set to Wed Mar 29 2023
    /// 03:27:39 UTC.
    pub static ref ID_EPOCH: SystemTime = UNIX_EPOCH + Duration::from_millis(1_680_060_459_710);
}

#[tokio::main]
async fn main() -> Result<()> {
    runtime::setup()?;

    Node::builder()
        .handle("generate", generate)
        .with_state(Arc::new(RwLock::new(Source::default())))
        .run(tokio::io::stdin(), tokio::io::stdout())
        .await?;

    Ok(())
}

/// An opaque, unique id type using local system time to provide a pseudo ordering between ids, so
/// long as nodes' clocks only grow monotonically and never skew from one-another. Only useful for
/// loose ordering and not good for determining any sort of causal ordering or true timeline of
/// events in a distributed system but much cheaper than true vector clocks as every node can
/// calculate this id with no network communication.
///
/// ```text
/// 63           53                                                 14       8       0
///  ├─reserved─┤ ├────────────────────timestamp───────────────────┤ ├──id─┤ ├─count─┤
///  0000 0000 00 00 0000 0000 0000 0000 0000 0000 0000 0000 0000 00 00 0000 0000 0000
///  ├────10────┤ ├────────────────────────42──────────────────────┤ ├──6──┤ ├───8───┤
/// ```
///
/// This scheme allows a maximum of 256 unique ids per millisecond by allocating 8 bits to
/// a monotonically-increasing counter that resets every millisecond. With the 42 bits allocated to
/// the millisecond-precise timestamp, this id has a maximum lifespan of ~139.46 years from the
/// [`struct@ID_EPOCH`], meaning that it will be valid until about the end of September, 2162.
///
/// If we did not need the ordering between nodes, we could simply build this id using the six-bit
/// node id and a 50-bit counter, yielding 1,125,899,906,842,624 unique ids per-node before rolling
/// over. If we generated these ids at maximum rate for the timestamp-based id (256 ids per
/// millisecond) then this counter would last 4,398,046,511,104 milliseconds or ~139.46 years.
/// However, this would also allow a much faster query rate. One downside of a timestamp-less
/// scheme though is that nodes would need to store the last-vended counter in durable storage if
/// they were ever to survive restarts and resume vending ids from the same node id. We could
/// instead imagine generating new node ids from some central resource on startup but then we'd
/// probably need more than six bits for the node id if we wanted to support regular restarts,
/// meaning that we'd have less space for the unique counter and therefore a shorter lifespan for
/// this scheme.
#[derive(Debug, Eq, Hash, PartialEq, Serialize)]
pub struct Id(u64);

impl Id {
    /// Constructs a new Id from its component uints.
    ///
    /// Panics if:
    /// - `timestamp` is wider than 42 bits
    /// - `id` is wider than 6 bits
    fn new(timestamp: u64, id: u8, count: u8) -> Self {
        info!(timestamp, id, count, "id params");

        if timestamp >> 42 > 0 {
            panic!("timestamp overflows 42 bits: {timestamp}");
        }
        if id >> 6 > 0 {
            panic!("id overflows 6 bits: {id}");
        }

        let id: u64 = id.into();
        let count: u64 = count.into();
        Self((timestamp << 14) | (id << 8) | count)
    }
}

/// The main handler in this scenario responsible for generating each unique [`Id`].
pub async fn generate<C: Clock, I: IdSource>(
    state: Arc<RwLock<Source<C, I>>>,
    req: Generate,
) -> GenerateOk {
    let state = state.write().await;
    gen_sync(state, req)
}

// N.B. this is not thread-safe and is only used directly for a convenient sequential test
// interface.
fn gen_sync<C: Clock, I: IdSource>(
    mut state: impl DerefMut<Target = Source<C, I>>,
    req: Generate,
) -> GenerateOk {
    let now = state.clock.now();
    let since: Duration = now.duration_since(*ID_EPOCH).unwrap();
    let timestamp: u64 = since.as_millis().try_into().unwrap();
    info!(
        epoch = now.duration_since(UNIX_EPOCH).unwrap().as_millis(),
        timestamp, "timing"
    );

    if timestamp == state.last {
        state.counter += 1;
    } else {
        state.last = timestamp;
        state.counter = 0;
    }

    req.ok(Id::new(timestamp, state.node_id.get(), state.counter))
}

/// A request for a new [`Id`].
#[derive(Debug, Default, Deserialize)]
#[serde(tag = "type", rename = "generate")]
pub struct Generate {
    #[serde(flatten)]
    headers: Headers,
}

impl Generate {
    fn ok(self, id: Id) -> GenerateOk {
        GenerateOk {
            headers: self.headers.reply(),
            id,
        }
    }
}

/// A successful response.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename = "generate_ok")]
pub struct GenerateOk {
    #[serde(flatten)]
    headers: Headers,
    id: Id,
}

/// The various stateful resources needed to generate an [`Id`].
pub struct Source<C: Clock, I: IdSource> {
    /// A function returning the current time.
    clock: C,
    /// The last timestamp used in an id.
    last: u64,
    /// A function returning the local node's id number.
    node_id: I,
    /// A counter tracking the number of times
    counter: u8,
}

impl Default for Source<SystemClock, ProcessIdSource> {
    fn default() -> Self {
        Self::new(SystemClock, ProcessIdSource)
    }
}

impl<C: Clock, I: IdSource> Source<C, I> {
    fn new(clock: C, node_id: I) -> Self {
        Self {
            clock,
            node_id,
            last: 0,
            counter: 0,
        }
    }
}

pub trait Clock {
    fn now(&self) -> SystemTime;
}

pub trait IdSource {
    fn get(&self) -> u8;
}

struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// In maelstrom, every node is a process running on the same host. This assumes that process ids
/// are probably vended sequentially or that the chance of the lower six bits of the ids colliding
/// are low enough that we can treat those bits as unique.
///
/// It'd probably be better to build a way for our state to access the node's unique id that
/// maelstrom assigns in its `{"type": "init", "node_id": "n1", ... }` message but, because that
/// message arrives like any other after startup, we'd have to build some way for us to block on
/// that message's arrival before we could access the id. Maybe I'll come back for that later; it
/// seems like it'd be useful in the broadcast maelstrom scenario as well.
struct ProcessIdSource;

impl IdSource for ProcessIdSource {
    fn get(&self) -> u8 {
        (std::process::id() as u8) & 0b00111111
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use rstest::rstest;

    use super::*;

    #[rstest]
    async fn two_calls_with_default_sources_returns_unique_ids() {
        let state = Arc::new(RwLock::new(Source::default()));
        let GenerateOk { id: id1, .. } = generate(state.clone(), Generate::default()).await;
        let GenerateOk { id: id2, .. } = generate(state.clone(), Generate::default()).await;
        assert_ne!(id1, id2);
    }

    struct ConstantClock(u64);

    impl Clock for ConstantClock {
        fn now(&self) -> SystemTime {
            *ID_EPOCH + Duration::from_millis(self.0)
        }
    }

    #[rstest]
    fn max_calls_per_millisecond_are_unique() {
        let mut state = Source::new(ConstantClock(42), ProcessIdSource);
        let ids: HashSet<Id> = (0..256)
            .map(|_| gen_sync(&mut state, Generate::default()).id)
            .collect();
        assert_eq!(ids.len(), 256);
    }

    #[rstest]
    #[should_panic]
    /// It's important that this panics in this scenario. Otherwise, we'd risk vending the same id
    /// twice in a millisecond if the request rate exceeded our expectations.
    fn panic_if_257_calls_in_same_millisecond() {
        let mut state = Source::new(ConstantClock(42), ProcessIdSource);
        for _ in 0..257 {
            gen_sync(&mut state, Generate::default());
        }
    }
}
