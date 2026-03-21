//! Server-side analytics API endpoint.
//!
//! `GET /api/analytics/summary?range=7d` returns pre-computed analytics
//! from SQLite, replacing client-side computation.

use actix_web::{web, HttpResponse};

use crate::database::{AnalyticsSummary, SharedDatabase};

/// Query params for analytics.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct AnalyticsQuery {
    /// Time range: "1h", "6h", "12h", "1d", "3d", "7d" (default), "30d"
    #[serde(default = "default_range")]
    pub range: String,
}

fn default_range() -> String {
    "7d".to_string()
}

/// Parse a range string into a chrono::Duration.
fn parse_range(range: &str) -> Option<chrono::Duration> {
    let range = range.trim().to_lowercase();
    if range.ends_with('h') {
        let hours: i64 = range.trim_end_matches('h').parse().ok()?;
        Some(chrono::Duration::hours(hours))
    } else if range.ends_with('d') {
        let days: i64 = range.trim_end_matches('d').parse().ok()?;
        Some(chrono::Duration::days(days))
    } else {
        None
    }
}

/// GET /api/analytics/summary?range=7d
///
/// Returns a comprehensive analytics summary computed server-side from SQLite.
/// Supports time ranges: 1h, 6h, 12h, 1d, 3d, 7d (default), 30d.
/// Results are cached for 60 seconds to reduce computation overhead.
pub(crate) async fn get_analytics_summary(
    query: web::Query<AnalyticsQuery>,
    db: web::Data<SharedDatabase>,
) -> actix_web::Result<HttpResponse> {
    let duration = match parse_range(&query.range) {
        Some(d) => d,
        None => {
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid range '{}'. Use: 1h, 6h, 12h, 1d, 3d, 7d, 30d", query.range)
            })));
        }
    };

    let cache_key = format!("analytics_summary_{}", query.range);
    let since = chrono::Utc::now() - duration;
    let range_clone = query.range.clone();
    let db_clone = db.as_ref().clone();

    // Run all SQLite work (cache check, queries, cache set) on a blocking thread
    let result = tokio::task::spawn_blocking(move || -> Result<(String, bool), String> {
        let database = match db_clone.as_ref() {
            Some(d) => d,
            None => return Err("Database not available".to_string()),
        };

        // Check cache first (60-second TTL)
        if let Some(cached) = database.get_analytics_cache(&cache_key, 60) {
            return Ok((cached, true));
        }

        // Compute all analytics from SQLite
        let summary = AnalyticsSummary {
            time_range: range_clone,
            success_rate: database.analytics_success_rate(&since).map_err(|e| format!("Analytics query failed: {e}"))?,
            duration: database.analytics_duration(&since).map_err(|e| format!("Analytics query failed: {e}"))?,
            triggers: database.analytics_triggers(&since).map_err(|e| format!("Analytics query failed: {e}"))?,
            actors: database.analytics_actors(&since).map_err(|e| format!("Analytics query failed: {e}"))?,
            failures: database.analytics_failures(&since).map_err(|e| format!("Analytics query failed: {e}"))?,
            branches: database.analytics_branches(&since).map_err(|e| format!("Analytics query failed: {e}"))?,
        };

        let json = serde_json::to_string(&summary)
            .map_err(|_| "Failed to serialize analytics".to_string())?;

        // Cache the result
        database.set_analytics_cache(&cache_key, &json);

        Ok((json, false))
    }).await;

    match result {
        Ok(Ok((json, _cached))) => {
            Ok(HttpResponse::Ok()
                .content_type("application/json")
                .body(json))
        }
        Ok(Err(e)) => {
            tracing::error!(error = %e, "Analytics computation failed");
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Analytics computation failed"
            })))
        }
        Err(e) => {
            tracing::error!(error = %e, "Analytics task panicked");
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Analytics computation failed"
            })))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_range_hours() {
        assert_eq!(parse_range("1h"), Some(chrono::Duration::hours(1)));
        assert_eq!(parse_range("6h"), Some(chrono::Duration::hours(6)));
        assert_eq!(parse_range("12h"), Some(chrono::Duration::hours(12)));
    }

    #[test]
    fn test_parse_range_days() {
        assert_eq!(parse_range("1d"), Some(chrono::Duration::days(1)));
        assert_eq!(parse_range("3d"), Some(chrono::Duration::days(3)));
        assert_eq!(parse_range("7d"), Some(chrono::Duration::days(7)));
        assert_eq!(parse_range("30d"), Some(chrono::Duration::days(30)));
    }

    #[test]
    fn test_parse_range_invalid() {
        assert_eq!(parse_range("abc"), None);
        assert_eq!(parse_range(""), None);
        assert_eq!(parse_range("7x"), None);
    }

    #[test]
    fn test_parse_range_case_insensitive() {
        assert_eq!(parse_range("7D"), Some(chrono::Duration::days(7)));
        assert_eq!(parse_range("6H"), Some(chrono::Duration::hours(6)));
    }

    #[test]
    fn test_default_range() {
        assert_eq!(default_range(), "7d");
    }
}
