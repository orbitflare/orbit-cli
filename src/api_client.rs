use crate::error::{CliError, CliResult};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

const BASE_URL: &str = "https://orbitflare.com/api/customer/v2";

#[derive(Deserialize)]
struct ApiResponse<T> {
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub data: Option<T>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
pub struct DeviceCodeData {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Deserialize, Debug, Default)]
pub struct DeviceTokenData {
    pub access_token: String,
    #[serde(default)]
    pub user: Option<UserData>,
}

#[derive(Deserialize, Debug, Default)]
pub struct WalletChallengeData {
    pub challenge: String,
}

#[derive(Deserialize, Debug, Default)]
pub struct WalletAuthData {
    pub access_token: String,
    #[serde(default)]
    pub user: Option<UserData>,
    #[serde(default)]
    pub api_key: Option<ApiKeyData>,
}

#[derive(Deserialize, Debug)]
pub struct ApiKeyData {
    #[serde(default)]
    pub key: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct UserData {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub wallet_address: Option<String>,
    #[serde(default)]
    pub payment_wallet_address: Option<String>,
    #[serde(default)]
    pub balance: Option<f64>,
    #[serde(default)]
    pub created_at: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct ProfileData {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub wallet_address: Option<String>,
    #[serde(default)]
    pub payment_wallet_address: Option<String>,
    #[serde(default)]
    pub balance: Option<f64>,
    #[serde(default)]
    pub connected_accounts: Option<ConnectedAccounts>,
    #[serde(default)]
    pub created_at: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct ConnectedAccounts {
    #[serde(default)]
    pub discord: Option<bool>,
    #[serde(default)]
    pub github: Option<bool>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct ApiKeyEntry {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub key_masked: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub ip_whitelist: Vec<String>,
    #[serde(default)]
    pub ip_count: Option<u32>,
    #[serde(default)]
    pub created_at: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct ApiKeysWrapper {
    #[serde(default)]
    pub api_keys: Vec<ApiKeyEntry>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct LicensesWrapper {
    #[serde(default)]
    pub licenses: Vec<LicenseData>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LicenseData {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub product_name: Option<String>,
    #[serde(default)]
    pub license_key: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub issued_date: Option<String>,
    #[serde(default)]
    pub renewal_date: Option<String>,
    #[serde(default)]
    pub auth_mode: Option<String>,
    #[serde(default)]
    pub allowed_locations: Vec<LocationData>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LocationData {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub urls: Option<LocationUrls>,
    #[serde(default)]
    pub slots: Option<u32>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LocationUrls {
    #[serde(default)]
    pub rpc: Option<String>,
    #[serde(default)]
    pub websocket: Option<String>,
    #[serde(default)]
    pub yellowstone: Option<String>,
    #[serde(default)]
    pub jetstream: Option<String>,
    #[serde(default)]
    pub tx_sender: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct LicenseDetailWrapper {
    #[serde(default)]
    pub license: Option<LicenseDetailData>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct LicenseDetailData {
    #[serde(default)]
    pub license_key: Option<String>,
    #[serde(default)]
    pub issued_date: Option<String>,
    #[serde(default)]
    pub renewal_date: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub auth_mode: Option<String>,
    #[serde(default)]
    pub auto_renew_enabled: Option<bool>,
    #[serde(default)]
    pub product_name: Option<String>,
    #[serde(default)]
    pub allowed_locations: Vec<LocationData>,
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    #[serde(default)]
    pub devnet_allowed_domains: Vec<String>,
    #[serde(default)]
    pub whitelisted_ips: std::collections::HashMap<String, Vec<WhitelistedIp>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct WhitelistedIp {
    #[serde(default)]
    pub ip_address: Option<String>,
    #[serde(default)]
    pub is_rented: Option<bool>,
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct InvoiceData {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub amount: Option<f64>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct PaginatedInvoices {
    #[serde(default)]
    pub data: Vec<InvoiceData>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct BalanceData {
    #[serde(default)]
    pub balance: Option<f64>,
    #[serde(default)]
    pub currency: Option<String>,
}

pub enum DevicePollResult {
    Success(DeviceTokenData),
    Pending,
    SlowDown,
    Expired,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct CreatedApiKey {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub key_masked: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub ip_whitelist: Vec<String>,
    #[serde(default)]
    pub ip_count: Option<u32>,
    #[serde(default)]
    pub created_at: Option<String>,
}

fn extract_error(body: &str) -> String {
    let trimmed = body.trim();
    if let Ok(parsed) = serde_json::from_str::<ApiResponse<serde_json::Value>>(trimmed) {
        if let Some(msg) = parsed.message {
            return msg;
        }
        if let Some(err) = parsed.error {
            return err;
        }
    }
    if trimmed.starts_with('<') || trimmed.starts_with("<!") {
        return "Server Error".to_string();
    }
    trimmed.to_string()
}

type ApiResult<T> = CliResult<T>;

pub struct OrbitApi {
    client: Client,
    api_key: Option<String>,
    token: Option<String>,
}

impl OrbitApi {
    pub fn new() -> Self {
        Self::with_api_key(None)
    }

    pub fn with_api_key(api_key: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        Self { client, api_key, token: None }
    }

    pub fn with_token(token: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        Self { client, api_key: None, token: Some(token.to_string()) }
    }

    fn auth_header(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(ref key) = self.api_key {
            req.header("X-ORBIT-KEY", key)
        } else if let Some(ref tok) = self.token {
            req.header("Authorization", format!("Bearer {}", tok))
        } else {
            req
        }
    }


    pub async fn device_code(&self) -> CliResult<DeviceCodeData> {
        let req = self.client.post(format!("{}/auth/device/code", BASE_URL));
        let resp = self.auth_header(req).send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(format!("POST /auth/device/code — HTTP {}: {}", status.as_u16(), extract_error(&text)).into());
        }
        let parsed: ApiResponse<DeviceCodeData> = serde_json::from_str(&text)?;
        parsed.data.ok_or_else(|| format!("No data in response: {}", parsed.message.unwrap_or_default()).into())
    }

    pub async fn device_token(&self, device_code: &str) -> CliResult<DevicePollResult> {
        let req = self.client
            .post(format!("{}/auth/device/token", BASE_URL))
            .json(&serde_json::json!({ "device_code": device_code }));
        let resp = self.auth_header(req).send().await?;
        let status = resp.status();
        let text = resp.text().await?;

        if status.is_success() {
            let parsed: ApiResponse<DeviceTokenData> = serde_json::from_str(&text)?;
            if let Some(data) = parsed.data {
                return Ok(DevicePollResult::Success(data));
            }
        }

        if let Ok(parsed) = serde_json::from_str::<ApiResponse<serde_json::Value>>(&text) {
            match parsed.error.as_deref() {
                Some("authorization_pending") => return Ok(DevicePollResult::Pending),
                Some("slow_down") => return Ok(DevicePollResult::SlowDown),
                Some("expired_token") => return Ok(DevicePollResult::Expired),
                _ => {}
            }
        }

        if status.as_u16() == 428 { return Ok(DevicePollResult::Pending); }
        if status.as_u16() == 429 { return Ok(DevicePollResult::SlowDown); }
        if status.as_u16() == 410 { return Ok(DevicePollResult::Expired); }

        Err(format!("POST /auth/device/token — HTTP {}: {}", status.as_u16(), extract_error(&text)).into())
    }


    pub async fn wallet_challenge(&self, wallet_address: &str) -> CliResult<WalletChallengeData> {
        let req = self.client
            .post(format!("{}/auth/wallet/challenge", BASE_URL))
            .json(&serde_json::json!({ "wallet_address": wallet_address }));
        let resp = self.auth_header(req).send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(format!("POST /auth/wallet/challenge — HTTP {}: {}", status.as_u16(), extract_error(&text)).into());
        }
        let parsed: ApiResponse<WalletChallengeData> = serde_json::from_str(&text)?;
        parsed.data.ok_or_else(|| format!("No data in response: {}", parsed.message.unwrap_or_default()).into())
    }

    pub async fn wallet_verify(&self, wallet_address: &str, signature: &str) -> CliResult<WalletAuthData> {
        let req = self.client
            .post(format!("{}/auth/wallet/verify", BASE_URL))
            .json(&serde_json::json!({ "wallet_address": wallet_address, "signature": signature }));
        let resp = self.auth_header(req).send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            if let Ok(parsed) = serde_json::from_str::<ApiResponse<serde_json::Value>>(&text)
                && parsed.error.as_deref() == Some("wallet_not_linked") {
                    return Err(CliError::WalletNotLinked);
            }
            return Err(format!("POST /auth/wallet/verify — HTTP {}: {}", status.as_u16(), extract_error(&text)).into());
        }
        let parsed: ApiResponse<WalletAuthData> = serde_json::from_str(&text)?;
        parsed.data.ok_or_else(|| format!("No data in response: {}", parsed.message.unwrap_or_default()).into())
    }

    pub async fn wallet_register(&self, wallet_address: &str, signature: &str, name: &str, email: &str) -> CliResult<WalletAuthData> {
        let req = self.client
            .post(format!("{}/auth/wallet/register", BASE_URL))
            .json(&serde_json::json!({ "wallet_address": wallet_address, "signature": signature, "name": name, "email": email }));
        let resp = self.auth_header(req).send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(format!("POST /auth/wallet/register — HTTP {}: {}", status.as_u16(), extract_error(&text)).into());
        }
        let parsed: ApiResponse<WalletAuthData> = serde_json::from_str(&text)?;
        parsed.data.ok_or_else(|| format!("No data in response: {}", parsed.message.unwrap_or_default()).into())
    }


    pub async fn me_with_api_key(&self, api_key: &str) -> CliResult<UserData> {
        let resp = self.client
            .get(format!("{}/auth/me", BASE_URL))
            .header("X-ORBIT-KEY", api_key)
            .send()
            .await?;
        self.parse_user_response(resp).await
    }

    pub async fn me_with_token(&self, token: &str) -> CliResult<UserData> {
        let req = self.client
            .get(format!("{}/auth/me", BASE_URL))
            .header("Authorization", format!("Bearer {}", token));
        let resp = self.auth_header(req).send().await?;
        self.parse_user_response(resp).await
    }

    async fn parse_user_response(&self, resp: reqwest::Response) -> CliResult<UserData> {
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(format!("GET /auth/me — HTTP {}: {}", status.as_u16(), extract_error(&text)).into());
        }
        let parsed: ApiResponse<UserData> = serde_json::from_str(&text)?;
        parsed.data.ok_or_else(|| format!("No data in response: {}", parsed.message.unwrap_or_default()).into())
    }

    pub async fn profile(&self) -> CliResult<ProfileData> {
        let req = self.client.get(format!("{}/profile", BASE_URL));
        let resp = self.auth_header(req).send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(format!("GET /profile — HTTP {}: {}", status.as_u16(), extract_error(&text)).into());
        }
        let parsed: ApiResponse<ProfileData> = serde_json::from_str(&text)?;
        parsed.data.ok_or_else(|| format!("No data in response: {}", parsed.message.unwrap_or_default()).into())
    }

    pub async fn balance(&self) -> CliResult<BalanceData> {
        let req = self.client.get(format!("{}/balance", BASE_URL));
        let resp = self.auth_header(req).send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(format!("GET /balance — HTTP {}: {}", status.as_u16(), extract_error(&text)).into());
        }
        let parsed: ApiResponse<BalanceData> = serde_json::from_str(&text)?;
        parsed.data.ok_or_else(|| format!("No data in response: {}", parsed.message.unwrap_or_default()).into())
    }


    pub async fn licenses(&self) -> CliResult<Vec<LicenseData>> {
        let req = self.client.get(format!("{}/licenses", BASE_URL));
        let resp = self.auth_header(req).send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(format!("GET /licenses — HTTP {}: {}", status.as_u16(), extract_error(&text)).into());
        }
        let parsed: ApiResponse<LicensesWrapper> = serde_json::from_str(&text)?;
        Ok(parsed.data.map(|w| w.licenses).unwrap_or_default())
    }

    pub async fn license_detail(&self, license_key: &str) -> CliResult<Option<LicenseDetailData>> {
        let req = self.client.get(format!("{}/licenses/{}", BASE_URL, license_key));
        let resp = self.auth_header(req).send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(format!("GET /licenses/{} — HTTP {}: {}", license_key, status.as_u16(), extract_error(&text)).into());
        }
        let parsed: ApiResponse<LicenseDetailWrapper> = serde_json::from_str(&text)?;
        Ok(parsed.data.and_then(|w| w.license))
    }


    pub async fn invoices(&self) -> CliResult<Vec<InvoiceData>> {
        let req = self.client.get(format!("{}/invoices", BASE_URL));
        let resp = self.auth_header(req).send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(format!("GET /invoices — HTTP {}: {}", status.as_u16(), extract_error(&text)).into());
        }
        if let Ok(parsed) = serde_json::from_str::<ApiResponse<PaginatedInvoices>>(&text)
            && let Some(paginated) = parsed.data {
                return Ok(paginated.data);
        }
        let parsed: ApiResponse<Vec<InvoiceData>> = serde_json::from_str(&text)?;
        Ok(parsed.data.unwrap_or_default())
    }


    pub async fn api_keys(&self) -> ApiResult<Vec<ApiKeyEntry>> {
        let req = self.client.get(format!("{}/api-keys", BASE_URL));
        let resp = self.auth_header(req).send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(format!("GET /api-keys — HTTP {}: {}", status.as_u16(), extract_error(&text)).into());
        }
        let parsed: ApiResponse<ApiKeysWrapper> = serde_json::from_str(&text)?;
        Ok(parsed.data.map(|w| w.api_keys).unwrap_or_default())
    }


    pub async fn update_profile(&self, name: Option<&str>, payment_wallet_address: Option<&str>) -> ApiResult<ProfileData> {
        let mut body = serde_json::Map::new();
        if let Some(n) = name {
            body.insert("name".into(), serde_json::json!(n));
        }
        if let Some(w) = payment_wallet_address {
            body.insert("payment_wallet_address".into(), serde_json::json!(w));
        }
        let req = self.client
            .put(format!("{}/profile", BASE_URL))
            .json(&serde_json::Value::Object(body));
        let resp = self.auth_header(req).send().await?;
        self.parse_response::<ProfileData>(resp, "PUT /profile").await
    }


    pub async fn license_regenerate_key(&self, license_key: &str) -> ApiResult<serde_json::Value> {
        self.post_empty(&format!("/licenses/{}/regenerate-key", license_key)).await
    }

    pub async fn license_toggle_auth_mode(&self, license_key: &str, auth_mode: &str) -> ApiResult<serde_json::Value> {
        let req = self.client
            .post(format!("{}/licenses/{}/toggle-auth-mode", BASE_URL, license_key))
            .json(&serde_json::json!({ "auth_mode": auth_mode }));
        let resp = self.auth_header(req).send().await?;
        self.parse_json_response(resp, &format!("POST /licenses/{}/toggle-auth-mode", license_key)).await
    }

    pub async fn license_toggle_auto_renewal(&self, license_key: &str) -> ApiResult<serde_json::Value> {
        self.post_empty(&format!("/licenses/{}/toggle-auto-renewal", license_key)).await
    }

    pub async fn license_add_ip(&self, license_key: &str, ip_address: &str, location_id: u64) -> ApiResult<serde_json::Value> {
        let req = self.client
            .post(format!("{}/licenses/{}/ips", BASE_URL, license_key))
            .json(&serde_json::json!({ "ip_address": ip_address, "location_id": location_id }));
        let resp = self.auth_header(req).send().await?;
        self.parse_json_response(resp, &format!("POST /licenses/{}/ips", license_key)).await
    }

    pub async fn license_remove_ip(&self, license_key: &str, ip_address: &str, location_id: u64) -> ApiResult<serde_json::Value> {
        let req = self.client
            .delete(format!("{}/licenses/{}/ips", BASE_URL, license_key))
            .json(&serde_json::json!({ "ip_address": ip_address, "location_id": location_id }));
        let resp = self.auth_header(req).send().await?;
        self.parse_json_response(resp, &format!("DELETE /licenses/{}/ips", license_key)).await
    }

    pub async fn license_add_domain(&self, license_key: &str, domain: &str) -> ApiResult<serde_json::Value> {
        let req = self.client
            .post(format!("{}/licenses/{}/domains", BASE_URL, license_key))
            .json(&serde_json::json!({ "domain": domain }));
        let resp = self.auth_header(req).send().await?;
        self.parse_json_response(resp, &format!("POST /licenses/{}/domains", license_key)).await
    }

    pub async fn license_remove_domain(&self, license_key: &str, domain: &str) -> ApiResult<serde_json::Value> {
        let req = self.client
            .delete(format!("{}/licenses/{}/domains", BASE_URL, license_key))
            .json(&serde_json::json!({ "domain": domain }));
        let resp = self.auth_header(req).send().await?;
        self.parse_json_response(resp, &format!("DELETE /licenses/{}/domains", license_key)).await
    }

    pub async fn license_add_devnet_domain(&self, license_key: &str, domain: &str) -> ApiResult<serde_json::Value> {
        let req = self.client
            .post(format!("{}/licenses/{}/devnet-domains", BASE_URL, license_key))
            .json(&serde_json::json!({ "domain": domain }));
        let resp = self.auth_header(req).send().await?;
        self.parse_json_response(resp, &format!("POST /licenses/{}/devnet-domains", license_key)).await
    }

    pub async fn license_remove_devnet_domain(&self, license_key: &str, domain: &str) -> ApiResult<serde_json::Value> {
        let req = self.client
            .delete(format!("{}/licenses/{}/devnet-domains", BASE_URL, license_key))
            .json(&serde_json::json!({ "domain": domain }));
        let resp = self.auth_header(req).send().await?;
        self.parse_json_response(resp, &format!("DELETE /licenses/{}/devnet-domains", license_key)).await
    }


    pub async fn create_api_key(&self, name: &str) -> ApiResult<CreatedApiKey> {
        let req = self.client
            .post(format!("{}/api-keys", BASE_URL))
            .json(&serde_json::json!({ "name": name }));
        let resp = self.auth_header(req).send().await?;
        self.parse_response::<CreatedApiKey>(resp, "POST /api-keys").await
    }

    pub async fn update_api_key(&self, id: u64, name: Option<&str>, enabled: Option<bool>) -> ApiResult<ApiKeyEntry> {
        let mut body = serde_json::Map::new();
        if let Some(n) = name {
            body.insert("name".into(), serde_json::json!(n));
        }
        if let Some(e) = enabled {
            body.insert("enabled".into(), serde_json::json!(e));
        }
        let req = self.client
            .put(format!("{}/api-keys/{}", BASE_URL, id))
            .json(&serde_json::Value::Object(body));
        let resp = self.auth_header(req).send().await?;
        self.parse_response::<ApiKeyEntry>(resp, &format!("PUT /api-keys/{}", id)).await
    }

    pub async fn delete_api_key(&self, id: u64) -> ApiResult<String> {
        let req = self.client.delete(format!("{}/api-keys/{}", BASE_URL, id));
        let resp = self.auth_header(req).send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(format!("DELETE /api-keys/{} — HTTP {}: {}", id, status.as_u16(), extract_error(&text)).into());
        }
        let parsed: ApiResponse<serde_json::Value> = serde_json::from_str(&text)?;
        Ok(parsed.message.unwrap_or_else(|| "Deleted".into()))
    }

    pub async fn regenerate_api_key(&self, id: u64) -> ApiResult<CreatedApiKey> {
        let req = self.client.post(format!("{}/api-keys/{}/regenerate", BASE_URL, id));
        let resp = self.auth_header(req).send().await?;
        self.parse_response::<CreatedApiKey>(resp, &format!("POST /api-keys/{}/regenerate", id)).await
    }

    pub async fn api_key_add_ip(&self, id: u64, ip: &str) -> ApiResult<ApiKeyEntry> {
        let req = self.client
            .post(format!("{}/api-keys/{}/ips", BASE_URL, id))
            .json(&serde_json::json!({ "ip": ip }));
        let resp = self.auth_header(req).send().await?;
        self.parse_response::<ApiKeyEntry>(resp, &format!("POST /api-keys/{}/ips", id)).await
    }

    pub async fn api_key_remove_ip(&self, id: u64, ip: &str) -> ApiResult<ApiKeyEntry> {
        let req = self.client
            .delete(format!("{}/api-keys/{}/ips", BASE_URL, id))
            .json(&serde_json::json!({ "ip": ip }));
        let resp = self.auth_header(req).send().await?;
        self.parse_response::<ApiKeyEntry>(resp, &format!("DELETE /api-keys/{}/ips", id)).await
    }


    pub async fn pay_invoice_with_balance(&self, reference: &str) -> ApiResult<serde_json::Value> {
        self.post_empty(&format!("/invoices/{}/pay-with-balance", reference)).await
    }


    async fn post_empty(&self, path: &str) -> ApiResult<serde_json::Value> {
        let req = self.client.post(format!("{}{}", BASE_URL, path));
        let resp = self.auth_header(req).send().await?;
        self.parse_json_response(resp, &format!("POST {}", path)).await
    }

    async fn parse_response<T: serde::de::DeserializeOwned + Default>(&self, resp: reqwest::Response, label: &str) -> ApiResult<T> {
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(format!("{} — HTTP {}: {}", label, status.as_u16(), extract_error(&text)).into());
        }
        let parsed: ApiResponse<T> = serde_json::from_str(&text)?;
        parsed.data.ok_or_else(|| format!("No data in {}: {}", label, text).into())
    }

    async fn parse_json_response(&self, resp: reqwest::Response, label: &str) -> ApiResult<serde_json::Value> {
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(format!("{} — HTTP {}: {}", label, status.as_u16(), extract_error(&text)).into());
        }
        let parsed: ApiResponse<serde_json::Value> = serde_json::from_str(&text)?;
        Ok(parsed.data.unwrap_or(serde_json::Value::Null))
    }
}
