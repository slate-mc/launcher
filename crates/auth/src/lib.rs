//! Microsoft, Xbox, and Minecraft identity for the slate desktop launcher.
//!
//! This crate is deliberately independent of Tauri and SQLite. It owns network
//! protocol details and OS credential-vault access while exposing only explicit
//! secret-bearing Rust types to trusted native callers.

mod wire;

use wire::*;

use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use url::Url;
use uuid::Uuid;

const MICROSOFT_AUTHORIZE_ORIGIN: &str = "https://login.microsoftonline.com";
const MICROSOFT_TOKEN_ORIGIN: &str = "https://login.microsoftonline.com";
const XBOX_USER_AUTH_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_AUTH_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MINECRAFT_LOGIN_URL: &str =
    "https://api.minecraftservices.com/authentication/login_with_xbox";
const MINECRAFT_ENTITLEMENTS_URL: &str = "https://api.minecraftservices.com/entitlements/mcstore";
const MINECRAFT_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const MINECRAFT_SESSION_PROFILE_ORIGIN: &str = "https://sessionserver.mojang.com";
const XBOX_SCOPE: &str = "XboxLive.signin offline_access";
const CALLBACK_MAX_BYTES: usize = 16 * 1024;
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_SESSION_PROFILE_BYTES: usize = 64 * 1024;
const CREDENTIAL_SERVICE: &str = "dev.slate.launcher.minecraft";
const CREDENTIAL_PREFIX: &str = "microsoft-refresh:";
const CALLBACK_PORT: u16 = 38_643;

/// The allow-listed public application identifier for slate's native launcher.
/// Public OAuth clients intentionally ship their client ID and never ship a secret.
pub const SLATE_MICROSOFT_CLIENT_ID: &str = "73823938-7288-4e14-9ede-33d98af655cf";
pub const MICROSOFT_CONSUMER_TENANT: &str = "consumers";
pub const SLATE_MICROSOFT_REDIRECT_URI: &str = "http://localhost:38643";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MinecraftAuthConfig {
    pub client_id: String,
    pub tenant: String,
}

impl MinecraftAuthConfig {
    pub fn new(client_id: impl Into<String>, tenant: impl Into<String>) -> Result<Self, AuthError> {
        let client_id = client_id.into();
        let tenant = tenant.into();
        Uuid::parse_str(&client_id).map_err(|_| AuthError::InvalidClientId)?;
        if tenant != "consumers" {
            return Err(AuthError::InvalidTenant);
        }
        Ok(Self { client_id, tenant })
    }
}

#[derive(Clone)]
pub struct MinecraftAuthClient {
    config: MinecraftAuthConfig,
    http: reqwest::Client,
}

impl std::fmt::Debug for MinecraftAuthClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MinecraftAuthClient")
            .field("tenant", &self.config.tenant)
            .field("client_id", &"<public-client-id>")
            .finish_non_exhaustive()
    }
}

impl MinecraftAuthClient {
    pub fn new(config: MinecraftAuthConfig) -> Result<Self, AuthError> {
        let http = reqwest::Client::builder()
            .user_agent(format!(
                "slate/{} (Minecraft authentication)",
                env!("CARGO_PKG_VERSION")
            ))
            .connect_timeout(Duration::from_secs(15))
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self { config, http })
    }

    pub async fn begin_login(&self) -> Result<(AuthFlowStart, PendingMinecraftLogin), AuthError> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, CALLBACK_PORT))
            .await
            .map_err(|source| AuthError::CallbackPortUnavailable {
                port: CALLBACK_PORT,
                source,
            })?;
        let redirect_uri = SLATE_MICROSOFT_REDIRECT_URI.to_owned();
        let flow_id = Uuid::new_v4();
        let (authorization_url, state, verifier) = self.authorization_request(&redirect_uri)?;

        Ok((
            AuthFlowStart {
                flow_id,
                authorization_url,
                expires_in: CALLBACK_TIMEOUT,
            },
            PendingMinecraftLogin {
                client: self.clone(),
                listener,
                redirect_uri,
                expected_state: state,
                verifier,
            },
        ))
    }

    fn authorization_request(
        &self,
        redirect_uri: &str,
    ) -> Result<(Url, String, SecretString), AuthError> {
        let state = random_url_secret();
        let verifier = random_url_secret();
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let mut authorization_url = Url::parse(&format!(
            "{MICROSOFT_AUTHORIZE_ORIGIN}/{}/oauth2/v2.0/authorize",
            self.config.tenant
        ))?;
        authorization_url
            .query_pairs_mut()
            .append_pair("client_id", &self.config.client_id)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("response_mode", "query")
            .append_pair("scope", XBOX_SCOPE)
            .append_pair("state", &state)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("prompt", "select_account");
        Ok((authorization_url, state, SecretString::from(verifier)))
    }

    pub async fn refresh_session(
        &self,
        refresh_token: &SecretString,
    ) -> Result<RefreshedMinecraftSession, AuthError> {
        let oauth = self.refresh_microsoft_token(refresh_token).await?;
        let session = self.minecraft_session(&oauth.access_token).await?;
        Ok(RefreshedMinecraftSession {
            session,
            replacement_refresh_token: oauth.refresh_token.map(SecretString::from),
        })
    }

    async fn exchange_authorization_code(
        &self,
        code: &SecretString,
        redirect_uri: &str,
        verifier: &SecretString,
    ) -> Result<OAuthToken, AuthError> {
        let endpoint = format!(
            "{MICROSOFT_TOKEN_ORIGIN}/{}/oauth2/v2.0/token",
            self.config.tenant
        );
        let response = self
            .http
            .post(endpoint)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(form_body(&[
                ("client_id", self.config.client_id.as_str()),
                ("code", code.expose_secret()),
                ("redirect_uri", redirect_uri),
                ("grant_type", "authorization_code"),
                ("code_verifier", verifier.expose_secret()),
                ("scope", XBOX_SCOPE),
            ]))
            .send()
            .await?;
        parse_oauth_response(response).await
    }

    async fn refresh_microsoft_token(
        &self,
        refresh_token: &SecretString,
    ) -> Result<OAuthToken, AuthError> {
        let endpoint = format!(
            "{MICROSOFT_TOKEN_ORIGIN}/{}/oauth2/v2.0/token",
            self.config.tenant
        );
        let response = self
            .http
            .post(endpoint)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(form_body(&[
                ("client_id", self.config.client_id.as_str()),
                ("refresh_token", refresh_token.expose_secret()),
                ("grant_type", "refresh_token"),
                ("scope", XBOX_SCOPE),
            ]))
            .send()
            .await?;
        parse_oauth_response(response).await
    }

    async fn minecraft_session(
        &self,
        microsoft_access_token: &SecretString,
    ) -> Result<MinecraftSession, AuthError> {
        let xbox = self.xbox_user_token(microsoft_access_token).await?;
        let xsts = self.xsts_token(&xbox.token).await?;
        let minecraft = self
            .minecraft_access_token(&xsts.user_hash, &xsts.token)
            .await?;
        self.verify_entitlement(&minecraft.access_token).await?;
        let profile = self.minecraft_profile(&minecraft.access_token).await?;
        Ok(MinecraftSession {
            profile,
            access_token: minecraft.access_token,
            xuid: xsts.xuid.unwrap_or_else(|| "0".to_owned()),
            client_id: self.config.client_id.clone(),
        })
    }

    async fn xbox_user_token(
        &self,
        microsoft_access_token: &SecretString,
    ) -> Result<XboxToken, AuthError> {
        let response = self
            .http
            .post(XBOX_USER_AUTH_URL)
            .header("x-xbl-contract-version", "1")
            .header(reqwest::header::ACCEPT, "application/json")
            .json(&XboxUserRequest {
                relying_party: "http://auth.xboxlive.com",
                token_type: "JWT",
                properties: XboxUserProperties {
                    auth_method: "RPS",
                    site_name: "user.auth.xboxlive.com",
                    rps_ticket: format!("d={}", microsoft_access_token.expose_secret()),
                },
            })
            .send()
            .await?;
        let response = require_success(response, AuthStage::XboxUser).await?;
        let payload: XboxTokenResponse = response.json().await?;
        let claim = payload
            .display_claims
            .xui
            .into_iter()
            .next()
            .ok_or(AuthError::XboxIdentityMissing)?;
        Ok(XboxToken {
            token: SecretString::from(payload.token),
            user_hash: claim.user_hash,
            xuid: claim.xuid,
        })
    }

    async fn xsts_token(&self, user_token: &SecretString) -> Result<XboxToken, AuthError> {
        let response = self
            .http
            .post(XSTS_AUTH_URL)
            .header("x-xbl-contract-version", "1")
            .header(reqwest::header::ACCEPT, "application/json")
            .json(&XstsRequest {
                properties: XstsProperties {
                    sandbox_id: "RETAIL",
                    user_tokens: [user_token.expose_secret()],
                },
                relying_party: "rp://api.minecraftservices.com/",
                token_type: "JWT",
            })
            .send()
            .await?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let xerr = response
                .json::<XstsError>()
                .await
                .ok()
                .and_then(|error| error.xerr);
            return Err(xerr.map_or(
                AuthError::StageRejected {
                    stage: AuthStage::Xsts,
                    status,
                },
                AuthError::XboxPolicy,
            ));
        }
        let payload: XboxTokenResponse = response.json().await?;
        let claim = payload
            .display_claims
            .xui
            .into_iter()
            .next()
            .ok_or(AuthError::XboxIdentityMissing)?;
        Ok(XboxToken {
            token: SecretString::from(payload.token),
            user_hash: claim.user_hash,
            xuid: claim.xuid,
        })
    }

    async fn minecraft_access_token(
        &self,
        user_hash: &str,
        xsts_token: &SecretString,
    ) -> Result<MinecraftAccessToken, AuthError> {
        let response = self
            .http
            .post(MINECRAFT_LOGIN_URL)
            .header(reqwest::header::ACCEPT, "application/json")
            .json(&MinecraftLoginRequest {
                identity_token: format!("XBL3.0 x={user_hash};{}", xsts_token.expose_secret()),
            })
            .send()
            .await?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let invalid_registration = response
                .json::<MinecraftServiceError>()
                .await
                .ok()
                .is_some_and(|error| {
                    error
                        .error_message
                        .to_ascii_lowercase()
                        .contains("invalid app registration")
                });
            return Err(if invalid_registration {
                AuthError::MinecraftApplicationNotAuthorized
            } else {
                AuthError::StageRejected {
                    stage: AuthStage::MinecraftLogin,
                    status,
                }
            });
        }
        let payload: MinecraftTokenResponse = response.json().await?;
        Ok(MinecraftAccessToken {
            access_token: SecretString::from(payload.access_token),
        })
    }

    async fn verify_entitlement(&self, access_token: &SecretString) -> Result<(), AuthError> {
        let response = self
            .http
            .get(MINECRAFT_ENTITLEMENTS_URL)
            .bearer_auth(access_token.expose_secret())
            .send()
            .await?;
        let response = require_success(response, AuthStage::MinecraftEntitlements).await?;
        let payload: MinecraftEntitlements = response.json().await?;
        if payload.items.is_empty() {
            return Err(AuthError::MinecraftNotOwned);
        }
        Ok(())
    }

    async fn minecraft_profile(
        &self,
        access_token: &SecretString,
    ) -> Result<MinecraftProfile, AuthError> {
        let response = self
            .http
            .get(MINECRAFT_PROFILE_URL)
            .bearer_auth(access_token.expose_secret())
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(AuthError::MinecraftProfileMissing);
        }
        let response = require_success(response, AuthStage::MinecraftProfile).await?;
        let payload: MinecraftProfileResponse = response.json().await?;
        let id = Uuid::parse_str(&payload.id).map_err(|_| AuthError::MinecraftProfileInvalid)?;
        if payload.name.is_empty()
            || payload.name.len() > 16
            || !payload
                .name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(AuthError::MinecraftProfileInvalid);
        }
        let skin_url = payload
            .skins
            .into_iter()
            .find_map(|skin| normalize_skin_url(&skin.url));
        let skin_url = match skin_url {
            Some(skin_url) => Some(skin_url),
            None => self.session_profile_skin(id, &payload.name).await,
        };
        Ok(MinecraftProfile {
            id,
            name: payload.name,
            skin_url,
        })
    }

    async fn session_profile_skin(&self, profile_id: Uuid, profile_name: &str) -> Option<String> {
        let endpoint = format!(
            "{MINECRAFT_SESSION_PROFILE_ORIGIN}/session/minecraft/profile/{}",
            profile_id.simple()
        );
        let response = self.http.get(endpoint).send().await.ok()?;
        if !response.status().is_success() {
            return None;
        }
        let bytes = response.bytes().await.ok()?;
        if bytes.len() > MAX_SESSION_PROFILE_BYTES {
            return None;
        }
        skin_url_from_session_profile(profile_id, profile_name, &bytes)
    }
}

pub struct PendingMinecraftLogin {
    client: MinecraftAuthClient,
    listener: TcpListener,
    redirect_uri: String,
    expected_state: String,
    verifier: SecretString,
}

impl std::fmt::Debug for PendingMinecraftLogin {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PendingMinecraftLogin")
            .field("redirect_uri", &self.redirect_uri)
            .field("expected_state", &"<redacted>")
            .field("verifier", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl PendingMinecraftLogin {
    pub async fn complete(self) -> Result<CompletedMinecraftLogin, AuthError> {
        self.complete_with_callback(|| {}).await
    }

    pub async fn complete_with_callback<F>(
        self,
        callback_received: F,
    ) -> Result<CompletedMinecraftLogin, AuthError>
    where
        F: FnOnce(),
    {
        let (mut stream, peer) = tokio::time::timeout(CALLBACK_TIMEOUT, self.listener.accept())
            .await
            .map_err(|_| AuthError::CallbackExpired)??;
        if !peer.ip().is_loopback() {
            return Err(AuthError::CallbackNotLoopback);
        }
        let result = read_callback(&mut stream, &self.expected_state).await;
        let response = if result.is_ok() {
            callback_response(
                "Sign-in received",
                "Return to slate while your Minecraft account is verified.",
            )
        } else {
            callback_response(
                "Sign-in was not completed",
                "Return to slate and try signing in again.",
            )
        };
        let _ = stream.write_all(response.as_bytes()).await;
        let code = SecretString::from(result?);
        callback_received();
        let oauth = self
            .client
            .exchange_authorization_code(&code, &self.redirect_uri, &self.verifier)
            .await?;
        let refresh_token = oauth
            .refresh_token
            .map(SecretString::from)
            .ok_or(AuthError::RefreshTokenMissing)?;
        let session = self.client.minecraft_session(&oauth.access_token).await?;
        Ok(CompletedMinecraftLogin {
            profile: session.profile,
            refresh_token,
        })
    }
}

#[derive(Clone, Debug)]
pub struct AuthFlowStart {
    pub flow_id: Uuid,
    pub authorization_url: Url,
    pub expires_in: Duration,
}

pub struct CompletedMinecraftLogin {
    pub profile: MinecraftProfile,
    pub refresh_token: SecretString,
}

impl std::fmt::Debug for CompletedMinecraftLogin {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompletedMinecraftLogin")
            .field("profile", &self.profile)
            .field("refresh_token", &"<redacted>")
            .finish()
    }
}

pub struct RefreshedMinecraftSession {
    pub session: MinecraftSession,
    pub replacement_refresh_token: Option<SecretString>,
}

impl std::fmt::Debug for RefreshedMinecraftSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RefreshedMinecraftSession")
            .field("session", &self.session)
            .field("replacement_refresh_token", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MinecraftProfile {
    pub id: Uuid,
    pub name: String,
    pub skin_url: Option<String>,
}

pub struct MinecraftSession {
    pub profile: MinecraftProfile,
    access_token: SecretString,
    xuid: String,
    client_id: String,
}

impl MinecraftSession {
    #[must_use]
    pub fn access_token(&self) -> &str {
        self.access_token.expose_secret()
    }

    #[must_use]
    pub fn xuid(&self) -> &str {
        &self.xuid
    }

    #[must_use]
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
}

impl std::fmt::Debug for MinecraftSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MinecraftSession")
            .field("profile", &self.profile)
            .field("access_token", &"<redacted>")
            .field("xuid", &"<redacted>")
            .field("client_id", &"<public-client-id>")
            .finish()
    }
}

#[derive(Clone, Debug, Default)]
pub struct CredentialVault;

impl CredentialVault {
    #[must_use]
    pub fn credential_ref(profile_id: Uuid) -> String {
        format!("{CREDENTIAL_PREFIX}{profile_id}")
    }

    pub fn check_available(&self) -> Result<(), CredentialError> {
        keyring::Entry::store_status()
            .as_ref()
            .map_err(|error| CredentialError::Unavailable(error.to_string()))
            .copied()
    }

    pub fn store(
        &self,
        credential_ref: &str,
        refresh_token: &SecretString,
    ) -> Result<(), CredentialError> {
        credential_entry(credential_ref)?.set_password(refresh_token.expose_secret())?;
        Ok(())
    }

    pub fn load(&self, credential_ref: &str) -> Result<SecretString, CredentialError> {
        Ok(SecretString::from(
            credential_entry(credential_ref)?.get_password()?,
        ))
    }

    pub fn remove(&self, credential_ref: &str) -> Result<(), CredentialError> {
        match credential_entry(credential_ref)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

fn credential_entry(credential_ref: &str) -> Result<keyring::Entry, CredentialError> {
    let profile = credential_ref
        .strip_prefix(CREDENTIAL_PREFIX)
        .ok_or(CredentialError::InvalidReference)?;
    Uuid::parse_str(profile).map_err(|_| CredentialError::InvalidReference)?;
    Ok(keyring::Entry::new(CREDENTIAL_SERVICE, credential_ref)?)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthStage {
    MicrosoftToken,
    XboxUser,
    Xsts,
    MinecraftLogin,
    MinecraftEntitlements,
    MinecraftProfile,
}

async fn parse_oauth_response(response: reqwest::Response) -> Result<OAuthToken, AuthError> {
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let code = response
            .json::<OAuthErrorResponse>()
            .await
            .ok()
            .and_then(|payload| payload.error)
            .filter(|value| {
                value.len() <= 80
                    && value
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            });
        return Err(code.map_or(
            AuthError::StageRejected {
                stage: AuthStage::MicrosoftToken,
                status,
            },
            AuthError::MicrosoftRejected,
        ));
    }
    let payload: OAuthTokenResponse = response.json().await?;
    Ok(OAuthToken {
        access_token: SecretString::from(payload.access_token),
        refresh_token: payload.refresh_token,
    })
}

async fn require_success(
    response: reqwest::Response,
    stage: AuthStage,
) -> Result<reqwest::Response, AuthError> {
    if response.status().is_success() {
        Ok(response)
    } else {
        Err(AuthError::StageRejected {
            stage,
            status: response.status().as_u16(),
        })
    }
}

fn form_body(values: &[(&str, &str)]) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(values.iter().copied())
        .finish()
}

fn random_url_secret() -> String {
    let mut value = String::with_capacity(96);
    for _ in 0..3 {
        value.push_str(&Uuid::new_v4().simple().to_string());
    }
    value
}

fn normalize_skin_url(value: &str) -> Option<String> {
    let mut url = Url::parse(value).ok()?;
    let digest = url.path().strip_prefix("/texture/")?;
    if url.host_str() != Some("textures.minecraft.net")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || digest.len() != 64
        || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        || !matches!(url.scheme(), "http" | "https")
    {
        return None;
    }
    url.set_scheme("https").ok()?;
    Some(url.into())
}

fn skin_url_from_session_profile(
    expected_id: Uuid,
    expected_name: &str,
    response: &[u8],
) -> Option<String> {
    let profile: MinecraftSessionProfileResponse = serde_json::from_slice(response).ok()?;
    if Uuid::parse_str(&profile.id).ok()? != expected_id || profile.name != expected_name {
        return None;
    }
    let encoded = profile
        .properties
        .into_iter()
        .find(|property| property.name == "textures")?
        .value;
    if encoded.len() > MAX_SESSION_PROFILE_BYTES {
        return None;
    }
    let decoded = STANDARD.decode(encoded).ok()?;
    if decoded.len() > MAX_SESSION_PROFILE_BYTES {
        return None;
    }
    let textures: MinecraftTexturesPayload = serde_json::from_slice(&decoded).ok()?;
    if Uuid::parse_str(&textures.profile_id).ok()? != expected_id
        || textures.profile_name != expected_name
    {
        return None;
    }
    textures
        .textures
        .skin
        .and_then(|skin| normalize_skin_url(&skin.url))
}

async fn read_callback(stream: &mut TcpStream, expected_state: &str) -> Result<String, AuthError> {
    let mut request = Vec::with_capacity(2_048);
    let mut chunk = [0_u8; 1_024];
    loop {
        let read = tokio::time::timeout(Duration::from_secs(10), stream.read(&mut chunk))
            .await
            .map_err(|_| AuthError::CallbackReadTimedOut)??;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if request.len() > CALLBACK_MAX_BYTES {
            return Err(AuthError::CallbackTooLarge);
        }
    }
    let request = std::str::from_utf8(&request).map_err(|_| AuthError::CallbackInvalid)?;
    let target = request
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("GET "))
        .and_then(|line| line.split_ascii_whitespace().next())
        .ok_or(AuthError::CallbackInvalid)?;
    parse_callback_target(target, expected_state)
}

fn parse_callback_target(target: &str, expected_state: &str) -> Result<String, AuthError> {
    if target.len() > CALLBACK_MAX_BYTES || !target.starts_with('/') {
        return Err(AuthError::CallbackInvalid);
    }
    let url = Url::parse(&format!("http://localhost{target}"))?;
    if url.path() != "/" {
        return Err(AuthError::CallbackInvalid);
    }
    let query = url
        .query_pairs()
        .collect::<std::collections::BTreeMap<_, _>>();
    if let Some(error) = query.get("error") {
        let code = error.as_ref();
        return Err(if code == "access_denied" {
            AuthError::AuthorizationDenied
        } else {
            AuthError::MicrosoftRejected(code.to_owned())
        });
    }
    let state = query.get("state").ok_or(AuthError::CallbackStateMissing)?;
    if state.as_ref() != expected_state {
        return Err(AuthError::CallbackStateMismatch);
    }
    let code = query
        .get("code")
        .ok_or(AuthError::AuthorizationCodeMissing)?;
    if code.is_empty() || code.len() > 8_192 || code.chars().any(char::is_control) {
        return Err(AuthError::AuthorizationCodeInvalid);
    }
    Ok(code.to_string())
}

fn callback_response(title: &str, message: &str) -> String {
    let body = format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><title>{title}</title><style>body{{font-family:system-ui;background:#101413;color:#f0f4ef;display:grid;place-items:center;min-height:100vh;margin:0}}main{{max-width:34rem;padding:2rem}}h1{{font-size:1.5rem}}p{{color:#aab5af;line-height:1.6}}</style><main><h1>{title}</h1><p>{message}</p></main></html>"
    );
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Security-Policy: default-src 'none'; style-src 'unsafe-inline'\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
}

#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("credential reference is invalid")]
    InvalidReference,
    #[error("the operating-system credential vault is unavailable: {0}")]
    Unavailable(String),
    #[error("the operating-system credential operation failed")]
    Keyring(#[from] keyring::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Microsoft client ID is invalid")]
    InvalidClientId,
    #[error("Microsoft tenant must be consumers")]
    InvalidTenant,
    #[error("the browser callback port {port} is unavailable")]
    CallbackPortUnavailable {
        port: u16,
        #[source]
        source: std::io::Error,
    },
    #[error("the browser callback expired")]
    CallbackExpired,
    #[error("the browser callback was not received from this device")]
    CallbackNotLoopback,
    #[error("the browser callback timed out while being read")]
    CallbackReadTimedOut,
    #[error("the browser callback is too large")]
    CallbackTooLarge,
    #[error("the browser callback is invalid")]
    CallbackInvalid,
    #[error("the browser callback did not contain state")]
    CallbackStateMissing,
    #[error("the browser callback state did not match")]
    CallbackStateMismatch,
    #[error("Microsoft authorization was denied")]
    AuthorizationDenied,
    #[error("the browser callback did not contain an authorization code")]
    AuthorizationCodeMissing,
    #[error("the browser callback authorization code is invalid")]
    AuthorizationCodeInvalid,
    #[error("Microsoft rejected authentication: {0}")]
    MicrosoftRejected(String),
    #[error("Microsoft did not issue a refresh token")]
    RefreshTokenMissing,
    #[error("Xbox identity claims were missing")]
    XboxIdentityMissing,
    #[error("Xbox policy rejected authentication with XErr {0}")]
    XboxPolicy(u64),
    #[error("Minecraft Services has not authorized this application registration")]
    MinecraftApplicationNotAuthorized,
    #[error("this Microsoft account does not own Minecraft: Java Edition")]
    MinecraftNotOwned,
    #[error("this account does not have a Minecraft: Java Edition profile")]
    MinecraftProfileMissing,
    #[error("Minecraft returned an invalid profile")]
    MinecraftProfileInvalid,
    #[error("{stage:?} rejected authentication with HTTP {status}")]
    StageRejected { stage: AuthStage, status: u16 },
    #[error("authentication HTTP request failed")]
    Http(#[from] reqwest::Error),
    #[error("authentication URL is invalid")]
    Url(#[from] url::ParseError),
    #[error("authentication callback I/O failed")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::{
        AuthError, MICROSOFT_CONSUMER_TENANT, MinecraftAuthClient, MinecraftAuthConfig,
        SLATE_MICROSOFT_CLIENT_ID, SLATE_MICROSOFT_REDIRECT_URI, normalize_skin_url,
        parse_callback_target, random_url_secret, skin_url_from_session_profile,
    };
    use base64::Engine as _;
    use uuid::Uuid;

    #[test]
    fn callback_requires_matching_state_and_single_code() {
        assert_eq!(
            parse_callback_target("/?code=abc&state=expected", "expected").ok(),
            Some("abc".to_owned())
        );
        assert!(matches!(
            parse_callback_target("/?code=abc&state=wrong", "expected"),
            Err(AuthError::CallbackStateMismatch)
        ));
    }

    #[test]
    fn verifier_has_high_entropy_shape() {
        let value = random_url_secret();
        assert_eq!(value.len(), 96);
        assert!(value.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }

    #[test]
    fn callback_uri_is_stable_for_consumer_app_registration() {
        assert_eq!(SLATE_MICROSOFT_REDIRECT_URI, "http://localhost:38643");
    }

    #[test]
    fn minecraft_texture_urls_are_pinned_and_upgraded_to_https() {
        let digest = "c8118a94cc3a7fc3b9ce1d9b2f1b57585f7bd890e7ad30fbe3d4b0788a125f7c";
        assert_eq!(
            normalize_skin_url(&format!("http://textures.minecraft.net/texture/{digest}")),
            Some(format!("https://textures.minecraft.net/texture/{digest}"))
        );
        assert!(normalize_skin_url(&format!("https://example.com/texture/{digest}")).is_none());
    }

    #[test]
    fn official_session_profile_supplies_missing_skin() -> Result<(), Box<dyn std::error::Error>> {
        let profile_id = Uuid::parse_str("12345678-1234-4234-8234-123456789abc")?;
        let digest = "c8118a94cc3a7fc3b9ce1d9b2f1b57585f7bd890e7ad30fbe3d4b0788a125f7c";
        let texture_payload = serde_json::json!({
            "profileId": profile_id.simple().to_string(),
            "profileName": "test_player",
            "textures": {
                "SKIN": {
                    "url": format!("http://textures.minecraft.net/texture/{digest}")
                }
            }
        });
        let encoded =
            base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(&texture_payload)?);
        let response = serde_json::to_vec(&serde_json::json!({
            "id": profile_id.simple().to_string(),
            "name": "test_player",
            "properties": [{"name": "textures", "value": encoded}]
        }))?;

        assert_eq!(
            skin_url_from_session_profile(profile_id, "test_player", &response),
            Some(format!("https://textures.minecraft.net/texture/{digest}"))
        );
        assert!(skin_url_from_session_profile(profile_id, "another_name", &response).is_none());
        Ok(())
    }

    #[tokio::test]
    async fn authorization_url_is_public_client_pkce_without_secret()
    -> Result<(), Box<dyn std::error::Error>> {
        let client = MinecraftAuthClient::new(MinecraftAuthConfig::new(
            SLATE_MICROSOFT_CLIENT_ID,
            MICROSOFT_CONSUMER_TENANT,
        )?)?;
        let (authorization_url, _, _) =
            client.authorization_request(SLATE_MICROSOFT_REDIRECT_URI)?;
        let query = authorization_url
            .query_pairs()
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(
            query.get("code_challenge_method").map(|v| v.as_ref()),
            Some("S256")
        );
        assert!(!query.contains_key("client_secret"));
        Ok(())
    }
}
