use runtime::{setup, Result};
use tracing;

fn main() -> Result<()> {
    setup()?;
    tracing::info!(level = ?std::env::var("RUST_LOG"), "logging enabled");
    println!("{}", foo()?);
    println!("{}", bar()?);
    Ok(())
}

fn foo() -> Result<&'static str> {
    Ok("foo")
}

fn bar() -> Result<&'static str> {
    Err(Error("bar").into())
}

#[derive(Debug)]
struct Error(&'static str);

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", self.0)
    }
}
