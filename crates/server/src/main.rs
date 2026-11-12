use lol_coach_server::config::Config;
use lol_coach_server::riot::Riot;
use lol_coach_server::{AppState, app};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

fn config_path() -> Result<PathBuf, String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--config" {
            return args.next().map(PathBuf::from).ok_or_else(|| "--config needs a path".to_owned());
        }
    }
    std::env::var_os("LOL_COACH_SERVER_CONFIG").map(PathBuf::from).ok_or_else(|| "no config: pass --config <path> or set LOL_COACH_SERVER_CONFIG".to_owned())
}

fn riot_client(config: &Config) -> Option<Riot> {
    let path = config.riot_key_file.as_ref()?;
    let key = match std::fs::read_to_string(path) {
        Ok(key) if !key.trim().is_empty() => key,
        Ok(_) => {
            tracing::warn!(path = %path.display(), "Riot key file is empty, lookups are off");
            return None;
        }
        Err(err) => {
            tracing::warn!(path = %path.display(), %err, "cannot read the Riot key file, lookups are off");
            return None;
        }
    };
    Riot::new(&key).inspect_err(|err| tracing::warn!(%err, "lookups are off")).ok()
}

async fn shutdown() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut signal) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {}
        () = term => {}
    }
    tracing::info!("shutting down");
}

async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::load(&config_path()?)?;
    std::fs::create_dir_all(config.data_dir.join("players"))?;
    let riot = riot_client(&config);
    tracing::info!(listen = %config.listen, data_dir = %config.data_dir.display(), lookups = riot.is_some(), crawl = config.crawl.is_some(), "starting");
    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    let state = AppState { config: Arc::new(config), riot };
    axum::serve(listener, app(state)).with_graceful_shutdown(shutdown()).await?;
    Ok(())
}

#[tokio::main(worker_threads = 2)]
async fn main() -> ExitCode {
    tracing_subscriber::fmt().with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "lol_coach_server=info,tower_http=info".into())).init();
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("lol-coach-server: {err}");
            ExitCode::FAILURE
        }
    }
}
