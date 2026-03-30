//! Notification target persistence and dispatch helpers.
//!
//! SPDX-License-Identifier: GPL-3.0-or-later

use secluso_client_lib::http_client::{HttpClient, NotificationTarget};
use std::fs;
use std::io;
use std::path::Path;

const TARGET_FILENAME: &str = "notification_target.json";

pub fn persist_notification_target(state_dir: &str, target: &NotificationTarget) -> io::Result<()> {
    fs::create_dir_all(state_dir)?;
    let path = Path::new(state_dir).join(TARGET_FILENAME);
    let payload = serde_json::to_vec(target)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    fs::write(path, payload)
}

pub fn load_notification_target(state_dir: &str) -> io::Result<Option<NotificationTarget>> {
    let path = Path::new(state_dir).join(TARGET_FILENAME);
    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(path)?;
    let target = serde_json::from_str::<NotificationTarget>(&raw)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    Ok(Some(target))
}

fn clear_notification_target(state_dir: &str) -> io::Result<()> {
    let path = Path::new(state_dir).join(TARGET_FILENAME);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn refresh_notification_target(
    state_dir: &str,
    http_client: &HttpClient,
) -> Option<NotificationTarget> {
    match http_client.fetch_notification_target() {
        Ok(Some(target)) => {
            if let Err(e) = persist_notification_target(state_dir, &target) {
                error!("Failed to persist notification target: {e}");
            }
            Some(target)
        }
        Ok(None) => {
            let cached = load_notification_target(state_dir).unwrap_or_else(|e| {
                error!("Failed to load cached notification target: {e}");
                None
            });
            if let Some(target) = cached {
                if target.platform.eq_ignore_ascii_case("ios") {
                    let placeholder = NotificationTarget {
                        platform: target.platform.clone(),
                        ios_relay_binding: None,
                        unifiedpush_endpoint_url: None,
                        unifiedpush_pub_key: None,
                        unifiedpush_auth: None,
                    };
                    if let Err(e) = persist_notification_target(state_dir, &placeholder) {
                        error!("Failed to persist iOS notification placeholder: {e}");
                    }
                    return Some(placeholder);
                }
            }
            if let Err(e) = clear_notification_target(state_dir) {
                error!("Failed to clear cached notification target: {e}");
            }
            None
        }
        Err(e) => {
            error!("Failed to fetch notification target: {e}");
            load_notification_target(state_dir).unwrap_or_else(|load_err| {
                error!("Failed to load cached notification target: {load_err}");
                None
            })
        }
    }
}

pub fn send_notification(
    state_dir: &str,
    http_client: &HttpClient,
    notification_msg: Vec<u8>,
) -> io::Result<()> {
    let target = refresh_notification_target(state_dir, http_client);

    if let Some(target) = target {
        if target.platform.eq_ignore_ascii_case("ios") {
            if let Some(binding) = target.ios_relay_binding.as_ref() {
                let result = http_client.send_ios_notification(notification_msg, binding);
                if let Err(e) = result.as_ref() {
                    if e.to_string().contains("Relay error: 403") {
                        let placeholder = NotificationTarget {
                            platform: target.platform.clone(),
                            ios_relay_binding: None,
                            unifiedpush_endpoint_url: None,
                            unifiedpush_pub_key: None,
                            unifiedpush_auth: None,
                        };
                        if let Err(clear_err) = persist_notification_target(state_dir, &placeholder)
                        {
                            error!(
                                "Failed to persist iOS notification placeholder after relay 403: {clear_err}"
                            );
                        }
                    }
                }
                return result;
            }

            info!("Skipping iOS notification; relay binding is not available yet");
            return Ok(());
        }
    }

    http_client.send_fcm_notification(notification_msg)
}
