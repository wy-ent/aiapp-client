//! Anonymous event reporting client (telemetry, reserved for incentives/ad-revenue-share/billing).
//!
//! The open-source side (on-prem enterprise backend) only **produces** anonymous usage events,
//! asynchronously reporting them to the closed-source Pro service at
//! `POST {pro_url}/v1/telemetry` (`aiapp-pro-server`), where the closed-source side aggregates stats.
//!
//! Event definitions and fields follow a versioned schema (see `docs/telemetry.schema.v1.md`),
//! evolving independently on each side. Failed reports are silent (do not block the main flow);
//! an enterprise can set `AIAPP_TELEMETRY=false` to disable reporting (pure private mode, without
//! revenue-share / cloud statistics).

use serde_json::json;

/// Event types (aligned with `docs/telemetry.schema.v1.md`).
pub const EVENT_TYPES: &[&str] = &[
    "user_register", // new user registration
    "app_generate",  // user generates an app
    "app_launch",    // app launch (open)
    "app_report",    // app reported/flagged
    "app_publish",   // app published/shared
];

/// Telemetry client.
#[derive(Clone)]
pub struct Telemetry {
    /// Pro service base URL (`AIAPP_PRO_URL`); empty means disabled.
    pro_url: String,
    /// Pro service API Key (optional, `X-Api-Key` header).
    api_key: String,
    /// Whether reporting is enabled (`AIAPP_TELEMETRY`, default true; set false to disable).
    enabled: bool,
}

impl Telemetry {
    /// Whether reporting is enabled (Pro URL configured and not explicitly disabled).
    pub fn enabled(&self) -> bool {
        self.enabled && !self.pro_url.is_empty()
    }

    /// Report a single anonymous event (async fire-and-forget, failures silent).
    ///
    /// `kind` is the event type (see [`EVENT_TYPES`]), `detail` is an optional structured
    /// supplement. The event body is uniformly wrapped as
    /// `{ schema_version, type, ts, detail }`.
    pub fn report(&self, kind: &str, detail: serde_json::Value) {
        if !self.enabled() {
            return;
        }
        let url = format!(
            "{}/v1/telemetry",
            self.pro_url.trim_end_matches('/')
        );
        let api_key = self.api_key.clone();
        let event = json!({
            "schema_version": 1,
            "type": kind,
            "ts": crate::chrono_now(),
            "detail": detail,
        });
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let mut req = client
                .post(&url)
                .timeout(std::time::Duration::from_secs(5))
                .json(&event);
            if !api_key.is_empty() {
                req = req.header("X-Api-Key", &api_key);
            }
            let _ = req.send().await;
        });
    }

    /// Build the client (disabled when no Pro URL is configured).
    pub fn new(pro_url: String, api_key: String, enabled: bool) -> Self {
        Telemetry {
            pro_url,
            api_key,
            enabled,
        }
    }
}

/// Convenience constructor: reads configuration from environment variables.
pub fn from_env() -> Telemetry {
    let pro_url = std::env::var("AIAPP_PRO_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_default();
    let api_key = std::env::var("AIAPP_PRO_API_KEY")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_default();
    let enabled = std::env::var("AIAPP_TELEMETRY")
        .ok()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .map(|s| s != "0" && s != "false" && s != "off")
        .unwrap_or(true);
    if !pro_url.is_empty() {
        println!(
            "[telemetry] Anonymous event reporting: {} (Pro service {pro_url}), AIAPP_TELEMETRY={enabled}",
            if enabled { "enabled" } else { "disabled" }
        );
    }
    Telemetry::new(pro_url, api_key, enabled)
}
