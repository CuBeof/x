use crate::config::AppConfig;

pub fn run_live(_cfg: &AppConfig) -> anyhow::Result<()> {
    anyhow::bail!("Live trading: implement exchange connector and LiveNode setup")
}
