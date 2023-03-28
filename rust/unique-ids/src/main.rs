use maelstrom::prelude::*;
use runtime::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    runtime::setup()?;

    Node::builder()
        .handle("generate", generate)
        .with_state(Default::default())
        .run(tokio::io::stdin(), tokio::io::stdout())
        .await?;

    Ok(())
}

async fn generate(_: State<()>, req: Generate) -> GenerateOk {
    // TODO: this id is not unique at all. try something like twitter's snowflake id algorithm
    // here.
    req.ok(Id(0))
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

#[derive(Debug, Serialize)]
struct Id(u64);
