use maelstrom::prelude::*;
use runtime::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    runtime::setup()?;

    Node::builder()
        .handle("echo", echo)
        .with_state(Default::default())
        .run(tokio::io::stdin(), tokio::io::stdout())
        .await?;

    Ok(())
}

async fn echo(_: State<()>, req: Echo) -> EchoOk {
    req.ok()
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename = "echo")]
struct Echo {
    #[serde(flatten)]
    headers: Headers,
    echo: String,
}

impl Body for Echo {
    fn headers(&self) -> &Headers {
        &self.headers
    }
}

impl Echo {
    fn ok(self) -> EchoOk {
        EchoOk {
            headers: Headers {
                in_reply_to: self.headers.msg_id,
                ..Default::default()
            },
            echo: self.echo,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename = "echo_ok")]
struct EchoOk {
    #[serde(flatten)]
    headers: Headers,
    echo: String,
}

impl Body for EchoOk {
    fn headers(&self) -> &Headers {
        &self.headers
    }
}
