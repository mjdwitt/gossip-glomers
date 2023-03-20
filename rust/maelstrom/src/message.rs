//! `message` provides types for building and parsing [maelstrom protocol] messages.
//!
//! [maelstrom protocol]: https://github.com/jepsen-io/maelstrom/blob/main/doc/protocol.md

use std::ops::Add;

use serde::{de, ser, Deserialize, Serialize};

/// All nodes (and clients) in maelstrom have a unique id.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
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
pub trait Body: std::fmt::Debug {
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

/// A convenience trait describing everything a type used as a request must implement.
pub trait Request: Body + de::DeserializeOwned + Send {}

impl<R> Request for R where R: Body + de::DeserializeOwned + Send {}

/// A convenience trait describing everything a type used as a response must implement.
pub trait Response: Body + ser::Serialize + Send {}

impl<R> Response for R where R: Body + ser::Serialize + Send {}

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
