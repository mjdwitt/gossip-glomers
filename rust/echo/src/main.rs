use maelstrom::prelude::*;
use runtime::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    runtime::setup()?;

    Node::builder()
        .handle("echo", echo)
        .with_state(())
        .run(tokio::io::stdin(), tokio::io::stdout())
        .await?;

    Ok(())
}

async fn echo(req: Echo) -> EchoOk {
    req.ok()
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename = "echo")]
struct Echo {
    echo: String,
}

impl Echo {
    fn ok(self) -> EchoOk {
        EchoOk { echo: self.echo }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename = "echo_ok")]
struct EchoOk {
    echo: String,
}
