use std::sync::OnceLock;

#[cfg(feature = "telemetry")]
use tracing_subscriber::{EnvFilter, fmt};

/// Initialize tracing subscriber once per process.
pub fn init_tracing() {
    #[cfg(feature = "telemetry")]
    static INIT: OnceLock<()> = OnceLock::new();

    #[cfg(feature = "telemetry")]
    {
        INIT.get_or_init(|| {
            let filter = EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("move_clippy=info"));
            let _ = fmt().with_env_filter(filter).try_init();
        });
    }
}

#[cfg(not(feature = "telemetry"))]
pub fn init_tracing() {}

/// Instrument an inline block with a span if telemetry is enabled.
#[macro_export]
macro_rules! instrument_block {
    ($name:expr, $block:block) => {{
        #[cfg(feature = "telemetry")]
        {
            let span = tracing::info_span!("move_clippy", phase = $name);
            let _guard = span.enter();
            (|| $block)()
        }
        #[cfg(not(feature = "telemetry"))]
        {
            (|| $block)()
        }
    }};
}
