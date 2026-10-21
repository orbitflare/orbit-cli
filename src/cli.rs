use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "orbitflare",
    version,
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

    /// Override network (mainnet|devnet)
    #[arg(long, global = true)]
    pub network: Option<String>,

    /// Region for default endpoints (va, ny, la, slc, ams, fra, lon, jp, sg; Jetstream also dub, sqq, sgp)
    #[arg(long, global = true)]
    pub region: Option<String>,

    /// Override commitment (processed|confirmed|finalized)
    #[arg(long, global = true)]
    pub commitment: Option<String>,

    /// Override RPC endpoint
    #[arg(long, global = true)]
    pub rpc_url: Option<String>,

    /// Override Yellowstone gRPC endpoint
    #[arg(long, global = true)]
    pub grpc_url: Option<String>,

    /// Override Jetstream endpoint
    #[arg(long, global = true)]
    pub jetstream_url: Option<String>,

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
        /// Ping a specific service (rpc, jetstream, grpc)
        #[arg(long)]
        service: Option<String>,
    },
    /// Open documentation in browser
    Docs {
        /// Topic (cli, rpc, ws, jet, grpc, das, history, apex, sdk, api) or a docs path
        topic: Vec<String>,
    },
    /// Real-time transaction streaming (Jetstream)
    Jet(JetArgs),
    /// Real-time gRPC streaming (Yellowstone Geyser)
    Grpc(GrpcArgs),
    /// Send transactions through Apex
    Apex {
        #[command(subcommand)]
        command: ApexCommands,
    },
    /// Execute RPC queries
    Rpc {
        #[command(subcommand)]
        command: RpcCommands,
    },
    /// Query tokens, NFTs and compressed assets (Digital Asset Standard API)
    Das {
        #[command(subcommand)]
        command: DasCommands,
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
    /// Set your Apex API key
    SetApexKey {
        /// The Apex API key
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
    #[arg(long, required_unless_present_any = ["include", "required", "slots"])]
    pub config: Option<String>,
    /// Stream transactions touching any of these accounts (repeatable)
    #[arg(long, conflicts_with = "config")]
    pub include: Vec<String>,
    /// Drop transactions touching these accounts (repeatable)
    #[arg(long, conflicts_with = "config")]
    pub exclude: Vec<String>,
    /// Stream transactions touching all of these accounts (repeatable)
    #[arg(long, conflicts_with = "config")]
    pub required: Vec<String>,
    /// Request enriched transactions (fee payer, compute budget, program IDs, size, resolved lookup tables)
    #[arg(long, conflicts_with = "v1")]
    pub enrichment: bool,
    /// Stream slot lifecycle events (alive, complete, dead) instead of transactions
    #[arg(long, conflicts_with_all = ["config", "include", "required", "exclude", "v1"])]
    pub slots: bool,
    /// Use the deprecated Jetstream v1 protocol
    #[arg(long)]
    pub v1: bool,
    /// Stop after this many seconds
    #[arg(long)]
    pub duration: Option<u64>,
}

#[derive(clap::Args)]
pub struct GrpcArgs {
    /// Path to gRPC stream config YAML file
    #[arg(long)]
    pub config: String,
    /// Stop after this many seconds
    #[arg(long)]
    pub duration: Option<u64>,
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
pub enum ApexCommands {
    /// Measure latency to each Apex region
    Ping {
        /// Ping a single region (global, fra, ams, dub, lon, nyc, slc, sgp, tyo, sqq)
        #[arg(long)]
        region: Option<String>,
    },
    /// List the tip accounts Apex accepts
    TipAccounts {
        /// Apex region to query
        #[arg(long, default_value = "global")]
        region: String,
    },
    /// Send a signed transaction over Apex QUIC
    #[command(
        after_help = "The transaction must carry exactly one top-level SystemProgram transfer of at least 0.001 SOL to an Apex tip account (see `apex tip-accounts`) in its static account keys."
    )]
    Send {
        /// Signed transaction, base64 or base58
        #[arg(required_unless_present = "file", conflicts_with = "file")]
        transaction: Option<String>,
        /// Read the transaction from a file (base64, base58 or raw bytes)
        #[arg(long)]
        file: Option<String>,
        /// Apex region to send through
        #[arg(long, default_value = "global")]
        region: String,
        /// Skip leaders on the Shield blocklist
        #[arg(long)]
        mev_protect: bool,
        /// Do not check the tip rule locally before sending
        #[arg(long)]
        skip_tip_check: bool,
        /// Wait for on-chain confirmation via the configured RPC
        #[arg(long)]
        confirm: bool,
    },
    /// Transfer SOL through Apex, with the tip added and signed by a keypair
    Transfer {
        /// Recipient address
        to: String,
        /// Amount in SOL
        amount: String,
        /// Path to Solana wallet keypair file (default: ~/.config/solana/id.json)
        #[arg(long)]
        wallet: Option<String>,
        /// Apex tip in SOL (minimum 0.001)
        #[arg(long, default_value = "0.001")]
        tip: String,
        /// Compute unit price in micro-lamports
        #[arg(long, default_value_t = 10_000)]
        cu_price: u64,
        /// Apex region to send through
        #[arg(long, default_value = "global")]
        region: String,
        /// Skip leaders on the Shield blocklist
        #[arg(long)]
        mev_protect: bool,
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
    /// Get transaction history for an address (getTransactionsForAddress)
    History {
        /// Wallet or account address
        wallet: String,
        /// Maximum number of transactions (up to 1000)
        #[arg(long, default_value = "10")]
        limit: u32,
        /// Oldest first instead of newest first
        #[arg(long)]
        oldest_first: bool,
        /// Only succeeded or failed transactions (succeeded|failed)
        #[arg(long)]
        status: Option<String>,
        /// Include token account activity (none|balanceChanged|all)
        #[arg(long)]
        token_accounts: Option<String>,
        /// Only transactions at or after this time (unix seconds or YYYY-MM-DD)
        #[arg(long)]
        since: Option<String>,
        /// Only transactions before this time (unix seconds or YYYY-MM-DD)
        #[arg(long)]
        until: Option<String>,
        /// Continue from a previous page's pagination token
        #[arg(long)]
        page_token: Option<String>,
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
        /// Plan slug (e.g., dev, growth, scale; see `plan list`)
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

#[derive(Subcommand)]
pub enum DasCommands {
    /// Get one or more assets by ID (mint or asset address)
    Asset {
        /// Asset IDs
        #[arg(required = true)]
        ids: Vec<String>,
    },
    /// Wallet portfolio: SOL, fungible tokens with prices, and NFTs
    Owner {
        /// Owner address
        address: String,
        /// Only fungible tokens
        #[arg(long, conflicts_with = "nfts")]
        tokens: bool,
        /// Only NFTs and other non-fungible assets
        #[arg(long)]
        nfts: bool,
        /// Page size (up to 1000)
        #[arg(long, default_value = "50")]
        limit: u32,
        /// Page number
        #[arg(long, default_value = "1")]
        page: u32,
    },
    /// Search assets by owner, creator, authority, collection, name and more
    Search {
        /// Owner address
        #[arg(long)]
        owner: Option<String>,
        /// Creator address
        #[arg(long)]
        creator: Option<String>,
        /// Update authority address
        #[arg(long)]
        authority: Option<String>,
        /// Collection address
        #[arg(long)]
        collection: Option<String>,
        /// Asset name (requires --owner)
        #[arg(long, requires = "owner")]
        name: Option<String>,
        /// Token type (fungible|nonFungible|regularNFT|compressedNFT|all)
        #[arg(long)]
        token_type: Option<String>,
        /// Only compressed (true) or uncompressed (false) assets
        #[arg(long)]
        compressed: Option<bool>,
        /// Only burnt (true) or live (false) assets
        #[arg(long)]
        burnt: Option<bool>,
        /// Page size (up to 1000)
        #[arg(long, default_value = "50")]
        limit: u32,
        /// Page number
        #[arg(long, default_value = "1")]
        page: u32,
    },
    /// List the assets in a collection
    Collection {
        /// Collection address
        address: String,
        /// Page size (up to 1000)
        #[arg(long, default_value = "50")]
        limit: u32,
        /// Page number
        #[arg(long, default_value = "1")]
        page: u32,
    },
    /// List the token accounts (holders) of a mint
    Holders {
        /// Token mint address
        mint: String,
        /// Page size (up to 1000)
        #[arg(long, default_value = "50")]
        limit: u32,
        /// Page number
        #[arg(long, default_value = "1")]
        page: u32,
    },
    /// Get the Merkle proof of a compressed asset
    Proof {
        /// Asset ID
        id: String,
    },
    /// List the transactions that changed a compressed asset
    Signatures {
        /// Asset ID
        id: String,
        /// Page size (up to 1000)
        #[arg(long, default_value = "50")]
        limit: u32,
        /// Page number
        #[arg(long, default_value = "1")]
        page: u32,
    },
}
