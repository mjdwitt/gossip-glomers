use std::env;

use eyre::Result;

pub fn setup() -> Result<()> {
    if env::var("RUST_LIB_BACKTRACE").is_err() {
        env::set_var("RUST_LIB_BACKTRACE", "1");
    }

    color_eyre::install()
}
