use serde::{Deserialize, Serialize};
use tracing::*;

use crate::message::Message;
use crate::node::rpc::Rpc;

/// A maelstrom [error] message provides an error code and a descriptive error message.
///
/// [error]: https://github.com/jepsen-io/maelstrom/blob/main/doc/protocol.md#errors
#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename = "error")]
pub struct Error {
    pub code: u32, // TODO: use an error code enum?
    pub text: String,
}

#[instrument(skip(rpc))]
pub async fn error(rpc: impl Rpc, msg: Message<Error>) {
    if let Some(source_id) = msg.headers.in_reply_to {
        rpc.notify_error(source_id, msg).await;
    } else {
        error!(?msg, "received an error message");
    }
}
