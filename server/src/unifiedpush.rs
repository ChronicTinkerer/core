//! UnifiedPush / Web Push delivery helpers.
//!
//! SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{Context, Result};
use web_push::{
    ContentEncoding, IsahcWebPushClient, SubscriptionInfo, WebPushClient,
    WebPushMessageBuilder,
};

fn validate_https_endpoint_url(raw_url: &str) -> Result<String> {
    let parsed = reqwest::Url::parse(raw_url)
        .with_context(|| format!("Invalid UnifiedPush endpoint URL: {raw_url}"))?;
    if parsed.scheme() != "https" {
        anyhow::bail!("Refusing non-HTTPS UnifiedPush endpoint URL: {raw_url}");
    }
    if parsed.host_str().is_none() {
        anyhow::bail!("UnifiedPush endpoint URL is missing a host: {raw_url}");
    }
    Ok(parsed.to_string())
}

pub async fn send_notification(
    endpoint_url: &str,
    pub_key: &str,
    auth: &str,
    payload: &[u8],
) -> Result<()> {
    let endpoint_url = validate_https_endpoint_url(endpoint_url)?;
    let subscription_info = SubscriptionInfo::new(
        endpoint_url,
        pub_key.to_string(),
        auth.to_string(),
    );

    let mut builder = WebPushMessageBuilder::new(&subscription_info);
    builder.set_payload(ContentEncoding::Aes128Gcm, payload);
    builder.set_ttl(60);

    let client = IsahcWebPushClient::new()?;
    client.send(builder.build()?).await?;
    Ok(())
}
