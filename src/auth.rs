use crate::twitch_auth::{
    poll_for_tokens, refresh_access_token, request_device_code, save_tokens, save_tokens_to_file,
    validate_token,
};
use crate::{App, Confirm, Message};
use iced::Task;
use std::time::Duration;
use tracing::{info, warn};

/// Delay before re-trying token validation after a transient (network/5xx) failure.
const VALIDATE_RETRY_DELAY: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub enum AuthMessage {
    StartAuth,
    DeviceCodeReceived(Result<DeviceCodeInfo, String>),
    PollForTokens {
        device_code: String,
        interval: u64,
        expires_in: u64,
    },
    AuthCompleted(Result<(String, String), String>),
    ConfirmFallback,
    FallbackConfirmed(bool),
    ValidateToken,
    TokenValidated(Result<Option<String>, String>),
    RefreshToken,
}
impl App {
    pub fn handle_auth(&mut self, auth_message: AuthMessage) -> Task<Message> {
        use crate::auth::AuthMessage::*;
        match auth_message {
            StartAuth => {
                self.auth_status = "Requesting device code...".to_string();
                self.auth_in_progress = true;
                self.device_code_info = None;
                self.refresh_attempted = false;

                let client = self.client.clone();
                Task::perform(
                    async move {
                        match request_device_code(&client).await {
                            Ok(resp) => {
                                // Open browser to verification URL
                                let _ = open::that(&resp.verification_uri);
                                Ok(DeviceCodeInfo {
                                    verification_uri: resp.verification_uri,
                                    user_code: resp.user_code,
                                    device_code: resp.device_code,
                                    interval: resp.interval,
                                    expires_in: resp.expires_in,
                                })
                            }
                            Err(e) => Err(e),
                        }
                    },
                    |result| Message::Auth(DeviceCodeReceived(result)),
                )
            }
            DeviceCodeReceived(result) => {
                match result {
                    Ok(info) => {
                        self.auth_status = format!(
                            "Go to {} and enter code: {}",
                            info.verification_uri, info.user_code
                        );
                        let device_code = info.device_code.clone();
                        let interval = info.interval;
                        let expires_in = info.expires_in;
                        self.device_code_info = Some(info);

                        // Start polling for tokens
                        Task::done(Message::Auth(PollForTokens {
                            device_code,
                            interval,
                            expires_in,
                        }))
                    }
                    Err(e) => {
                        self.auth_status = format!("Error: {}", e);
                        self.auth_in_progress = false;
                        Task::none()
                    }
                }
            }
            PollForTokens {
                device_code,
                interval,
                expires_in,
            } => {
                let client = self.client.clone();
                Task::perform(
                    async move { poll_for_tokens(&client, &device_code, interval, expires_in).await },
                    |result| Message::Auth(AuthCompleted(result)),
                )
            }
            AuthCompleted(res) => {
                self.auth_in_progress = false;
                self.device_code_info = None;
                match res {
                    Ok((access_token, refresh_token)) => {
                        let resp = save_tokens(&access_token, &refresh_token);
                        self.access_token = Some(access_token);
                        self.refresh_token = Some(refresh_token);
                        self.auth_status = "Authenticated".to_string();
                        match resp {
                            Ok(_) => Task::done(Message::Auth(ValidateToken)),
                            Err(_) => Task::done(Message::Auth(ConfirmFallback)),
                        }
                    }
                    Err(e) => {
                        self.auth_status = format!("Error: {}", e);
                        Task::none()
                    }
                }
            }
            ValidateToken => {
                if let Some(token) = &self.access_token {
                    let t = token.clone();
                    let client = self.client.clone();
                    Task::perform(async move { validate_token(&client, &t).await }, |result| {
                        Message::Auth(TokenValidated(result))
                    })
                } else {
                    Task::none()
                }
            }
            TokenValidated(result) => {
                info!("Token validation result: {:?}", result);
                match result {
                    Ok(Some(user_id)) => {
                        self.auth_status = "Authenticated".to_string();
                        self.broadcaster_id = Some(user_id);
                        self.refresh_attempted = false;
                        Task::none()
                    }
                    // Twitch confirmed the token is invalid (401): refreshing is safe.
                    Ok(None) => {
                        if self.refresh_token.is_some() && !self.refresh_attempted {
                            info!("Token invalid, refreshing...");
                            self.auth_status = "Token expired, refreshing...".to_string();
                            Task::done(Message::Auth(RefreshToken))
                        } else {
                            // No refresh token, or a freshly refreshed token failed
                            // validation too — don't loop, start a new device flow.
                            info!("Token invalid and not refreshable, starting auth...");
                            Task::done(Message::Auth(StartAuth))
                        }
                    }
                    // Transient failure (network, 5xx): the token may still be valid, retrying
                    Err(e) => {
                        warn!("Token validation failed transiently: {e}");
                        self.auth_status = "Could not reach Twitch, retrying...".to_string();
                        Task::perform(tokio::time::sleep(VALIDATE_RETRY_DELAY), |_| {
                            Message::Auth(ValidateToken)
                        })
                    }
                }
            }
            RefreshToken => {
                if let Some(refresh) = &self.refresh_token {
                    self.refresh_attempted = true;
                    let t = refresh.clone();
                    let client = self.client.clone();
                    Task::perform(
                        async move { refresh_access_token(&client, &t).await },
                        |result| Message::Auth(AuthCompleted(result)),
                    )
                } else {
                    Task::none()
                }
            }
            ConfirmFallback => {
                self.confirm = Some(Confirm {
                    message: String::from(
                        "Could not save tokens to the Operating System's keystore.\nDo you want the tokens saved to a file? This is not recommended since it might leak the tokens to other processes.\nIf \"No\", you'll have to re-authenticate every time you restart the app.",
                    ),
                    on_yes: Box::new(Message::Auth(AuthMessage::FallbackConfirmed(true))),
                    on_no: Some(Box::new(Message::Auth(AuthMessage::FallbackConfirmed(false)))),
                });
                Task::none()
            }
            FallbackConfirmed(confirmed) => {
                if confirmed
                    && let (Some(access), Some(refresh)) =
                        (self.access_token.clone(), self.refresh_token.clone())
                {
                    let _ = save_tokens_to_file(&access, &refresh, &self.config_path);
                }
                // if saving failed, still validate to have usable app
                Task::done(Message::Auth(ValidateToken))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeviceCodeInfo {
    pub verification_uri: String,
    pub user_code: String,
    device_code: String,
    interval: u64,
    expires_in: u64,
}
