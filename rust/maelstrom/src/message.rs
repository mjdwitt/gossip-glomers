//! `message` provides types for building and parsing [maelstrom protocol] messages.
//!
//! [maelstrom protocol]: https://github.com/jepsen-io/maelstrom/blob/main/doc/protocol.md

use serde::{Deserialize, Serialize};

/// All nodes (and clients) in maelstrom have a unique id.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct NodeId(pub String);

impl Into<String> for NodeId {
    fn into(self) -> String {
        self.0
    }
}

impl AsRef<str> for NodeId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Message bodies have whole-number ids.
#[derive(Copy, Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct MsgId(u64);

/// A [message] has a source, destination, and a body.
///
/// [message]: https://github.com/jepsen-io/maelstrom/blob/main/doc/protocol.md#messages
#[derive(Deserialize, Serialize)]
pub struct Message<B: Body> {
    pub src: NodeId,
    pub dest: NodeId,
    pub body: B,
}

impl<B: Body> Message<B> {
    pub fn new(src: NodeId, dest: NodeId, body: B) -> Self {
        Self { src, dest, body }
    }
}

/// The body trait provides access to the common [`Headers`] fields.
pub trait Body {
    fn headers(&self) -> &Headers;

    fn type_(&self) -> &str {
        &self.headers().type_
    }

    fn msg_id(&self) -> Option<MsgId> {
        self.headers().msg_id
    }

    fn in_reply_to(&self) -> Option<MsgId> {
        self.headers().in_reply_to
    }
}

/// All [message bodies] contain a type. They may optionally contain a message id and a field
/// identifying the message to which this one is responding.
///
/// [message bodies]: https://github.com/jepsen-io/maelstrom/blob/main/doc/protocol.md#message-bodies
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Headers {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg_id: Option<MsgId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<MsgId>,
}

impl Body for Headers {
    fn headers(&self) -> &Headers {
        self
    }
}

/// A maelstrom [error] message provides an error code and a descriptive error message.
///
/// [error]: https://github.com/jepsen-io/maelstrom/blob/main/doc/protocol.md#errors
#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct Error {
    #[serde(flatten)]
    pub headers: Headers,
    pub code: u32, // TODO: use an error code enum?
    pub text: String,
}

impl Body for Error {
    fn headers(&self) -> &Headers {
        &self.headers
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use rstest::rstest;
    use serde_json;

    use super::*;

    #[rstest]
    #[case::node_id(NodeId("n1".into()), "\"n1\"")]
    #[case::msg_id(MsgId(1), "1")]
    #[case::headers(
        Headers { type_: "msg".into(), msg_id: Some(MsgId(2)), in_reply_to: Some(MsgId(1)) },
        r#"{"type":"msg","msg_id":2,"in_reply_to":1}"#,
    )]
    #[case::optional_headers(
        Headers { type_: "msg".into(), ..Headers::default() },
        r#"{"type":"msg"}"#,
    )]
    #[case::error(
        Error {
            headers: Headers { type_: "error".into(), ..Headers::default() },
            code: 13,
            text: "node crashed".into(),
        },
        r#"{"type":"error","code":13,"text":"node crashed"}"#,
    )]
    fn json_formatting<'a, T>(#[case] t: T, #[case] s: &'static str)
    where
        T: Debug + PartialEq + Deserialize<'a> + Serialize,
    {
        assert_eq!(t, serde_json::from_str(s).unwrap());
        assert_eq!(s, &serde_json::to_string(&t).unwrap());
    }
}
