//! `message` provides types for building and parsing [maelstrom protocol] messages.
//!
//! [maelstrom protocol]: https://github.com/jepsen-io/maelstrom/blob/main/doc/protocol.md

use std::fmt::Debug as FmtDebug;
use std::ops::Add;

use serde::{de, ser, Deserialize, Serialize};

/// All nodes (and clients) in maelstrom have a unique id.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub struct NodeId(pub String);

impl From<String> for NodeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for NodeId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for NodeId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<&str> for NodeId {
    fn eq(&self, rhs: &&str) -> bool {
        self.0 == *rhs
    }
}

/// Message bodies have whole-number ids.
#[derive(Copy, Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct MsgId(u64);

impl Add for MsgId {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        MsgId(self.0 + rhs.0)
    }
}

impl Add<u64> for MsgId {
    type Output = Self;
    fn add(self, rhs: u64) -> Self {
        MsgId(self.0 + rhs)
    }
}

/// A [message] has a source, destination, and a body.
///
/// [message]: https://github.com/jepsen-io/maelstrom/blob/main/doc/protocol.md#messages
#[derive(Debug, Deserialize, Serialize)]
pub struct Message<B: FmtDebug> {
    pub src: NodeId,
    pub dest: NodeId,
    pub body: B,
}

impl<B: FmtDebug> Message<B> {
    pub fn new(src: NodeId, dest: NodeId, body: B) -> Self {
        Self { src, dest, body }
    }
}

/// A convenience trait describing everything a type used as a request must implement.
pub trait Request: FmtDebug + de::DeserializeOwned + Send {}

impl<R> Request for R where R: FmtDebug + de::DeserializeOwned + Send {}

/// A convenience trait describing everything a type used as a response must implement.
pub trait Response: FmtDebug + ser::Serialize + Send {}

impl<R> Response for R where R: FmtDebug + ser::Serialize + Send {}

/// All [message bodies] may optionally contain a message id and a field identifying the message to
/// which this one is responding.
///
/// N.B. All messages contain a type, but we're using `#[serde(tag = "type")]` to generate that for
/// us from the message type names.
///
/// [message bodies]: https://github.com/jepsen-io/maelstrom/blob/main/doc/protocol.md#message-bodies
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Headers {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg_id: Option<MsgId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<MsgId>,
}

impl Headers {
    pub fn reply(self) -> Headers {
        Headers {
            in_reply_to: self.msg_id,
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Type {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(flatten)]
    pub headers: Headers,
}

/// A maelstrom [error] message provides an error code and a descriptive error message.
///
/// [error]: https://github.com/jepsen-io/maelstrom/blob/main/doc/protocol.md#errors
#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename = "error")]
pub struct Error {
    #[serde(flatten)]
    pub headers: Headers,
    pub code: u32, // TODO: use an error code enum?
    pub text: String,
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
        Headers { msg_id: Some(MsgId(2)), in_reply_to: Some(MsgId(1)) },
        r#"{"msg_id":2,"in_reply_to":1}"#,
    )]
    #[case::optional_headers(Headers::default(), "{}")]
    #[case::error(
        Error {
            headers: Headers::default(),
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
