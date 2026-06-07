use reqwest::Client;
use serde::Deserialize;

pub const CLIENT_ID: &str = "9w729lqufngx4sztgex20eztz7o879";

pub struct TwitchClient {
    client: Client,
    client_id: String,
    access_token: String,
}

#[derive(Debug, Deserialize)]
pub struct User {
    pub id: String,
    pub login: String,
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
struct HelixResponse<T> {
    data: Vec<T>,
}

impl TwitchClient {
    pub fn new(client_id: String, access_token: String) -> Self {
        Self {
            client: Client::new(),
            client_id,
            access_token,
        }
    }

    pub async fn get_user(&self, login: &str) -> Result<Option<User>, reqwest::Error> {
        let resp: HelixResponse<User> = self.client
            .get("https://api.twitch.tv/helix/users")
            .query(&[("login", login)])
            .header("Client-Id", &self.client_id)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.data.into_iter().next())
    }
}