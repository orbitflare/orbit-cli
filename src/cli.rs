use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "orbitflare",
    version = "0.3.0",
    about = "OrbitFlare Developer CLI",
    long_about = "The single entry point for everything a developer needs from OrbitFlare:\n\
                  streaming on-chain data, querying the blockchain, scaffolding projects, and monitoring usage."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Output raw JSON (for scripting/piping)
    #[arg(long, global = true)]
    pub json: bool,

    /// Minimal output, no formatting
    #[arg(long, global = true)]
    pub raw: bool,

    /// Override network (mainnet|devnet|testnet)
    #[arg(long, global = true)]
    pub network: Option<String>,

    /// Override commitment (processed|confirmed|finalized)
    #[arg(long, global = true)]
    pub commitment: Option<String>,

    /// Override RPC endpoint
    #[arg(long, global = true)]
    pub rpc_url: Option<String>,

    /// Override gRPC endpoint
    #[arg(long, global = true)]
    pub grpc_url: Option<String>,

    /// Override WebSocket endpoint (default: derived from RPC URL)
    #[arg(long, global = true)]
    pub ws_url: Option<String>,

    /// Fallback URLs (tried in order if primary fails; applies to RPC or gRPC depending on command)
    #[arg(long = "fallback-url", global = true)]
    pub fallback_url: Vec<String>,

    /// Use a specific auth profile
    #[arg(long, global = true)]
    pub profile: Option<String>,

    /// Suppress non-essential output
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    pub no_color: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Authentication management
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },
    /// Configuration management
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    /// Check connection to OrbitFlare services
    Ping {
        /// Ping a specific service (rpc, grpc, jupiter, jito)
        #[arg(long)]
        service: Option<String>,
    },
    /// Open documentation in browser
    Docs {
        /// Documentation topic (e.g., stream, template grpc-indexer)
        topic: Vec<String>,
    },
    /// Real-time gRPC streaming (Jetstream)
    Jet(JetArgs),
    /// Real-time gRPC streaming (Yellowstone Geyser)
    Grpc(GrpcArgs),
    /// Execute RPC queries
    Rpc {
        #[command(subcommand)]
        command: RpcCommands,
    },
    /// Real-time WebSocket subscriptions (Solana pubsub)
    Ws {
        #[command(subcommand)]
        command: WsCommands,
    },
    /// Scaffold projects from templates
    Template(TemplateArgs),
    /// Browse and compare RPC plans
    Plan {
        #[command(subcommand)]
        command: PlanCommands,
    },
    /// Payments, top-ups, and plan purchases
    Pay {
        #[command(subcommand)]
        command: PayCommands,
    },
    /// Real-time TUI dashboard
    Dashboard,
}


#[derive(Subcommand)]
pub enum AuthCommands {
    /// Log in to OrbitFlare
    Login {
        /// API key (from orbitflare.com/dashboard)
        #[arg(long = "x-orbit-key")]
        api_key: Option<String>,
        /// Authenticate with a Solana wallet keypair file
        #[arg(long)]
        wallet: Option<String>,
        /// Profile name to store credentials under
        #[arg(long, default_value = "default")]
        profile: String,
    },
    /// Remove stored credentials
    Logout,
    /// Show current authentication status
    Status,
    /// Switch between auth profiles
    Switch {
        /// Profile name to switch to
        #[arg(long)]
        profile: String,
    },
    /// Set your RPC license key
    SetLicenseKey {
        /// The license key
        key: String,
    },
}


#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Show all configuration values
    Show,
    /// Set a configuration value
    Set {
        /// Config key
        key: String,
        /// Value to set
        value: String,
    },
    /// Remove a value from a list config (e.g., rpc.fallback, jetstream.fallback)
    Remove {
        /// Config key
        key: String,
        /// Value to remove
        value: String,
    },
    /// Reset configuration to defaults
    Reset,
}


#[derive(clap::Args)]
pub struct JetArgs {
    /// Path to stream config YAML file
    #[arg(long)]
    pub config: String,
}


#[derive(clap::Args)]
pub struct GrpcArgs {
    /// Path to gRPC stream config YAML file
    #[arg(long)]
    pub config: String,
}


#[derive(Subcommand)]
pub enum WsCommands {
    /// Subscribe to slot updates
    Slot,
    /// Subscribe to account changes
    Account {
        /// Account address
        address: String,
    },
    /// Subscribe to transaction logs
    Logs {
        /// Limit to logs mentioning these addresses (can be passed multiple times)
        #[arg(long = "mentions")]
        mentions: Vec<String>,
    },
    /// Subscribe to signature confirmation
    Signature {
        /// Transaction signature
        signature: String,
    },
}


#[derive(Subcommand)]
pub enum RpcCommands {
    /// Get account info
    Account {
        /// Account address
        address: String,
    },
    /// Get token accounts for a wallet
    Tokens {
        /// Wallet address
        wallet: String,
    },
    /// Get SOL balance
    Balance {
        /// Account address
        address: String,
    },
    /// Get transaction details
    Tx {
        /// Transaction signature
        signature: String,
    },
    /// Get recent transactions for a wallet
    History {
        /// Wallet address
        wallet: String,
        /// Maximum number of transactions
        #[arg(long, default_value = "10")]
        limit: u32,
    },
    /// Get program accounts
    ProgramAccounts {
        /// Program ID
        program_id: String,
        /// Limit results
        #[arg(long, default_value = "20")]
        limit: u32,
        /// Filter by data size
        #[arg(long)]
        data_size: Option<u64>,
    },
    /// Get current slot
    Slot,
    /// Get current epoch info
    Epoch,
    /// Get recent blockhash
    Blockhash,
    /// Get cluster statistics
    Stats,
    /// Get recent priority fees
    PriorityFees {
        /// Get fees for a specific account (local fee market)
        #[arg(long)]
        account: Option<String>,
    },
    /// Execute a raw JSON-RPC request
    Raw {
        /// Full JSON-RPC body (e.g. '{"jsonrpc":"2.0","id":1,"method":"getSlot","params":[]}')
        body: String,
    },
}


#[derive(clap::Args)]
pub struct TemplateArgs {
    /// List all available templates
    #[arg(long)]
    pub list: bool,

    /// Filter templates by language (rust|ts)
    #[arg(long)]
    pub filter: Option<String>,

    /// View template details
    #[arg(long)]
    pub view: Option<String>,

    /// Install a template
    #[arg(long)]
    pub install: Option<String>,

    /// Target directory for installation
    #[arg(long)]
    pub dir: Option<String>,
}


#[derive(Subcommand)]
pub enum PayCommands {
    /// Check account balance
    CheckBalance,
    /// Top up account balance with USDC (on-chain)
    Topup {
        /// Amount in USDC (min $5)
        amount: f64,
        /// Path to Solana wallet keypair file (default: ~/.config/solana/id.json)
        #[arg(long)]
        wallet: Option<String>,
    },
    /// Top-up and credit transaction history
    History,
    /// Purchase an RPC plan using account balance
    Purchase {
        /// Plan slug (e.g., solana-rpc-lite)
        slug: String,
        /// Billing period (monthly, quarterly, semi-annual, annual)
        period: String,
        /// Discount code
        #[arg(long)]
        coupon: Option<String>,
    },
    /// View order details or order history
    Order {
        /// Order reference (e.g., ORD-ABC123)
        reference: Option<String>,
        /// List order history
        #[arg(long)]
        history: bool,
    },
    /// View invoice details or list invoices
    Invoice {
        /// Invoice reference to view details
        reference: Option<String>,
        /// List all invoices
        #[arg(long)]
        list: bool,
    },
    /// Pay an invoice from account balance (renewal)
    Renew {
        /// Invoice reference
        invoice: String,
    },
}


#[derive(Subcommand)]
pub enum PlanCommands {
    /// List all available RPC plans
    List,
    /// View details of a specific plan
    View {
        /// Plan slug (e.g., dev, growth, scale)
        slug: String,
    },
    /// Compare plans side by side
    Compare {
        /// Plan slugs to compare (omit for all plans)
        plans: Vec<String>,
        /// Compare all available plans
        #[arg(long)]
        all: bool,
    },
}

