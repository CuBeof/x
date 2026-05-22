use log::warn;
use crate::config::PaperConfig;

pub fn run_paper(_cfg: &PaperConfig) -> anyhow::Result<()> {
    warn!("Paper trading: implement WebSocket connector for live data feed");
    Ok(())
}
