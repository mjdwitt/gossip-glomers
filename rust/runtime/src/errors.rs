use std::env;

use eyre::Result;

/// Installs color_eyre and enables backtrace reporting on panics and errors.
pub fn setup_reporting() -> Result<()> {
    if env::var("RUST_LIB_BACKTRACE").is_err() {
        // FIXME: Audit that the environment access only happens in single-threaded code.
        unsafe { env::set_var("RUST_LIB_BACKTRACE", "1") };
    }

    stable_eyre::install()
}
