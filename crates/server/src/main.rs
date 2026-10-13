use server::config::{Config, HELP};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

async fn shutdown_signal() {
    let ctrl_c = async {
        // If the handler cannot be installed, never resolve on this branch.
        if tokio::signal::ctrl_c().await.is_err() {
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cfg = match Config::from_env_and_args() {
        Ok(Some(c)) => c,
        Ok(None) => {
            println!("{HELP}");
            return std::process::ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("error: {e}\n\n{HELP}");
            return std::process::ExitCode::from(2);
        }
    };
    let listener = match TcpListener::bind(cfg.bind).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(bind = %cfg.bind, error = %e, "cannot bind");
            return std::process::ExitCode::FAILURE;
        }
    };
    let port = listener
        .local_addr()
        .map(|a| a.port())
        .unwrap_or(cfg.bind.port());
    if cfg.bind.ip().is_unspecified() {
        println!("Local:   http://127.0.0.1:{port}/");
        if let Some(ip) = server::lan_ipv4() {
            println!("LAN:     http://{ip}:{port}/   (share this with players)");
        }
    } else {
        println!("Serving: http://{}/", cfg.bind);
    }
    println!(
        "Limits:  {} rooms, {} players/room, {} connections",
        cfg.max_rooms, cfg.max_players_per_room, cfg.max_conns
    );

    match server::run(listener, cfg, shutdown_signal()).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %e, "server error");
            std::process::ExitCode::FAILURE
        }
    }
}
