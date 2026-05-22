use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use crate::config::LoggingConfig;
use chrono::Local;

pub fn init_logging(cfg: &LoggingConfig) -> anyhow::Result<WorkerGuard> {
    std::fs::create_dir_all(&cfg.log_dir)?;

    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
    let log_filename = format!("{}_{}.log", cfg.log_level, timestamp);

    let file_appender = tracing_appender::rolling::never(&cfg.log_dir, &log_filename);
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_new(&cfg.log_level)
        .unwrap_or_else(|_| EnvFilter::new("info"));

    if cfg.log_to_file {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_writer(std::io::stdout))
            .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_writer(std::io::stdout))
            .init();
    }

    Ok(guard)
}
