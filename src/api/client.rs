use crate::api::types::{
    ChannelResponse, CreateMessageRequest, EditMessageRequest, GatewayBotResponse, GuildResponse,
    HandoffInitiateResponse, HandoffStatusResponse, MessageQuery, MessageResponse,
    UserGuildSettingsPatch, UserGuildSettingsResponse, UserPrivateResponse, UserSettingsResponse,
    WellKnownFluxerResponse,
};
use anyhow::{Context, Result, anyhow, bail};
use reqwest::{Method, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;
use tokio::time::{Duration, sleep};
use urlencoding;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("{status} {message}")]
    Response {
        status: StatusCode,
        code: Option<String>,
        message: String,
        body: Value,
    },
}

#[derive(Debug, Clone)]
pub struct FluxerHttpClient {
    inner: reqwest::Client,
    base_url: String,
    token: Option<String>,
}

impl FluxerHttpClient {
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        let (os_token, platform_token) = match std::env::consts::OS {
            "linux" => ("Linux", "X11"),
            "macos" => ("Mac OS X", "Macintosh"),
            "windows" => ("Windows NT 10.0", "Windows"),
            other => (other, other),
        };
        let arch = std::env::consts::ARCH;
        let ua = format!(
            "Mozilla/5.0 ({platform_token}; {os_token}; {arch}) FluxerTUI/{}",
            env!("CARGO_PKG_VERSION")
        );

        // isreali GPT was here... Beep Boop. (joke)\

        Ok(Self {
            inner: reqwest::Client::builder()
                .user_agent(ua)
                .build()
                .context("failed to build HTTP client")?,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            token: None,
        })
    }

    pub fn with_token(&self, token: impl Into<String>) -> Self {
        let mut client = self.clone();
        client.token = Some(token.into());
        client
    }

    pub async fn discover(&self) -> Result<WellKnownFluxerResponse> {
        self.send_json::<(), (), WellKnownFluxerResponse>(
            Method::GET,
            "/.well-known/fluxer",
            None::<&()>,
            None::<&()>,
            true,
        )
        .await
    }

    pub async fn gateway_info(&self) -> Result<GatewayBotResponse> {
        self.send_json::<(), (), GatewayBotResponse>(
            Method::GET,
            "/gateway/bot",
            None::<&()>,
            None::<&()>,
            true,
        )
        .await
    }

    pub async fn current_user(&self) -> Result<UserPrivateResponse> {
        self.send_json::<(), (), UserPrivateResponse>(
            Method::GET,
            "/users/@me",
            None::<&()>,
            None::<&()>,
            false,
        )
        .await
    }

    pub async fn current_user_settings(&self) -> Result<UserSettingsResponse> {
        self.send_json::<(), (), UserSettingsResponse>(
            Method::GET,
            "/users/@me/settings",
            None::<&()>,
            None::<&()>,
            false,
        )
        .await
    }

    pub async fn update_user_guild_settings(
        &self,
        guild_id: Option<&str>,
        body: &UserGuildSettingsPatch,
    ) -> Result<UserGuildSettingsResponse> {
        let path = match guild_id {
            Some(guild_id) => format!("/users/@me/guilds/{guild_id}/settings"),
            None => "/users/@me/guilds/@me/settings".to_string(),
        };
        self.send_json::<(), UserGuildSettingsPatch, UserGuildSettingsResponse>(
            Method::PATCH,
            &path,
            None::<&()>,
            Some(body),
            false,
        )
        .await
    }

    pub async fn guilds(&self) -> Result<Vec<GuildResponse>> {
        self.send_json::<(), (), Vec<GuildResponse>>(
            Method::GET,
            "/users/@me/guilds",
            None::<&()>,
            None::<&()>,
            false,
        )
        .await
    }

    pub async fn private_channels(&self) -> Result<Vec<ChannelResponse>> {
        self.send_json::<(), (), Vec<ChannelResponse>>(
            Method::GET,
            "/users/@me/channels",
            None::<&()>,
            None::<&()>,
            false,
        )
        .await
    }

    pub async fn guild_channels(&self, guild_id: &str) -> Result<Vec<ChannelResponse>> {
        self.send_json::<(), (), Vec<ChannelResponse>>(
            Method::GET,
            &format!("/guilds/{guild_id}/channels"),
            None::<&()>,
            None::<&()>,
            false,
        )
        .await
    }

    pub async fn guild_members(
        &self,
        guild_id: &str,
    ) -> Result<Vec<crate::api::types::GuildMemberResponse>> {
        #[derive(Serialize)]
        struct MembersQuery<'a> {
            limit: u32,
            #[serde(skip_serializing_if = "Option::is_none")]
            after: Option<&'a str>,
        }

        let mut all = Vec::new();
        let mut after: Option<String> = None;
        loop {
            let query = MembersQuery {
                limit: 1000,
                after: after.as_deref(),
            };
            let batch = self
                .send_json::<MembersQuery, (), Vec<crate::api::types::GuildMemberResponse>>(
                    Method::GET,
                    &format!("/guilds/{guild_id}/members"),
                    Some(&query),
                    None::<&()>,
                    false,
                )
                .await?;
            let n = batch.len();
            if n == 0 {
                break;
            }
            let last_id = batch.last().unwrap().user.id.clone();
            all.extend(batch);
            if n < 1000 {
                break;
            }
            sleep(Duration::from_millis(400)).await;
            after = Some(last_id);
        }
        Ok(all)
    }

    pub async fn guild_emojis(
        &self,
        guild_id: &str,
    ) -> Result<Vec<crate::api::types::GuildEmojiResponse>> {
        self.send_json::<(), (), Vec<crate::api::types::GuildEmojiResponse>>(
            Method::GET,
            &format!("/guilds/{guild_id}/emojis"),
            None::<&()>,
            None::<&()>,
            false,
        )
        .await
    }

    pub async fn guild_roles(
        &self,
        guild_id: &str,
    ) -> Result<Vec<crate::api::types::GuildRoleResponse>> {
        self.send_json::<(), (), Vec<crate::api::types::GuildRoleResponse>>(
            Method::GET,
            &format!("/guilds/{guild_id}/roles"),
            None::<&()>,
            None::<&()>,
            false,
        )
        .await
    }

    pub async fn patch_current_guild_member_nick(
        &self,
        guild_id: &str,
        nick: Option<&str>,
    ) -> Result<crate::api::types::GuildMemberResponse> {
        let body = match nick {
            Some(s) => serde_json::json!({ "nick": s }),
            None => serde_json::json!({ "nick": serde_json::Value::Null }),
        };
        self.send_json::<(), serde_json::Value, crate::api::types::GuildMemberResponse>(
            Method::PATCH,
            &format!("/guilds/{guild_id}/members/@me"),
            None::<&()>,
            Some(&body),
            false,
        )
        .await
    }

    pub async fn channel_messages(
        &self,
        channel_id: &str,
        query: &MessageQuery,
    ) -> Result<Vec<MessageResponse>> {
        self.send_json::<MessageQuery, (), Vec<MessageResponse>>(
            Method::GET,
            &format!("/channels/{channel_id}/messages"),
            Some(query),
            None::<&()>,
            false,
        )
        .await
    }

    pub async fn send_message(
        &self,
        channel_id: &str,
        body: &CreateMessageRequest,
    ) -> Result<MessageResponse> {
        self.send_json(
            Method::POST,
            &format!("/channels/{channel_id}/messages"),
            None::<&()>,
            Some(body),
            false,
        )
        .await
    }

    pub async fn edit_message(
        &self,
        channel_id: &str,
        message_id: &str,
        content: &str,
    ) -> Result<MessageResponse> {
        let body = EditMessageRequest {
            content: content.to_string(),
        };
        self.send_json(
            Method::PATCH,
            &format!("/channels/{channel_id}/messages/{message_id}"),
            None::<&()>,
            Some(&body),
            false,
        )
        .await
    }

    pub async fn delete_message(&self, channel_id: &str, message_id: &str) -> Result<()> {
        let resp = self
            .inner
            .request(
                Method::DELETE,
                self.url(&format!("/channels/{channel_id}/messages/{message_id}")),
            )
            .header("X-Fluxer-Platform", "desktop")
            .header("Authorization", self.token.as_deref().unwrap_or(""))
            .send()
            .await
            .context("failed to delete message")?;
        if !resp.status().is_success() && resp.status() != StatusCode::NO_CONTENT {
            bail!("delete message failed: {}", resp.status());
        }
        Ok(())
    }

    pub async fn ack_message(&self, channel_id: &str, message_id: &str) -> Result<()> {
        let resp = self
            .inner
            .request(
                Method::POST,
                self.url(&format!("/channels/{channel_id}/messages/{message_id}/ack")),
            )
            .header("X-Fluxer-Platform", "desktop")
            .header("Authorization", self.token.as_deref().unwrap_or(""))
            .send()
            .await
            .context("failed to ack message")?;
        if !resp.status().is_success() && resp.status() != StatusCode::NO_CONTENT {
            bail!("ack failed: {}", resp.status());
        }
        Ok(())
    }

    pub async fn add_reaction(
        &self,
        channel_id: &str,
        message_id: &str,
        emoji: &str,
    ) -> Result<()> {
        let encoded = urlencoding::encode(emoji);
        let resp = self
            .inner
            .request(
                Method::PUT,
                self.url(&format!(
                    "/channels/{channel_id}/messages/{message_id}/reactions/{encoded}/@me"
                )),
            )
            .header("X-Fluxer-Platform", "desktop")
            .header("Authorization", self.token.as_deref().unwrap_or(""))
            .send()
            .await
            .context("failed to add reaction")?;
        if !resp.status().is_success() && resp.status() != StatusCode::NO_CONTENT {
            bail!("add reaction failed: {}", resp.status());
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn remove_reaction(
        &self,
        channel_id: &str,
        message_id: &str,
        emoji: &str,
    ) -> Result<()> {
        let encoded = urlencoding::encode(emoji);
        let resp = self
            .inner
            .request(
                Method::DELETE,
                self.url(&format!(
                    "/channels/{channel_id}/messages/{message_id}/reactions/{encoded}/@me"
                )),
            )
            .header("X-Fluxer-Platform", "desktop")
            .header("Authorization", self.token.as_deref().unwrap_or(""))
            .send()
            .await
            .context("failed to remove reaction")?;
        if !resp.status().is_success() && resp.status() != StatusCode::NO_CONTENT {
            bail!("remove reaction failed: {}", resp.status());
        }
        Ok(())
    }

    pub async fn handoff_initiate(&self) -> Result<HandoffInitiateResponse> {
        self.send_json::<(), (), HandoffInitiateResponse>(
            Method::POST,
            "/auth/handoff/initiate",
            None::<&()>,
            None::<&()>,
            true,
        )
        .await
    }

    pub async fn handoff_status(&self, code: &str) -> Result<HandoffStatusResponse> {
        self.send_json::<(), (), HandoffStatusResponse>(
            Method::GET,
            &format!("/auth/handoff/{code}/status"),
            None::<&()>,
            None::<&()>,
            true,
        )
        .await
    }

    async fn send_json<Q, B, T>(
        &self,
        method: Method,
        path: &str,
        query: Option<&Q>,
        body: Option<&B>,
        skip_auth: bool,
    ) -> Result<T>
    where
        Q: Serialize + ?Sized,
        B: Serialize + ?Sized,
        T: DeserializeOwned,
    {
        let mut builder = self
            .inner
            .request(method, self.url(path))
            .header("X-Fluxer-Platform", "desktop");

        if !skip_auth {
            let token = self
                .token
                .as_deref()
                .ok_or_else(|| anyhow!("authentication token is required for {path}"))?;
            builder = builder.header("Authorization", token);
        }

        if let Some(query) = query {
            builder = builder.query(query);
        }

        if let Some(body) = body {
            builder = builder.json(body);
        }

        let response = builder
            .send()
            .await
            .with_context(|| format!("request failed for {path}"))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let json = serde_json::from_str::<Value>(&body).unwrap_or(Value::Null);
            let code = json
                .get("code")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            let message = json
                .get("message")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| {
                    if body.is_empty() {
                        format!("request to {path} failed")
                    } else {
                        body.clone()
                    }
                });
            return Err(ApiError::Response {
                status,
                code,
                message,
                body: json,
            }
            .into());
        }

        if status == StatusCode::NO_CONTENT {
            bail!("unexpected empty response for {path}");
        }

        response
            .json::<T>()
            .await
            .with_context(|| format!("failed to decode JSON for {path}"))
    }

    pub async fn fetch_url_bytes(&self, url_or_path: &str) -> Result<Vec<u8>> {
        let target = self.url(url_or_path);
        let mut req = self
            .inner
            .get(&target)
            .header("X-Fluxer-Platform", "desktop");
        if let Some(token) = self.token.as_deref() {
            if !token.is_empty() {
                req = req.header("Authorization", token);
            }
        }
        let response = req
            .send()
            .await
            .with_context(|| format!("request failed for {target}"))?;
        let status = response.status();
        if !status.is_success() {
            bail!("fetch failed: {status} ({target})");
        }
        let bytes = response.bytes().await.context("read response body")?;
        Ok(bytes.to_vec())
    }

    fn url(&self, path: &str) -> String {
        if path.starts_with("http://") || path.starts_with("https://") {
            path.to_string()
        } else {
            format!("{}/{}", self.base_url, path.trim_start_matches('/'))
        }
    }
}
