use crate::config::LiveConfig;

pub fn run_live(_cfg: &LiveConfig) -> anyhow::Result<()> {
    anyhow::bail!("Live trading: implement exchange connector and LiveNode setup")
}
