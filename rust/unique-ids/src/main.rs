use std::time::{Duration, SystemTime, UNIX_EPOCH};

use maelstrom::prelude::*;
use runtime::prelude::*;

lazy_static! {
    static ref ID_EPOCH: SystemTime = UNIX_EPOCH + Duration::from_millis(1_680_060_459_710);
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

struct Source<C: Clock, I: IdSource> {
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

trait Clock {
    fn now(&self) -> SystemTime;
}

struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

trait IdSource {
    fn get(&self) -> u8;
}

struct ProcessIdSource;

impl IdSource for ProcessIdSource {
    fn get(&self) -> u8 {
        (std::process::id() as u8) & 0b00111111
    }
}

/// ```text
/// 63           53                                                 14       8       0
///  ├─reserved─┤ ├────────────────────timestamp───────────────────┤ ├─id──┤ ├─count─┤
///  0000 0000 00 00 0000 0000 0000 0000 0000 0000 0000 0000 0000 00 00 0000 0000 0000
/// ```
#[derive(Debug, Serialize)]
struct Id(u64);

impl Id {
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

impl From<u64> for Id {
    fn from(n: u64) -> Self {
        Self(n)
    }
}

async fn generate<C: Clock, I: IdSource>(state: State<Source<C, I>>, req: Generate) -> GenerateOk {
    let mut state = state.write().await;

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

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename = "generate")]
struct Generate {
    #[serde(flatten)]
    headers: Headers,
}

impl Body for Generate {
    fn headers(&self) -> &Headers {
        &self.headers
    }
}

impl Generate {
    fn ok(self, id: Id) -> GenerateOk {
        GenerateOk {
            headers: Headers {
                in_reply_to: self.headers.msg_id,
                ..Default::default()
            },
            id,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename = "generate_ok")]

struct GenerateOk {
    #[serde(flatten)]
    headers: Headers,
    id: Id,
}

impl Body for GenerateOk {
    fn headers(&self) -> &Headers {
        &self.headers
    }
}
