//! The build tables made by the minipc service from its crawled games (`/v1/builds.json`).

use crate::state::Shared;
use coach::builds::Table;
use std::sync::Arc;
use std::time::Duration;

/// Names another place for the table than the service.
const URL_ENV: &str = "LOL_COACH_BUILDS_URL";
const EVERY: Duration = Duration::from_secs(6 * 3600);
/// A restart within this long reads the copy on disk.
const FRESH: Duration = Duration::from_secs(3600);

/// `LOL_COACH_BUILDS_URL`, else the service when one is set; None: no table.
fn table_url(env: Option<String>, server_url: &str) -> Option<String> {
    let server = server_url.trim().trim_end_matches('/');
    match env.filter(|u| !u.is_empty()) {
        Some(url) => Some(url),
        None if !server.is_empty() => Some(format!("{server}/v1/builds.json")),
        None => None,
    }
}

fn checked(json: &str) -> anyhow::Result<Table> {
    let t = Table::parse(json).map_err(anyhow::Error::msg)?;
    anyhow::ensure!(t.games > 0, "build table with no games");
    Ok(t)
}

/// Reads the table now and every `EVERY`; a failed download keeps the last one.
pub async fn run(sh: Arc<Shared>) {
    loop {
        match table_url(std::env::var(URL_ENV).ok(), &sh.settings().server_url) {
            None => *sh.builds.lock().unwrap() = None,
            Some(url) => match sh.http.get_parsed(&url, FRESH, checked).await {
                Ok(t) => {
                    let mut cur = sh.builds.lock().unwrap();
                    if cur.as_deref() != Some(&t) {
                        tracing::info!("build tables: patch {}, {} games, {} champion and role pairs, from {url}", t.patch, t.games, t.pairs.len());
                        *cur = Some(Arc::new(t));
                    }
                }
                Err(e) => tracing::info!("build tables not downloaded: {e:#}"),
            },
        }
        tokio::time::sleep(EVERY).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_comes_from_the_env_then_the_service_and_else_there_is_none() {
        assert_eq!(table_url(None, ""), None);
        assert_eq!(table_url(Some(String::new()), "  "), None);
        assert_eq!(table_url(None, "https://lol.example/").as_deref(), Some("https://lol.example/v1/builds.json"));
        assert_eq!(table_url(Some("http://x/b.json".into()), "https://lol.example").as_deref(), Some("http://x/b.json"));
    }

    #[test]
    fn only_a_usable_table_is_kept() {
        assert_eq!(checked(include_str!("../../fixtures/coach/builds_cut.json")).unwrap().patch, "16.22");
        assert!(checked("<html>404: Not Found</html>").is_err());
    }
}
