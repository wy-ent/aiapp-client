//! Tencent Cloud Object Storage (COS) backend implementation.
//!
//! Uses the COS XML API **V5 query-string signature** (q-sign-algorithm=sha1):
//! the same signing logic is used for the "pre-signed download URL", the
//! "pre-signed upload URL", and regular server-side Put/Get, so there is no need
//! to maintain separate Header-signature and Query-signature code paths.
//!
//! Key point (a common pitfall in community implementations): the `;` separators in
//! `q-sign-time` / `q-key-time` **must not** be percent-encoded, otherwise COS
//! returns 403 AccessDenied (Request has expired).
//!
//! Pure-Rust TLS (reqwest + rustls), so the runtime image needs no OpenSSL installed.

use anyhow::{Context, Result};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::storage::Storage;

type HmacSha1 = Hmac<Sha1>;

/// COS connection config (credentials come from environment-variable placeholders, injected by config).
#[derive(Clone, Debug)]
pub struct CosConfig {
    pub secret_id: String,
    pub secret_key: String,
    /// Bucket name (including APPID), e.g. `aiapp-1250000000`
    pub bucket: String,
    /// Region, e.g. `ap-guangzhou`
    pub region: String,
}

impl CosConfig {
    /// Validate that credentials are complete; return a readable error if not.
    fn ensure_complete(&self) -> Result<()> {
        if self.secret_id.is_empty()
            || self.secret_key.is_empty()
            || self.bucket.is_empty()
            || self.region.is_empty()
        {
            anyhow::bail!(
                "COS credentials incomplete: set COS_SECRET_ID / COS_SECRET_KEY / COS_BUCKET / COS_REGION (see .env.example)"
            );
        }
        Ok(())
    }

    /// Base URL for object access: `https://{bucket}.cos.{region}.myqcloud.com`
    fn base_url(&self) -> String {
        format!("https://{}.cos.{}.myqcloud.com", self.bucket, self.region)
    }
}

/// COS storage backend.
pub struct CosStorage {
    cfg: CosConfig,
    client: reqwest::Client,
    /// Default pre-signed validity period (seconds)
    default_expiry: u64,
}

impl CosStorage {
    pub fn new(cfg: CosConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build reqwest client");
        CosStorage {
            cfg,
            client,
            default_expiry: 600,
        }
    }

    /// Generate a V5 query-string-signed URL (method is lower-case like get/put).
    fn presign(&self, method: &str, key: &str, expire_secs: u64) -> Result<String> {
        self.cfg.ensure_complete()?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let end = now + expire_secs;
        let sign_time = format!("{};{}", now, end);
        // sign-key = HMAC-SHA1(secret_key, q-key-time)
        let sign_key = hmac_sha1_hex(self.cfg.secret_key.as_bytes(), sign_time.as_bytes());

        // URI path: only percent-encode the key (keep `/` as the path separator)
        let encoded_key = percent_encode(key);
        let uri_path = format!("/{}", encoded_key);

        // Query signature: only sign the q-* fields themselves; no extra headers/params involved
        let http_method = method.to_lowercase();
        let http_parameters = "";
        let http_headers = "";
        let http_string = format!("{}\n{}\n{}\n{}\n", http_method, uri_path, http_parameters, http_headers);
        let sha1_http = sha1_hex(http_string.as_bytes());
        let string_to_sign = format!("sha1\n{}\n{}\n", sign_time, sha1_http);
        let signature = hmac_sha1_hex(sign_key.as_bytes(), string_to_sign.as_bytes());

        let url = format!(
            "{}{}?q-sign-algorithm=sha1&q-ak={}&q-sign-time={}&q-key-time={}&q-header-list=&q-url-param-list=&q-signature={}",
            self.cfg.base_url(),
            uri_path,
            self.cfg.secret_id,
            sign_time,
            sign_time,
            signature
        );
        Ok(url)
    }

    async fn get_bytes_inner(&self, key: &str) -> Result<Vec<u8>> {
        let url = self.presign("get", key, self.default_expiry)?;
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .context("failed to request COS (download object)")?;
        if !resp.status().is_success() {
            anyhow::bail!("COS download failed: HTTP {}", resp.status());
        }
        let bytes = resp.bytes().await.context("failed to read COS response body")?;
        Ok(bytes.to_vec())
    }

    async fn put_inner(&self, key: &str, data: &[u8]) -> Result<()> {
        let url = self.presign("put", key, self.default_expiry)?;
        let resp = self
            .client
            .put(&url)
            .header(reqwest::header::CONTENT_TYPE, "application/wasm")
            .body(data.to_vec())
            .send()
            .await
            .context("failed to request COS (upload object)")?;
        if !resp.status().is_success() {
            anyhow::bail!("COS upload failed: HTTP {}", resp.status());
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl Storage for CosStorage {
    async fn put(&self, key: &str, data: &[u8]) -> Result<()> {
        self.put_inner(key, data).await
    }

    async fn get_bytes(&self, key: &str) -> Result<Vec<u8>> {
        self.get_bytes_inner(key).await
    }

    fn scheme(&self) -> &str {
        "cos"
    }

    /// Generate a pre-signed download URL (URL pre-authorization).
    async fn presigned_url(&self, key: &str, expires_secs: u64) -> Result<Option<String>> {
        let url = self.presign("get", key, expires_secs)?;
        Ok(Some(url))
    }
}

/// HMAC-SHA1 and return a hex string.
fn hmac_sha1_hex(key: &[u8], data: &[u8]) -> String {
    let mut mac = HmacSha1::new_from_slice(key).expect("HMAC accepts keys of any length");
    mac.update(data);
    hex::encode(mac.finalize().into_bytes())
}

/// SHA1 hex.
fn sha1_hex(data: &[u8]) -> String {
    use sha1::Digest;
    let mut h = Sha1::new();
    h.update(data);
    hex::encode(h.finalize())
}

/// URL-encode an object key: keep unreserved chars and `/`, percent-encode the rest as `%XX`.
fn percent_encode(input: &str) -> String {
    const UNRESERVED: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.~";
    let mut out = String::with_capacity(input.len());
    for b in input.bytes() {
        if b == b'/' || UNRESERVED.contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}
