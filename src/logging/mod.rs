use log::LevelFilter;
use log4rs::{
    append::{console::ConsoleAppender, file::FileAppender},
    config::{Appender, Config, Logger, Root},
    encode::pattern::PatternEncoder,
};
use chrono::Local;
use crate::config::LoggingConfig;

// Timestamp pattern matches nautilus_trader's log format:
// 2024-01-15T10:30:00.123 INFO market_maker::runner::backtest - Message
const FILE_PATTERN: &str = "{d(%Y-%m-%dT%H:%M:%S%.3f)} {l:<5} {t} - {m}{n}";
const STDOUT_PATTERN: &str = "{d(%H:%M:%S%.3f)} {h({l:<5})} {t} - {m}{n}";

/// Initialise `log4rs` logging. The returned `Handle` must be kept alive for
/// the duration of the program (dropping it resets the logger).
pub fn init_logging(cfg: &LoggingConfig) -> anyhow::Result<log4rs::Handle> {
    let level: LevelFilter = cfg
        .log_level
        .parse()
        .unwrap_or(LevelFilter::Info);

    // ── stdout appender ──────────────────────────────────────────────────────
    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new(STDOUT_PATTERN)))
        .build();

    let mut config_builder = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)));

    let mut root_builder = Root::builder().appender("stdout");

    // ── file appender (optional) ──────────────────────────────────────────────
    if cfg.log_to_file {
        std::fs::create_dir_all(&cfg.log_dir)?;

        // File name format: {level}_{YYYY-MM-DD_HH-MM-SS}.log
        // Mirrors nautilus_trader: {trader_id}_{%Y-%m-%d_%H%M%S}_{instance_id}.log
        let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
        let filename = format!("{}_{}.log", cfg.log_level.to_lowercase(), timestamp);
        let log_path = format!("{}/{}", cfg.log_dir, filename);

        let file_appender = FileAppender::builder()
            .encoder(Box::new(PatternEncoder::new(FILE_PATTERN)))
            .build(&log_path)?;

        config_builder = config_builder
            .appender(Appender::builder().build("file", Box::new(file_appender)));
        root_builder = root_builder.appender("file");

        // Suppress overly verbose dependency logs to file
        config_builder = config_builder
            .logger(Logger::builder().build("tokio", LevelFilter::Warn))
            .logger(Logger::builder().build("mio", LevelFilter::Warn));
    }

    let config = config_builder.build(root_builder.build(level))?;
    Ok(log4rs::init_config(config)?)
}
