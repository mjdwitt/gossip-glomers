use tracing_subscriber::EnvFilter;

pub fn setup() {
    if std::env::var("RUST_LOG").is_err() {
        // FIXME: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("RUST_LOG", "debug") };
    }
    tracing_subscriber::fmt::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
}
