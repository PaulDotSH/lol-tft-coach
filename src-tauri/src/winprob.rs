//! The win-probability table in use: the one built in, or a newer one published by
//! `tools/winprob/nightly.py` and downloaded here.

use crate::state::Shared;
use coach::winprob::Model;
use std::sync::Arc;
use std::time::Duration;

/// `winprob.toml` on the repo's data branch; `LOL_COACH_WINPROB_URL` names another place.
const URL: &str = "https://raw.githubusercontent.com/PaulDotSH/lol-tft-coach/winprob-data/winprob.toml";
const URL_ENV: &str = "LOL_COACH_WINPROB_URL";
const EVERY: Duration = Duration::from_secs(6 * 3600);
/// A restart within this long reads the copy on disk.
const FRESH: Duration = Duration::from_secs(3600);

fn checked(toml: &str) -> anyhow::Result<Model> {
    let m = Model::parse(toml)?;
    match m.fault() {
        Some(fault) => anyhow::bail!("{fault}"),
        None => Ok(m),
    }
}

/// Looks for a published table now and every `EVERY`. One that is newer than the built-in
/// table replaces it; a failed download leaves things as they are.
pub async fn run(sh: Arc<Shared>) {
    let url = std::env::var(URL_ENV).ok().filter(|u| !u.is_empty()).unwrap_or_else(|| URL.into());
    loop {
        match sh.http.get_parsed(&url, FRESH, checked).await {
            Ok(m) => {
                let newer = m.newer_than(Model::shipped()).then(|| Arc::new(m));
                let mut cur = sh.winprob.lock().unwrap();
                if *cur != newer {
                    match &newer {
                        Some(m) => tracing::info!("win probability table: patch {}, {} games, from {url}", m.patch, m.games),
                        None => tracing::info!("win probability table: the built-in one is as new as {url}"),
                    }
                    *cur = newer;
                }
            }
            Err(e) => tracing::info!("win probability table not downloaded: {e:#}"),
        }
        tokio::time::sleep(EVERY).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_usable_table_is_kept() {
        let good = "patch = \"16.22\"\ngames = 500\nknots = [0, 20]\n[rift]\ngold = [5.0, 6.0]";
        assert_eq!(checked(good).unwrap().games, 500);
        assert!(checked(&good.replace("500", "0")).is_err());
        assert!(checked("<html>404: Not Found</html>").is_err());
        assert!(checked(include_str!("../../fixtures/coach/winprob_handset.toml")).is_ok());
    }
}
