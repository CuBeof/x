use log::warn;
use crate::config::AppConfig;

pub fn run_paper(_cfg: &AppConfig) -> anyhow::Result<()> {
    warn!("Paper trading: implement WebSocket connector for live data feed");
    Ok(())
}
