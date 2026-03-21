pub mod client;
pub mod log_parser;
pub mod poller;
pub mod webhook;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// ETag cache: maps URL -> (etag_value, cached_response_body).
/// Used for conditional requests to avoid GitHub rate limit consumption.
pub type ETagCache = Arc<RwLock<HashMap<String, (String, String)>>>;
