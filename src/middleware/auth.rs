use std::future::{ready, Ready, Future};
use std::pin::Pin;

use actix_web::body::EitherBody;
use actix_web::dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{web, Error, HttpResponse};

/// Authentication middleware that validates Bearer token on API endpoints.
/// When no dashboard token is configured, all requests are allowed (backward compatible).
pub struct AuthMiddleware;

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

pub struct AuthMiddlewareService<S> {
    service: S,
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
            .and_then(|t| t.as_ref().as_ref().map(|s| s.clone()));

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

        // Validate X-Requested-With header (CSRF protection)
        let has_csrf_header = req
            .headers()
            .get("x-requested-with")
            .and_then(|v| v.to_str().ok())
            .map(|v| v == "XMLHttpRequest")
            .unwrap_or(false);

        if !has_csrf_header {
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
            Some(ref token) if token == &expected_token => {
                // Valid token — proceed
                let fut = self.service.call(req);
                Box::pin(async move {
                    fut.await.map(|res| res.map_into_left_body())
                })
            }
            _ => {
                let response = HttpResponse::Unauthorized()
                    .json(serde_json::json!({"error": "Invalid or missing authentication token"}));
                Box::pin(async move {
                    Ok(req.into_response(response).map_into_right_body())
                })
            }
        }
    }
}
