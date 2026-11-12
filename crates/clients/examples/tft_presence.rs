//! What the standalone TFT client reports through the Riot Client, read once.
//! `cargo run -p clients --example tft_presence`

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let rc = clients::riot_client::discover().ok_or_else(|| anyhow::anyhow!("Riot Client not running"))?;
    println!("{:?}", rc.tft().await?);
    Ok(())
}
