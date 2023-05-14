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
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Deserialize, Serialize)]
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

impl From<u64> for MsgId {
    fn from(n: u64) -> Self {
        Self(n)
    }
}

impl Into<u64> for MsgId {
    fn into(self) -> u64 {
        self.0
    }
}

/// A [message] has a source, destination, and a body.
///
/// [message]: https://github.com/jepsen-io/maelstrom/blob/main/doc/protocol.md#messages
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Message<B: FmtDebug> {
    pub src: NodeId,
    pub dest: NodeId,
    #[serde(rename = "body")]
    pub headers: Headers<B>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Headers<B: FmtDebug> {
    pub msg_id: MsgId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<MsgId>,
    #[serde(flatten)]
    pub body: B,
}

impl<B: FmtDebug> Message<B> {
    pub fn body(&self) -> &B {
        &self.headers.body
    }
}

/// A convenience trait describing everything a type used as a request must implement.
pub trait Request: FmtDebug + de::DeserializeOwned + Send {}

impl<R> Request for R where R: FmtDebug + de::DeserializeOwned + Send {}

/// A convenience trait describing everything a type used as a response must implement.
pub trait Response: FmtDebug + ser::Serialize + Send {}

impl<R> Response for R where R: FmtDebug + ser::Serialize + Send {}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Type {
    #[serde(rename = "type")]
    pub type_: String,
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
    fn json_formatting<'a, T>(#[case] t: T, #[case] s: &'static str)
    where
        T: Debug + PartialEq + Deserialize<'a> + Serialize,
    {
        assert_eq!(t, serde_json::from_str(s).unwrap());
        assert_eq!(s, &serde_json::to_string(&t).unwrap());
    }
}
