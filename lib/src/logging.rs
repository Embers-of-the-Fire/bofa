use crate::config::log::{Format, LogConfig};
use tracing_subscriber::EnvFilter;

pub fn init(config: &LogConfig, from_default_env: bool) {
    if !config.enabled {
        return;
    }

    let base = EnvFilter::try_new(&config.level).unwrap_or_else(|_| EnvFilter::new("warn"));

    let filter = if from_default_env {
        EnvFilter::try_from_default_env().unwrap_or(base)
    } else {
        base
    };

    match config.format {
        Format::Full => {
            tracing_subscriber::fmt().with_env_filter(filter).init();
        }
        Format::Compact => {
            tracing_subscriber::fmt()
                .compact()
                .with_env_filter(filter)
                .init();
        }
        Format::Pretty => {
            tracing_subscriber::fmt()
                .pretty()
                .with_env_filter(filter)
                .init();
        }
        Format::Json => {
            tracing_subscriber::fmt()
                .json()
                .with_env_filter(filter)
                .init();
        }
    }
}
