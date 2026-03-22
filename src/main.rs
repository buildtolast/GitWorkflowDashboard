use std::sync::Arc;
use actix_cors::Cors;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::middleware::DefaultHeaders;
use actix_web::{web, App, HttpResponse, HttpServer};
use clap::Parser;
use tokio::sync::{broadcast, RwLock};
use tokio_util::sync::CancellationToken;
use tracing_actix_web::TracingLogger;

mod config;
mod database;
mod error;
mod frontend;
mod github;
mod handlers;
mod middleware;
mod models;
mod repo_store;

use config::{AppConfig, Args};
use database::SharedDatabase;
use models::{DashboardState, SharedDashboardState, SharedRepoList};

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

    // ─── 3c. Resolve data directory, open database, and load persisted repos ───
    let data_dir = repo_store::resolve_data_dir(&config.data_dir);
    let mut startup_repos = config.repos.clone();

    let db: Option<database::Database> = if let Some(ref dir) = data_dir {
        match database::Database::open(dir) {
            Ok(db) => {
                // Load repos from database (includes auto-migrated JSON repos)
                let persisted = db.load_repos();
                let before = startup_repos.len();
                for repo in persisted {
                    if !startup_repos.iter().any(|r| r.full_name() == repo.full_name()) {
                        startup_repos.push(repo);
                    }
                }
                let added = startup_repos.len() - before;
                if added > 0 {
                    tracing::info!(count = added, "Merged persisted repos with config repos");
                }
                // Ensure all startup repos are saved to database
                db.save_repos(&startup_repos);
                Some(db)
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to open database — falling back to JSON");
                // Fallback to legacy JSON loading
                let persisted = repo_store::load_repos(dir);
                let before = startup_repos.len();
                for repo in persisted {
                    if !startup_repos.iter().any(|r| r.full_name() == repo.full_name()) {
                        startup_repos.push(repo);
                    }
                }
                let added = startup_repos.len() - before;
                if added > 0 {
                    tracing::info!(count = added, "Merged persisted repos from JSON fallback");
                }
                None
            }
        }
    } else {
        None
    };
    let shared_db: SharedDatabase = Arc::new(db);

    tracing::info!(
        host = %host,
        port = port,
        repos = startup_repos.len(),
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
    let repo_list: SharedRepoList = Arc::new(RwLock::new(startup_repos));
    let (ws_tx, _ws_rx) = broadcast::channel::<Arc<String>>(4096);
    let webhook_secret = config.webhook_secret.clone();
    let dashboard_token: Option<String> = config.dashboard_token.clone();
    let cors_origins = config.cors_origins.clone();
    let max_repos = config.max_repos;
    let ws_connection_count = handlers::ws::new_connection_counter();
    let auth_rate_limit = web::Data::new(middleware::auth::new_rate_limit_state());

    // ─── 5. Create Graceful Shutdown Token ───
    let shutdown = CancellationToken::new();

    // ─── 6. Spawn Background Poller ───
    let run_tracker = github::poller::new_run_tracker();
    let poller_shutdown = shutdown.clone();
    let poller_deps = github::poller::PollerDeps {
        client: github_client.clone(),
        state: dashboard_state.clone(),
        repos: repo_list.clone(),
        ws_tx: ws_tx.clone(),
        db: shared_db.clone(),
        previous_runs: run_tracker.clone(),
        cancel: poller_shutdown,
    };
    let poller_config = config.clone();

    tokio::spawn(async move {
        github::poller::run_poller(&poller_config, poller_deps).await;
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
                "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; connect-src 'self' ws: wss:; frame-ancestors 'none'; base-uri 'self'"
            ))
            .add(("Permissions-Policy", "camera=(), microphone=(), geolocation=(), interest-cohort=()"))
            .add(("Cross-Origin-Opener-Policy", "same-origin"));

        App::new()
            .wrap(actix_web::middleware::from_fn(request_id_middleware))
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
            .app_data(web::Data::new(shared_db.clone()))
            .app_data(auth_rate_limit.clone())
            // Health probes (public, no auth)
            .route("/healthz", web::get().to(healthz))
            .route("/readyz", web::get().to(readyz))
            // Embedded frontend (no auth required)
            .route("/", web::get().to(index))
            // Build version (public, no auth)
            .route("/api/version", web::get().to(version_info))
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
                    // Branches (for dispatch dialog)
                    .route("/branches/{owner}/{repo}", web::get().to(handlers::workflows::get_branches))
                    // Runs (job logs route before {run_id} wildcard to avoid ambiguity)
                    .route("/runs/active", web::get().to(handlers::runs::get_active_runs))
                    .route("/runs/{owner}/{repo}/jobs/{job_id}/logs", web::get().to(handlers::runs::get_job_log))
                    .route("/runs/{owner}/{repo}/jobs/{job_id}/logs/plain", web::get().to(handlers::runs::get_job_logs))
                    .route("/runs/{owner}/{repo}/jobs/{job_id}/steps", web::get().to(handlers::runs::get_job_steps))
                    .route("/runs/{owner}/{repo}", web::get().to(handlers::runs::get_repo_runs))
                    .route("/runs/{owner}/{repo}/{run_id}", web::get().to(handlers::runs::get_run_detail))
                    .route("/runs/{owner}/{repo}/{run_id}/cancel", web::post().to(handlers::runs::cancel_run))
                    .route("/runs/{owner}/{repo}/{run_id}/rerun", web::post().to(handlers::runs::rerun))
                    // Analytics
                    .route("/analytics/summary", web::get().to(handlers::analytics::get_analytics_summary))
                    // Integrations
                    .route("/integrations/slack/message", web::post().to(handlers::integrations::post_slack_message))
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
    tracing::info!("Build: commit={} built_at={}", GIT_HASH, BUILT_AT);

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

/// Build metadata embedded at compile time.
const GIT_HASH: &str = env!("GWD_GIT_HASH");
const BUILT_AT: &str = env!("GWD_BUILT_AT");

/// Serve embedded frontend HTML with build version injected.
async fn index() -> HttpResponse {
    let html = frontend::INDEX_HTML
        .replace("{{GWD_GIT_HASH}}", GIT_HASH)
        .replace("{{GWD_BUILT_AT}}", BUILT_AT);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

/// Return build version info (public, no auth required).
async fn version_info() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/json")
        .body(format!(
            r#"{{"commit":"{}","built_at":"{}","version":"{}"}}"#,
            GIT_HASH,
            BUILT_AT,
            env!("CARGO_PKG_VERSION")
        ))
}

/// Health check endpoint — returns dependency status and key metrics.
async fn healthz(
    state: web::Data<SharedDashboardState>,
    ws_count: web::Data<handlers::ws::WsConnectionCount>,
) -> HttpResponse {
    let dashboard = state.read().await;
    let github_reachable = dashboard.github_reachable;
    let repos_monitored = dashboard.repos.len();
    let last_poll_age_secs = dashboard
        .last_poll
        .map(|t| (chrono::Utc::now() - t).num_seconds().max(0) as u64)
        .unwrap_or(0);
    drop(dashboard);

    let ws_connections = ws_count.load(std::sync::atomic::Ordering::Relaxed);

    let body = serde_json::json!({
        "status": if github_reachable { "ok" } else { "degraded" },
        "github_reachable": github_reachable,
        "last_poll_age_secs": last_poll_age_secs,
        "repos_monitored": repos_monitored,
        "ws_connections": ws_connections,
    });

    if github_reachable {
        HttpResponse::Ok().json(body)
    } else {
        HttpResponse::ServiceUnavailable().json(body)
    }
}

/// Readiness probe — returns 200 if dashboard state has been populated, 503 otherwise.
async fn readyz(state: web::Data<SharedDashboardState>) -> HttpResponse {
    let dashboard = state.read().await;
    if dashboard.last_poll.is_some() {
        HttpResponse::Ok().json(serde_json::json!({"status": "ready"}))
    } else {
        HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "not_ready",
            "reason": "Initial poll has not completed yet"
        }))
    }
}

/// Middleware that generates a UUID for each request and adds it as X-Request-Id header.
async fn request_id_middleware(
    req: actix_web::dev::ServiceRequest,
    next: actix_web::middleware::Next<impl actix_web::body::MessageBody>,
) -> Result<actix_web::dev::ServiceResponse<impl actix_web::body::MessageBody>, actix_web::Error> {
    let request_id = uuid::Uuid::new_v4().to_string();
    let mut response = next.call(req).await?;
    response.headers_mut().insert(
        actix_web::http::header::HeaderName::from_static("x-request-id"),
        actix_web::http::header::HeaderValue::from_str(&request_id).unwrap(),
    );
    Ok(response)
}
