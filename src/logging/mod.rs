use log::LevelFilter;
use nautilus_common::{
    logging::{
        init_logging as nautilus_init_logging,
        logger::{LogGuard, LoggerConfig},
        writer::FileWriterConfig,
    },
};
use nautilus_core::UUID4;
use nautilus_model::identifiers::TraderId;

use crate::config::LoggingConfig;

fn parse_level(s: &str) -> LevelFilter {
    match s.to_uppercase().as_str() {
        "TRACE" => LevelFilter::Trace,
        "DEBUG" => LevelFilter::Debug,
        "INFO"  => LevelFilter::Info,
        "WARN" | "WARNING" => LevelFilter::Warn,
        "ERROR" => LevelFilter::Error,
        _       => LevelFilter::Info,
    }
}

/// Initialise nautilus logging. The returned `LogGuard` must be kept alive
/// for the duration of the program.
pub fn init_logging(cfg: &LoggingConfig) -> anyhow::Result<LogGuard> {
    let level = parse_level(&cfg.log_level);

    let fw_config = if cfg.log_to_file {
        std::fs::create_dir_all(&cfg.log_dir)?;
        FileWriterConfig::new(
            Some(cfg.log_dir.clone()),
            None,   // file_name: None → nautilus generates a timestamped name
            None,   // file_format: None → plain text
            None,   // file_rotate: None → no size-based rotation
        )
    } else {
        FileWriterConfig::new(None, None, None, None)
    };

    let logger_cfg = if cfg.log_to_file {
        LoggerConfig::builder()
            .stdout_level(level)
            .fileout_level(level)
            .is_colored(true)
            .file_config(fw_config.clone())
            .build()
    } else {
        LoggerConfig::builder()
            .stdout_level(level)
            .fileout_level(LevelFilter::Off)
            .is_colored(true)
            .build()
    };

    nautilus_init_logging(
        TraderId::new("GLFT-001"),
        UUID4::new(),
        logger_cfg,
        fw_config,
    )
    .map_err(|e| anyhow::anyhow!("Failed to initialize nautilus logger: {e}"))
}
