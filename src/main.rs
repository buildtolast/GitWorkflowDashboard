use std::sync::Arc;
use std::time::Duration;

use actix_cors::Cors;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::middleware::DefaultHeaders;
use actix_web::{web, App, HttpResponse, HttpServer};
use clap::Parser;
use tokio::sync::{broadcast, RwLock};
use tokio_util::sync::CancellationToken;
use tracing_actix_web::TracingLogger;

mod config;
mod error;
mod frontend;
mod github;
mod handlers;
mod middleware;
mod models;

use config::{AppConfig, Args};
use models::{DashboardState, SharedDashboardState, SharedRepoList, WsMessage};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // ─── 1. Initialize Structured Logging ───
    let json_logging = std::env::var("LOG_FORMAT")
        .ok()
        .map(|v| v == "json")
        .unwrap_or(false);

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    if json_logging {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(env_filter)
            .with_target(true)
            .with_thread_ids(true)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_target(true)
            .init();
    }

    // ─── 2. Parse CLI Args & Merge with Env Vars ───
    let args = Args::parse();
    let config = AppConfig::from_args_and_env(args);

    // ─── 3. Validate Config at Startup ───
    if let Err(e) = config.validate() {
        tracing::error!(error = %e, "Invalid configuration");
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, e));
    }

    // ─── 3b. Security warnings ───
    if config.dashboard_token.is_none() {
        tracing::warn!("No dashboard token configured — API endpoints are unauthenticated. Set DASHBOARD_TOKEN or --dashboard-token for production use.");
    }
    if config.webhook_secret.is_none() {
        tracing::warn!("No webhook secret configured — webhook endpoint accepts unverified payloads. Set WEBHOOK_SECRET or --webhook-secret if using webhooks.");
    }

    let host = config.server_host.clone();
    let port = config.server_port;
    let dashboard_origin = format!("http://{}:{}", host, port);

    tracing::info!(
        host = %host,
        port = port,
        repos = config.repos.len(),
        poll_interval = config.poll_interval_secs,
        auth_enabled = config.dashboard_token.is_some(),
        "Starting GitWorkflowDashboard"
    );

    // ─── 4. Create Shared State ───
    let github_client = Arc::new(github::client::GitHubClient::new(
        config.github_token.clone(),
        config.github_api_url.clone(),
    ));
    let dashboard_state: SharedDashboardState =
        Arc::new(RwLock::new(DashboardState::default()));
    let repo_list: SharedRepoList = Arc::new(RwLock::new(config.repos.clone()));
    let (ws_tx, _ws_rx) = broadcast::channel::<WsMessage>(4096);
    let webhook_secret = config.webhook_secret.clone();
    let dashboard_token: Option<String> = config.dashboard_token.clone();
    let cors_origins = config.cors_origins.clone();
    let max_repos = config.max_repos;
    let ws_connection_count = handlers::ws::new_connection_counter();

    // ─── 5. Create Graceful Shutdown Token ───
    let shutdown = CancellationToken::new();

    // ─── 6. Spawn Background Poller ───
    let run_tracker = github::poller::new_run_tracker();
    let poller_shutdown = shutdown.clone();
    let poller_client = github_client.clone();
    let poller_state = dashboard_state.clone();
    let poller_repos = repo_list.clone();
    let poller_ws_tx = ws_tx.clone();
    let poller_tracker = run_tracker.clone();
    let poll_interval = Duration::from_secs(config.poll_interval_secs);

    tokio::spawn(async move {
        github::poller::run_poller(
            poller_client,
            poller_state,
            poller_repos,
            poller_ws_tx,
            poll_interval,
            poller_shutdown,
            poller_tracker,
        )
        .await;
    });

    // ─── 7. Build and Run HTTP Server ───
    let shutdown_for_signal = shutdown.clone();

    // Rate limiter: ~60 req/min per IP across all API endpoints
    let rate_limit_config = GovernorConfigBuilder::default()
        .seconds_per_request(1) // replenish 1 token/sec
        .burst_size(30) // burst up to 30, sustain ~60/min
        .finish()
        .expect("Failed to build rate limiter config");

    let server = HttpServer::new(move || {
        // ─── CORS Configuration ───
        let mut cors = Cors::default()
            .allowed_methods(vec!["GET", "POST", "DELETE", "OPTIONS"])
            .allowed_headers(vec![
                actix_web::http::header::AUTHORIZATION,
                actix_web::http::header::CONTENT_TYPE,
                actix_web::http::header::ACCEPT,
                actix_web::http::header::HeaderName::from_static("x-requested-with"),
            ])
            .max_age(3600);

        // Allow dashboard's own origin
        cors = cors.allowed_origin(&dashboard_origin);

        // Add any extra configured origins
        for origin in &cors_origins {
            cors = cors.allowed_origin(origin);
        }

        // ─── Security Headers ───
        let security_headers = DefaultHeaders::new()
            .add(("X-Content-Type-Options", "nosniff"))
            .add(("X-Frame-Options", "DENY"))
            .add(("Referrer-Policy", "strict-origin-when-cross-origin"))
            .add(("Content-Security-Policy",
                "default-src 'self'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src 'self' https://avatars.githubusercontent.com; connect-src 'self' ws: wss:; frame-ancestors 'none'"
            ));

        App::new()
            .wrap(TracingLogger::default())
            .wrap(security_headers)
            .wrap(cors)
            .app_data(web::Data::new(github_client.clone()))
            .app_data(web::Data::new(dashboard_state.clone()))
            .app_data(web::Data::new(repo_list.clone()))
            .app_data(web::Data::new(ws_tx.clone()))
            .app_data(web::Data::new(webhook_secret.clone()))
            .app_data(web::Data::new(run_tracker.clone()))
            .app_data(web::Data::new(dashboard_token.clone()))
            .app_data(web::Data::new(max_repos))
            .app_data(web::Data::new(ws_connection_count.clone()))
            // Embedded frontend (no auth required)
            .route("/", web::get().to(index))
            // API endpoints (auth-guarded + rate-limited)
            .service(
                web::scope("/api")
                    .wrap(Governor::new(&rate_limit_config))
                    .wrap(middleware::auth::AuthMiddleware)
                    // Repo management
                    .route("/repos", web::get().to(handlers::repos::get_repos))
                    .route("/repos", web::post().to(handlers::repos::add_repos))
                    .route("/repos", web::delete().to(handlers::repos::remove_repos))
                    .route("/repos/search", web::get().to(handlers::repos::search_repos))
                    // Workflows
                    .route("/workflows", web::get().to(handlers::workflows::get_all_workflows))
                    .route("/workflows/{owner}/{repo}", web::get().to(handlers::workflows::get_workflows))
                    .route("/workflows/dispatch", web::post().to(handlers::workflows::dispatch_workflow))
                    .route("/workflows/{owner}/{repo}/{workflow_id}/inputs", web::get().to(handlers::workflows::get_workflow_inputs))
                    // Stale branches & cleanup — disabled, will revisit later
                    // .route("/branches/stale", web::get().to(handlers::branches::get_stale_branches))
                    // .route("/branches/{owner}/{repo}", web::get().to(handlers::workflows::get_branches))
                    // .route("/branches/{owner}/{repo}/{branch}", web::delete().to(handlers::branches::delete_branch))
                    // Runs (job logs route before {run_id} wildcard to avoid ambiguity)
                    .route("/runs/active", web::get().to(handlers::runs::get_active_runs))
                    .route("/runs/{owner}/{repo}/jobs/{job_id}/logs", web::get().to(handlers::runs::get_job_log))
                    .route("/runs/{owner}/{repo}", web::get().to(handlers::runs::get_repo_runs))
                    .route("/runs/{owner}/{repo}/{run_id}", web::get().to(handlers::runs::get_run_detail))
                    .route("/runs/{owner}/{repo}/{run_id}/cancel", web::post().to(handlers::runs::cancel_run))
                    .route("/runs/{owner}/{repo}/{run_id}/rerun", web::post().to(handlers::runs::rerun))
                    // Manual refresh
                    .route("/refresh", web::post().to(handlers::runs::manual_refresh))
                    // Webhook (has its own HMAC auth)
                    .route("/webhook", web::post().to(github::webhook::webhook_handler))
            )
            // WebSocket (auth checked inside handler)
            .route("/ws/live", web::get().to(handlers::ws::ws_handler))
    })
    .bind((host.as_str(), port))?
    .run();

    tracing::info!("Dashboard available at http://{}:{}", host, port);

    // ─── 8. Graceful Shutdown via tokio::select! ───
    tokio::select! {
        result = server => {
            tracing::info!("Server stopped");
            result
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received SIGINT, starting graceful shutdown...");
            shutdown_for_signal.cancel();
            tracing::info!("Shutdown complete");
            Ok(())
        }
    }
}

/// Serve embedded frontend HTML.
async fn index() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(frontend::INDEX_HTML)
}
