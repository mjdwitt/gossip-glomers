pub use eyre::{bail, ensure, Result};

pub mod errors;
pub mod prelude;
pub mod tracing;

pub fn setup() -> Result<()> {
    tracing::setup();
    errors::setup()?;
    Ok(())
}
