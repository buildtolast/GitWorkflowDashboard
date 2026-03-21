use actix_web::{web, HttpResponse};

use crate::models::{SlackMessageRequest, SlackMessageResponse};

/// POST /api/integrations/slack/message — relay a Slack incoming webhook message.
pub(crate) async fn post_slack_message(
    body: web::Json<SlackMessageRequest>,
) -> actix_web::Result<HttpResponse> {
    let webhook_url = body.webhook_url.trim();
    let text = body.text.trim();

    if webhook_url.is_empty() {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "webhook_url is required"
        })));
    }
    if text.is_empty() {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "text is required"
        })));
    }

    let parsed = match reqwest::Url::parse(webhook_url) {
        Ok(url) => url,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid webhook URL"
            })));
        }
    };

    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    if parsed.scheme() != "https" || host != "hooks.slack.com" {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Only https://hooks.slack.com webhook URLs are allowed"
        })));
    }

    let response = match reqwest::Client::new()
        .post(parsed)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "text": text }))
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            tracing::error!(error = %e, "Slack webhook request failed");
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Slack request failed: {}", e)
            })));
        }
    };

    let status = response.status();
    let body_text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        tracing::error!(status = %status, body = %body_text, "Slack webhook returned non-success");
        return Ok(HttpResponse::BadGateway().json(serde_json::json!({
            "error": format!("Slack rejected message ({}): {}", status, body_text)
        })));
    }

    Ok(HttpResponse::Ok().json(SlackMessageResponse {
        success: true,
        message: "Message sent to Slack".into(),
    }))
}
