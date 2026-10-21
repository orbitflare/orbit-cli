#[derive(Clone, Default)]
pub struct CoinPrice {
    pub symbol: &'static str,
    pub price: f64,
    pub highlight: PriceHighlight,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum PriceHighlight {
    Green,
    Yellow,
    #[default]
    Normal,
}

const TICKER: [(&str, &str, PriceHighlight); 10] = [
    (
        "SOL",
        "So11111111111111111111111111111111111111112",
        PriceHighlight::Green,
    ),
    (
        "JUP",
        "JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN",
        PriceHighlight::Yellow,
    ),
    (
        "BONK",
        "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263",
        PriceHighlight::Normal,
    ),
    (
        "WIF",
        "EKpQGSJtjMFqKZ9KQanSqYXRcF8fBopzLHYxdM65zcjm",
        PriceHighlight::Normal,
    ),
    (
        "JTO",
        "jtojtomepa8beP8AuQc6eXt5FriJwfFMwQx2v2f9mCL",
        PriceHighlight::Normal,
    ),
    (
        "PYTH",
        "HZ1JovNiVvGrGNiiYvEozEVgZ58xaU3RKwX8eACQBCt3",
        PriceHighlight::Normal,
    ),
    (
        "RAY",
        "4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R",
        PriceHighlight::Normal,
    ),
    (
        "ORCA",
        "orcaEKTdK7LKz57vaAYr9QeNsVEPfiu6QeMU1kektZE",
        PriceHighlight::Normal,
    ),
    (
        "PUMP",
        "pumpCmXqMfrsAkQ5r49WcJnRayYRqmXz6ae8H7H9Dfn",
        PriceHighlight::Normal,
    ),
    (
        "USDT",
        "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
        PriceHighlight::Normal,
    ),
];

pub async fn fetch_prices(rpc: &orbitflare_sdk::RpcClient) -> Option<Vec<CoinPrice>> {
    let mints: Vec<&str> = TICKER.iter().map(|(_, mint, _)| *mint).collect();
    let assets = rpc.get_asset_batch(&mints).await.ok()?;
    let prices: Vec<CoinPrice> = TICKER
        .iter()
        .zip(assets)
        .filter_map(|((symbol, _, highlight), asset)| {
            let price = asset?.token_info?.price_info?.price_per_token;
            Some(CoinPrice {
                symbol,
                price,
                highlight: *highlight,
            })
        })
        .collect();
    (!prices.is_empty()).then_some(prices)
}

pub fn format_price(price: f64) -> String {
    if price >= 1.0 {
        format!("${:.2}", price)
    } else if price >= 0.01 {
        format!("${:.4}", price)
    } else {
        format!("${:.8}", price)
    }
}
#[derive(Clone, Default)]
pub struct ServiceInfo {
    pub key_name: String,
    pub license_key: String,
    pub issue_date: String,
    pub renewal_date: String,
}
#[derive(Clone, Default)]
pub struct Invoice {
    pub id: String,
    pub amount: String,
    pub date: String,
    pub status: InvoiceStatus,
}

#[derive(Clone, Copy, Default)]
pub enum InvoiceStatus {
    Paid,
    #[default]
    Pending,
}

impl InvoiceStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Paid => "Paid",
            Self::Pending => "Pending",
        }
    }
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum ServiceStatus {
    #[default]
    Active,
    Expired,
    Suspended,
}

impl ServiceStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Expired => "Expired",
            Self::Suspended => "Suspended",
        }
    }
}

#[derive(Clone, Default)]
pub struct Endpoint {
    pub label: String,
    pub url: String,
}

#[derive(Clone, Default)]
pub struct IpEntry {
    pub ip_address: String,
    pub is_rented: bool,
    pub expires_at: String,
}

#[derive(Clone, Default)]
pub struct Location {
    pub id: u64,
    pub name: String,
    pub slots: u32,
    pub endpoints: Vec<Endpoint>,
    pub ips: Vec<IpEntry>,
}

#[derive(Clone, Default)]
pub struct Service {
    pub id: u64,
    pub name: String,
    pub license_key: String,
    pub status: ServiceStatus,
    pub auth_mode: String,
    pub auto_renew: bool,
    pub issue_date: String,
    pub renewal_date: String,
    pub allowed_domains: Vec<String>,
    pub devnet_allowed_domains: Vec<String>,
    pub locations: Vec<Location>,
}
#[derive(Clone, Default)]
pub struct ConnectedAccountItem {
    pub provider: String,
    pub connected: bool,
}
#[derive(Clone, Default)]
pub struct ApiKeyItem {
    pub id: u64,
    pub name: String,
    pub key_masked: String,
    pub enabled: bool,
    pub created_at: String,
    pub ip_whitelist: Vec<String>,
    pub ip_count: u32,
}
