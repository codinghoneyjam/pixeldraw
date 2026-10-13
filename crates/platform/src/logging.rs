//! Logging bootstrap: `log` facade + `env_logger` backend.
//!
//! `RUST_LOG=debug` overrides; the default is `info` so every `log::info!` in
//! the native port is visible without configuration.

use std::sync::Once;

static INIT: Once = Once::new();

/// Idempotent: the second call is a no-op instead of a `try_init` error.
pub fn init_logging() {
    INIT.call_once(|| {
        let mut builder = env_logger::Builder::from_default_env();
        if std::env::var_os("RUST_LOG").is_none() {
            builder.filter_level(log::LevelFilter::Info);
        }
        let _ = builder.try_init();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_logging_is_idempotent() {
        init_logging();
        init_logging();
        log::info!("pixel-platform logging probe");
    }
}
