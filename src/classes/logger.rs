use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        #[cfg(debug_assertions)]
        return EnvFilter::new("debug");

        #[cfg(not(debug_assertions))]
        return EnvFilter::new("info");
    });

    #[cfg(debug_assertions)]
    {
        tracing_subscriber::registry()
            .with(
                fmt::layer()
                    .with_target(true)
                    .with_thread_ids(false)
                    .with_file(true)
                    .with_line_number(true)
                    .pretty(),
            )
            .with(filter)
            .init();
    }

    #[cfg(not(debug_assertions))]
    {
        tracing_subscriber::registry()
            .with(
                fmt::layer()
                    .with_target(false)
                    .with_thread_ids(false)
                    .with_file(false)
                    .with_line_number(false)
                    .compact(),
            )
            .with(filter)
            .init();
    }
}
