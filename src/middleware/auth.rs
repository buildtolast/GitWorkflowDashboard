use std::collections::HashMap;
use std::future::{ready, Ready, Future};
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::Mutex;
use std::time::Instant;

use subtle::ConstantTimeEq;
use actix_web::body::EitherBody;
use actix_web::dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{web, Error, HttpResponse};

/// Maximum failed auth attempts per IP before returning 429.
const MAX_FAILED_ATTEMPTS: u32 = 10;

/// Window in seconds for tracking failed attempts.
const RATE_LIMIT_WINDOW_SECS: u64 = 300; // 5 minutes

/// Per-IP failed auth attempt tracker: (count, window_start).
pub type AuthRateLimitState = Mutex<HashMap<IpAddr, (u32, Instant)>>;

/// Create a new rate limit state for use as app_data.
pub fn new_rate_limit_state() -> AuthRateLimitState {
    Mutex::new(HashMap::new())
}

/// Authentication middleware that validates Bearer token on API endpoints.
/// When no dashboard token is configured, all requests are allowed (backward compatible).
/// Includes per-IP rate limiting for failed auth attempts.
pub(crate) struct AuthMiddleware;

impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddlewareService { service }))
    }
}

pub(crate) struct AuthMiddlewareService<S> {
    service: S,
}

/// Extract client IP from the request (peer_addr).
fn client_ip(req: &ServiceRequest) -> Option<IpAddr> {
    req.peer_addr().map(|addr| addr.ip())
}

/// Check if an IP is rate-limited and return true if the request should be blocked.
fn is_rate_limited(state: &AuthRateLimitState, ip: IpAddr) -> bool {
    let mut map = state.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((count, window_start)) = map.get(&ip) {
        if window_start.elapsed().as_secs() >= RATE_LIMIT_WINDOW_SECS {
            // Window expired — remove entry
            map.remove(&ip);
            false
        } else {
            *count >= MAX_FAILED_ATTEMPTS
        }
    } else {
        false
    }
}

/// Record a failed auth attempt for the given IP.
fn record_failure(state: &AuthRateLimitState, ip: IpAddr) {
    let mut map = state.lock().unwrap_or_else(|e| e.into_inner());
    let entry = map.entry(ip).or_insert((0, Instant::now()));
    if entry.1.elapsed().as_secs() >= RATE_LIMIT_WINDOW_SECS {
        // Reset window
        *entry = (1, Instant::now());
    } else {
        entry.0 += 1;
    }
}

/// Reset the failure counter for a given IP on successful auth.
fn record_success(state: &AuthRateLimitState, ip: IpAddr) {
    let mut map = state.lock().unwrap_or_else(|e| e.into_inner());
    map.remove(&ip);
}

impl<S, B> Service<ServiceRequest> for AuthMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Skip auth for webhook (has its own HMAC auth) and OPTIONS (CORS preflight)
        let path = req.path().to_string();
        let method = req.method().clone();

        if path.ends_with("/webhook") || method == actix_web::http::Method::OPTIONS {
            let fut = self.service.call(req);
            return Box::pin(async move {
                fut.await.map(|res| res.map_into_left_body())
            });
        }

        // Check if dashboard token is configured
        let expected_token = req
            .app_data::<web::Data<Option<String>>>()
            .and_then(|t| t.as_ref().as_ref().cloned());

        let expected_token = match expected_token {
            Some(token) => token,
            None => {
                // No token configured — allow all requests
                let fut = self.service.call(req);
                return Box::pin(async move {
                    fut.await.map(|res| res.map_into_left_body())
                });
            }
        };

        // Get client IP and rate limit state
        let ip = client_ip(&req);
        let rate_state = req
            .app_data::<web::Data<AuthRateLimitState>>()
            .cloned();

        // Check rate limit
        if let (Some(ip), Some(state)) = (ip, &rate_state)
            && is_rate_limited(state, ip)
        {
            tracing::warn!(ip = %ip, "Auth rate limit exceeded");
            let response = HttpResponse::TooManyRequests()
                .json(serde_json::json!({"error": "Too many failed authentication attempts. Try again later."}));
            return Box::pin(async move {
                Ok(req.into_response(response).map_into_right_body())
            });
        }

        // Validate X-Requested-With header (CSRF protection)
        let has_csrf_header = req
            .headers()
            .get("x-requested-with")
            .and_then(|v| v.to_str().ok())
            .map(|v| v == "XMLHttpRequest")
            .unwrap_or(false);

        if !has_csrf_header {
            if let (Some(ip), Some(state)) = (ip, &rate_state) {
                record_failure(state, ip);
            }
            let response = HttpResponse::Forbidden()
                .json(serde_json::json!({"error": "Missing X-Requested-With header"}));
            return Box::pin(async move {
                Ok(req.into_response(response).map_into_right_body())
            });
        }

        // Extract Bearer token from Authorization header
        let auth_header = req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let provided_token = auth_header
            .as_deref()
            .and_then(|h| h.strip_prefix("Bearer "))
            .map(|s| s.to_string());

        match provided_token {
            Some(ref token) if token.as_bytes().ct_eq(expected_token.as_bytes()).into() => {
                // Valid token — reset rate limit counter and proceed
                if let (Some(ip), Some(state)) = (ip, &rate_state) {
                    record_success(state, ip);
                }
                let fut = self.service.call(req);
                Box::pin(async move {
                    fut.await.map(|res| res.map_into_left_body())
                })
            }
            _ => {
                if let (Some(ip), Some(state)) = (ip, &rate_state) {
                    record_failure(state, ip);
                }
                let response = HttpResponse::Unauthorized()
                    .json(serde_json::json!({"error": "Invalid or missing authentication token"}));
                Box::pin(async move {
                    Ok(req.into_response(response).map_into_right_body())
                })
            }
        }
    }
}
