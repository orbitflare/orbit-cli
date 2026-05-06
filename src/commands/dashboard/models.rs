use rand::RngExt;
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

pub fn ticker_prices() -> Vec<CoinPrice> {
    let mut rng = rand::rng();
    vec![
        CoinPrice {
            symbol: "SOL",
            price: 84.0 + rng.random_range(-2.0..2.0),
            highlight: PriceHighlight::Green,
        },
        CoinPrice {
            symbol: "BNB",
            price: 617.0 + rng.random_range(-5.0..5.0),
            highlight: PriceHighlight::Yellow,
        },
        CoinPrice {
            symbol: "BTC",
            price: 67880.0 + rng.random_range(-200.0..200.0),
            highlight: PriceHighlight::Normal,
        },
        CoinPrice {
            symbol: "ETH",
            price: 2010.0 + rng.random_range(-30.0..30.0),
            highlight: PriceHighlight::Normal,
        },
        CoinPrice {
            symbol: "XRP",
            price: 1.45 + rng.random_range(-0.1..0.1),
            highlight: PriceHighlight::Normal,
        },
        CoinPrice {
            symbol: "TRX",
            price: 0.28 + rng.random_range(-0.02..0.02),
            highlight: PriceHighlight::Normal,
        },
        CoinPrice {
            symbol: "DOGE",
            price: 0.10 + rng.random_range(-0.01..0.01),
            highlight: PriceHighlight::Normal,
        },
        CoinPrice {
            symbol: "FIGR_HELOC",
            price: 1.02 + rng.random_range(-0.05..0.05),
            highlight: PriceHighlight::Normal,
        },
        CoinPrice {
            symbol: "BCH",
            price: 559.0 + rng.random_range(-10.0..10.0),
            highlight: PriceHighlight::Normal,
        },
        CoinPrice {
            symbol: "WBT",
            price: 51.29 + rng.random_range(-1.0..1.0),
            highlight: PriceHighlight::Normal,
        },
    ]
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
