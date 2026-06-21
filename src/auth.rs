use crate::twitch_auth::{
    poll_for_tokens, refresh_access_token, request_device_code, save_tokens, save_tokens_to_file,
    validate_token,
};
use crate::{App, DeviceCodeInfo, Message};
use iced::Task;
use tracing::info;

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
    TokenValidated(Option<String>),
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
            TokenValidated(valid) => {
                info!("Token validation result: {:?}", valid);
                if valid.is_some() {
                    self.auth_status = "Authenticated".to_string();
                    self.broadcaster_id = valid;
                    Task::none()
                } else {
                    self.auth_status = "Token Expired, refreshing...".to_string();
                    if self.refresh_token.is_some() {
                        info!("Refreshing token...");
                        Task::done(Message::Auth(RefreshToken))
                    } else {
                        info!("No refresh token, starting auth...");
                        Task::done(Message::Auth(StartAuth))
                    }
                }
            }
            RefreshToken => {
                if let Some(refresh) = &self.refresh_token {
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
                self.confirm = Some(String::from(
                    "Could not save tokens to the Operating System's keystore.\nDo you want the tokens saved to a file?\nIf \"No\", you'll have to re-authenticate every time you restart the app.",
                ));
                Task::none()
            }
            FallbackConfirmed(confirmed) => {
                self.confirm = None;
                if confirmed {
                    let _ = save_tokens_to_file(
                        &self.access_token.clone().unwrap(),
                        &self.refresh_token.clone().unwrap(),
                        &self.config_path,
                    );
                }
                Task::none()
            }
        }
    }
}
