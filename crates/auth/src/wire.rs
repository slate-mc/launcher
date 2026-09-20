use secrecy::SecretString;
use serde::{Deserialize, Serialize};

pub(super) struct OAuthToken {
    pub(super) access_token: SecretString,
    pub(super) refresh_token: Option<String>,
}

pub(super) struct XboxToken {
    pub(super) token: SecretString,
    pub(super) user_hash: String,
    pub(super) xuid: Option<String>,
}

pub(super) struct MinecraftAccessToken {
    pub(super) access_token: SecretString,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct XboxUserRequest<'a> {
    pub(super) relying_party: &'a str,
    pub(super) token_type: &'a str,
    pub(super) properties: XboxUserProperties,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct XboxUserProperties {
    pub(super) auth_method: &'static str,
    pub(super) site_name: &'static str,
    pub(super) rps_ticket: String,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct XstsRequest<'a> {
    pub(super) properties: XstsProperties<'a>,
    pub(super) relying_party: &'a str,
    pub(super) token_type: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct XstsProperties<'a> {
    pub(super) sandbox_id: &'a str,
    pub(super) user_tokens: [&'a str; 1],
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct XboxTokenResponse {
    pub(super) token: String,
    pub(super) display_claims: XboxDisplayClaims,
}

#[derive(Deserialize)]
pub(super) struct XboxDisplayClaims {
    pub(super) xui: Vec<XboxUserClaim>,
}

#[derive(Deserialize)]
pub(super) struct XboxUserClaim {
    #[serde(rename = "uhs")]
    pub(super) user_hash: String,
    #[serde(rename = "xid")]
    pub(super) xuid: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct XstsError {
    pub(super) xerr: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MinecraftLoginRequest {
    pub(super) identity_token: String,
}

#[derive(Deserialize)]
pub(super) struct MinecraftTokenResponse {
    pub(super) access_token: String,
}

#[derive(Deserialize)]
pub(super) struct MinecraftEntitlements {
    pub(super) items: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
pub(super) struct MinecraftProfileResponse {
    pub(super) id: String,
    pub(super) name: String,
    #[serde(default)]
    pub(super) skins: Vec<MinecraftSkin>,
}

#[derive(Deserialize)]
pub(super) struct MinecraftSkin {
    pub(super) url: String,
}

#[derive(Deserialize)]
pub(super) struct MinecraftSessionProfileResponse {
    pub(super) id: String,
    pub(super) name: String,
    #[serde(default)]
    pub(super) properties: Vec<MinecraftSessionProperty>,
}

#[derive(Deserialize)]
pub(super) struct MinecraftSessionProperty {
    pub(super) name: String,
    pub(super) value: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MinecraftTexturesPayload {
    pub(super) profile_id: String,
    pub(super) profile_name: String,
    pub(super) textures: MinecraftTextures,
}

#[derive(Deserialize)]
pub(super) struct MinecraftTextures {
    #[serde(rename = "SKIN")]
    pub(super) skin: Option<MinecraftTexture>,
}

#[derive(Deserialize)]
pub(super) struct MinecraftTexture {
    pub(super) url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MinecraftServiceError {
    #[serde(default)]
    pub(super) error_message: String,
}

#[derive(Deserialize)]
pub(super) struct OAuthTokenResponse {
    pub(super) access_token: String,
    pub(super) refresh_token: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct OAuthErrorResponse {
    pub(super) error: Option<String>,
}
