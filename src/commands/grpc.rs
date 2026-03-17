use crate::cli::GrpcArgs;
use crate::config::Config;
use crate::error::CliResult;
use crate::yellowstone_proto::geyser::{
    geyser_client::GeyserClient, subscribe_update::UpdateOneof, CommitmentLevel, SlotStatus,
    SubscribeRequest, SubscribeRequestFilterAccounts, SubscribeRequestFilterTransactions,
    SubscribeRequestPing,
};
use colored::Colorize;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::signal;
use tokio::sync::watch;
use tokio_stream::StreamExt;
use tonic::transport::Channel;

#[derive(Deserialize)]
struct StreamConfig {
    #[serde(default)]
    transactions: HashMap<String, TransactionFilter>,

    #[serde(default)]
    accounts: HashMap<String, AccountFilter>,

    #[serde(default)]
    slots: HashMap<String, SlotFilter>,

    #[serde(default = "default_commitment")]
    commitment: String,

    #[serde(default = "default_30")]
    timeout_secs: u64,

    #[serde(default = "default_60")]
    tcp_keepalive_secs: u64,

    #[serde(default)]
    reconnect: ReconnectConfig,
}

#[derive(Deserialize)]
struct TransactionFilter {
    #[serde(default)]
    vote: Option<bool>,
    #[serde(default)]
    failed: Option<bool>,
    #[serde(default)]
    account_include: Vec<String>,
    #[serde(default)]
    account_exclude: Vec<String>,
    #[serde(default)]
    account_required: Vec<String>,
}

#[derive(Deserialize)]
struct AccountFilter {
    #[serde(default)]
    account: Vec<String>,
    #[serde(default)]
    owner: Vec<String>,
}

#[derive(Deserialize)]
struct SlotFilter {
    #[serde(default)]
    filter_by_commitment: Option<bool>,
}

#[derive(Deserialize)]
struct ReconnectConfig {
    #[serde(default = "default_100")]
    initial_delay_ms: u64,
    #[serde(default = "default_30000")]
    max_delay_ms: u64,
    #[serde(default = "default_2")]
    multiplier: f64,
    #[serde(default = "default_0")]
    max_retries: u64,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            initial_delay_ms: 100,
            max_delay_ms: 30_000,
            multiplier: 2.0,
            max_retries: 0,
        }
    }
}

fn default_commitment() -> String {
    "confirmed".into()
}
fn default_30() -> u64 {
    30
}
fn default_60() -> u64 {
    60
}
fn default_100() -> u64 {
    100
}
fn default_30000() -> u64 {
    30_000
}
fn default_2() -> f64 {
    2.0
}
fn default_0() -> u64 {
    0
}

struct StreamStats {
    started_at: Instant,
    tx_count: u64,
    acct_count: u64,
    slot_count: u64,
    block_meta_count: u64,
    ping_count: u64,
    reconnect_count: u64,
    latest_slot: u64,
    highest_slot: u64,
}

impl StreamStats {
    fn new() -> Self {
        Self {
            started_at: Instant::now(),
            tx_count: 0,
            acct_count: 0,
            slot_count: 0,
            block_meta_count: 0,
            ping_count: 0,
            reconnect_count: 0,
            latest_slot: 0,
            highest_slot: 0,
        }
    }

    fn update_slot(&mut self, slot: u64) {
        self.latest_slot = slot;
        if slot > self.highest_slot {
            self.highest_slot = slot;
        }
    }

    fn uptime(&self) -> Duration {
        self.started_at.elapsed()
    }

    fn format_uptime(&self) -> String {
        let secs = self.uptime().as_secs();
        if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            format!("{}m {}s", secs / 60, secs % 60)
        } else {
            format!("{}h {}m {}s", secs / 3600, (secs % 3600) / 60, secs % 60)
        }
    }
}

pub async fn run(
    args: GrpcArgs,
    config: &Config,
    grpc_override: Option<&str>,
    fallback_overrides: &[String],
) -> CliResult<()> {
    let raw = std::fs::read_to_string(&args.config).map_err(|e| {
        format!(
            "Failed to read gRPC stream config '{}': {}",
            args.config, e
        )
    })?;

    let expanded = shellexpand::env(&raw)
        .map_err(|e| format!("Failed to expand env vars in stream config: {}", e))?;

    let stream_config: StreamConfig = serde_yml::from_str(&expanded)
        .map_err(|e| format!("Failed to parse stream config: {}", e))?;

    let primary = grpc_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| config.endpoints.grpc.url.clone());

    if primary.is_empty() {
        return Err("No gRPC endpoint configured. Set endpoints.grpc.url in ~/.orbitflare/config.yml".into());
    }

    let mut endpoints = vec![primary];
    if !fallback_overrides.is_empty() {
        endpoints.extend(fallback_overrides.iter().cloned());
    } else {
        endpoints.extend(config.endpoints.grpc.fallback_url.clone());
    }

    if stream_config.transactions.is_empty()
        && stream_config.accounts.is_empty()
        && stream_config.slots.is_empty()
    {
        return Err(
            "No subscriptions defined. Add 'transactions', 'accounts', or 'slots' filters to your stream config."
                .into(),
        );
    }

    let subscribe_request = build_subscribe_request(&stream_config);

    print_header(&endpoints, &stream_config);

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    tokio::spawn(async move {
        let _ = signal::ctrl_c().await;
        let _ = shutdown_tx.send(true);
    });

    let mut stats = StreamStats::new();
    run_with_reconnect(
        &endpoints,
        &stream_config,
        &subscribe_request,
        shutdown_rx,
        &mut stats,
    )
    .await?;

    print_summary(&stats);

    Ok(())
}

fn parse_commitment(s: &str) -> CommitmentLevel {
    match s.to_lowercase().as_str() {
        "processed" => CommitmentLevel::Processed,
        "finalized" => CommitmentLevel::Finalized,
        _ => CommitmentLevel::Confirmed,
    }
}

fn print_header(endpoints: &[String], config: &StreamConfig) {
    println!();
    println!(
        "  {} Yellowstone gRPC (Geyser)",
        "●".green().bold()
    );
    println!("  {} {}", "Endpoint:".dimmed(), endpoints[0].cyan());
    if endpoints.len() > 1 {
        for (i, fb) in endpoints[1..].iter().enumerate() {
            println!(
                "  {} fallback #{}: {}",
                "".dimmed(),
                i + 1,
                fb.cyan()
            );
        }
    }

    println!(
        "  {} {}",
        "Commitment:".dimmed(),
        config.commitment.yellow()
    );

    if !config.transactions.is_empty() {
        let total_accounts: usize = config
            .transactions
            .values()
            .map(|f| f.account_include.len())
            .sum();
        println!(
            "  {} {} filter(s), {} account(s)",
            "Transactions:".dimmed(),
            config.transactions.len(),
            total_accounts
        );
        for (name, filter) in &config.transactions {
            if filter.account_include.is_empty() && filter.account_required.is_empty() {
                println!(
                    "    {} {} (all transactions)",
                    "→".dimmed(),
                    name.bold()
                );
            } else {
                for addr in &filter.account_include {
                    println!(
                        "    {} {} → {}",
                        "→".dimmed(),
                        name.bold(),
                        truncate_addr(addr).dimmed()
                    );
                }
                for addr in &filter.account_required {
                    println!(
                        "    {} {} → {} (required)",
                        "→".dimmed(),
                        name.bold(),
                        truncate_addr(addr).dimmed()
                    );
                }
            }
        }
    }

    if !config.accounts.is_empty() {
        let total: usize = config
            .accounts
            .values()
            .map(|f| f.account.len() + f.owner.len())
            .sum();
        println!(
            "  {} {} filter(s), {} target(s)",
            "Accounts:".dimmed(),
            config.accounts.len(),
            total
        );
        for (name, filter) in &config.accounts {
            for addr in &filter.account {
                println!(
                    "    {} {} → {}",
                    "→".dimmed(),
                    name.bold(),
                    truncate_addr(addr).dimmed()
                );
            }
            for owner in &filter.owner {
                println!(
                    "    {} {} → owner:{}",
                    "→".dimmed(),
                    name.bold(),
                    truncate_addr(owner).dimmed()
                );
            }
        }
    }

    if !config.slots.is_empty() {
        println!(
            "  {} {} filter(s)",
            "Slots:".dimmed(),
            config.slots.len()
        );
    }

    println!(
        "  {} timeout={}s keepalive={}s reconnect={}ms→{}ms (×{})",
        "Settings:".dimmed(),
        config.timeout_secs,
        config.tcp_keepalive_secs,
        config.reconnect.initial_delay_ms,
        config.reconnect.max_delay_ms,
        config.reconnect.multiplier,
    );

    println!();
    println!(
        "  {} {}",
        "▸".yellow(),
        "Press Ctrl+C to stop streaming".dimmed()
    );
    println!();
}

fn build_subscribe_request(config: &StreamConfig) -> SubscribeRequest {
    let transactions: HashMap<String, SubscribeRequestFilterTransactions> = config
        .transactions
        .iter()
        .map(|(name, filter)| {
            let f = SubscribeRequestFilterTransactions {
                vote: filter.vote,
                failed: filter.failed,
                signature: None,
                account_include: filter.account_include.clone(),
                account_exclude: filter.account_exclude.clone(),
                account_required: filter.account_required.clone(),
            };
            (name.clone(), f)
        })
        .collect();

    let accounts: HashMap<String, SubscribeRequestFilterAccounts> = config
        .accounts
        .iter()
        .map(|(name, filter)| {
            let f = SubscribeRequestFilterAccounts {
                account: filter.account.clone(),
                owner: filter.owner.clone(),
                filters: vec![],
                nonempty_txn_signature: None,
            };
            (name.clone(), f)
        })
        .collect();

    let slots = config
        .slots
        .iter()
        .map(|(name, filter)| {
            let f = crate::yellowstone_proto::geyser::SubscribeRequestFilterSlots {
                filter_by_commitment: filter.filter_by_commitment,
                interslot_updates: None,
            };
            (name.clone(), f)
        })
        .collect();

    let commitment = parse_commitment(&config.commitment);

    SubscribeRequest {
        accounts,
        slots,
        transactions,
        transactions_status: HashMap::new(),
        blocks: HashMap::new(),
        blocks_meta: HashMap::new(),
        entry: HashMap::new(),
        commitment: Some(commitment.into()),
        accounts_data_slice: vec![],
        ping: Some(SubscribeRequestPing { id: 1 }),
        from_slot: None,
    }
}

async fn run_with_reconnect(
    endpoints: &[String],
    config: &StreamConfig,
    subscribe_request: &SubscribeRequest,
    mut shutdown_rx: watch::Receiver<bool>,
    stats: &mut StreamStats,
) -> CliResult<()> {
    let mut delay_ms = config.reconnect.initial_delay_ms;
    let mut attempt: u64 = 0;
    let mut endpoint_idx: usize = 0;

    loop {
        if *shutdown_rx.borrow() {
            return Ok(());
        }

        attempt += 1;
        let endpoint = &endpoints[endpoint_idx];

        match connect_and_stream(endpoint, config, subscribe_request, &mut shutdown_rx, stats).await
        {
            Ok(()) => {
                return Ok(());
            }
            Err(e) => {
                if config.reconnect.max_retries > 0 && attempt >= config.reconnect.max_retries {
                    eprintln!(
                        "  {} Max retries ({}) reached. Giving up.",
                        "✗".red().bold(),
                        config.reconnect.max_retries
                    );
                    return Err(e);
                }

                stats.reconnect_count += 1;

                let next_idx = (endpoint_idx + 1) % endpoints.len();
                let switching = next_idx != endpoint_idx || endpoints.len() > 1;

                if switching && endpoints.len() > 1 {
                    endpoint_idx = next_idx;
                    eprintln!(
                        "  {} {} — switching to {} (attempt #{})",
                        "✗".red().bold(),
                        e,
                        endpoints[endpoint_idx].cyan(),
                        attempt,
                    );
                    delay_ms = config.reconnect.initial_delay_ms;
                } else {
                    eprintln!(
                        "  {} {} (attempt #{}, reconnecting in {}ms...)",
                        "✗".red().bold(),
                        e,
                        attempt,
                        delay_ms,
                    );

                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_millis(delay_ms)) => {}
                        _ = shutdown_rx.changed() => {
                            if *shutdown_rx.borrow() {
                                return Ok(());
                            }
                        }
                    }

                    delay_ms =
                        ((delay_ms as f64) * config.reconnect.multiplier) as u64;
                    delay_ms = delay_ms.min(config.reconnect.max_delay_ms);
                }
            }
        }
    }
}

async fn connect_and_stream(
    endpoint: &str,
    config: &StreamConfig,
    subscribe_request: &SubscribeRequest,
    shutdown_rx: &mut watch::Receiver<bool>,
    stats: &mut StreamStats,
) -> CliResult<()> {
    eprintln!(
        "  {} Connecting to {}...",
        "→".dimmed(),
        endpoint.cyan()
    );

    let channel = Channel::from_shared(endpoint.to_string())
        .map_err(|e| format!("Invalid endpoint URL: {}", e))?
        .timeout(Duration::from_secs(config.timeout_secs))
        .tcp_keepalive(Some(Duration::from_secs(config.tcp_keepalive_secs)))
        .connect_timeout(Duration::from_secs(config.timeout_secs))
        .connect()
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;

    let mut client = GeyserClient::new(channel);

    let outbound = tokio_stream::iter(vec![subscribe_request.clone()]);
    let response = client
        .subscribe(outbound)
        .await
        .map_err(|e| format!("Subscribe failed: {}", e))?;
    let mut inbound = response.into_inner();

    eprintln!(
        "  {} {}",
        "✓".green().bold(),
        "Connected — streaming events".green()
    );
    eprintln!();

    let mut ping_interval = tokio::time::interval(Duration::from_secs(30));
    ping_interval.tick().await;

    let mut stats_interval = tokio::time::interval(Duration::from_secs(60));
    stats_interval.tick().await;

    let connected_at = Instant::now();

    loop {
        tokio::select! {
            msg = inbound.next() => {
                match msg {
                    Some(Ok(update)) => {
                        let filters = &update.filters;
                        match update.update_oneof {
                            Some(UpdateOneof::Transaction(tx_update)) => {
                                stats.tx_count += 1;
                                stats.update_slot(tx_update.slot);
                                print_transaction(&tx_update, filters);
                            }
                            Some(UpdateOneof::Account(acct_update)) => {
                                stats.acct_count += 1;
                                stats.update_slot(acct_update.slot);
                                print_account(&acct_update, filters);
                            }
                            Some(UpdateOneof::Slot(slot_update)) => {
                                stats.slot_count += 1;
                                stats.update_slot(slot_update.slot);
                                print_slot(&slot_update, filters);
                            }
                            Some(UpdateOneof::BlockMeta(meta)) => {
                                stats.block_meta_count += 1;
                                stats.update_slot(meta.slot);
                                print_block_meta(&meta, filters);
                            }
                            Some(UpdateOneof::Ping(_)) => {
                                stats.ping_count += 1;
                            }
                            Some(UpdateOneof::Pong(pong)) => {
                                let elapsed = connected_at.elapsed();
                                eprintln!(
                                    "  {} pong id={} (uptime {}s)",
                                    "♥".green().dimmed(),
                                    pong.id,
                                    elapsed.as_secs()
                                );
                            }
                            Some(UpdateOneof::TransactionStatus(_))
                            | Some(UpdateOneof::Block(_))
                            | Some(UpdateOneof::Entry(_))
                            | None => {}
                        }
                    }
                    Some(Err(e)) => {
                        return Err(format!("Stream error: {}", e).into());
                    }
                    None => {
                        return Err("Stream ended unexpectedly".into());
                    }
                }
            }
            _ = ping_interval.tick() => {
            }
            _ = stats_interval.tick() => {
                print_stats(stats);
            }
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    eprintln!();
                    eprintln!(
                        "  {} Shutting down gracefully...",
                        "■".yellow().bold()
                    );
                    return Ok(());
                }
            }
        }
    }
}

fn print_transaction(
    tx: &crate::yellowstone_proto::geyser::SubscribeUpdateTransaction,
    filters: &[String],
) {
    let now = chrono::Local::now().format("%H:%M:%S%.3f");

    if let Some(ref info) = tx.transaction {
        let sig = bs58::encode(&info.signature).into_string();
        let sig_short = if sig.len() > 16 {
            format!("{}...{}", &sig[..8], &sig[sig.len() - 4..])
        } else {
            sig.clone()
        };

        let filter_tag = if filters.is_empty() {
            String::new()
        } else {
            format!(" [{}]", filters.join(",").dimmed())
        };

        let vote_tag = if info.is_vote {
            format!(" {}", "VOTE".yellow().dimmed())
        } else {
            String::new()
        };

        let fee = info.meta.as_ref().map(|m| m.fee).unwrap_or(0);

        if let Some(ref tx_msg) = info.transaction.as_ref().and_then(|t| t.message.as_ref()) {
            let num_accounts = tx_msg.account_keys.len();
            let num_instructions = tx_msg.instructions.len();
            let has_alt = !tx_msg.address_table_lookups.is_empty();

            let alt_tag = if has_alt {
                format!(" {}", "ALT".yellow().dimmed())
            } else {
                String::new()
            };

            println!(
                "  {} {} slot={} sig={} accts={} ix={} fee={}{}{}{} ",
                now.to_string().dimmed(),
                "TX".cyan().bold(),
                format_slot(tx.slot),
                sig_short.cyan(),
                num_accounts,
                num_instructions,
                fee,
                vote_tag,
                alt_tag,
                filter_tag,
            );

            for (i, ix) in tx_msg.instructions.iter().enumerate() {
                let program_idx = ix.program_id_index as usize;
                let program_id = if program_idx < tx_msg.account_keys.len() {
                    bs58::encode(&tx_msg.account_keys[program_idx]).into_string()
                } else {
                    format!("idx:{}", program_idx)
                };

                let program_label = identify_program(&program_id);
                let data_len = ix.data.len();

                println!(
                    "     {} ix[{}] {} ({} bytes data, {} accounts)",
                    "├".dimmed(),
                    i,
                    program_label.yellow(),
                    data_len,
                    ix.accounts.len(),
                );
            }
        } else {
            println!(
                "  {} {} slot={} sig={} fee={}{}{} ",
                now.to_string().dimmed(),
                "TX".cyan().bold(),
                format_slot(tx.slot),
                sig_short.cyan(),
                fee,
                vote_tag,
                filter_tag,
            );
        }
    } else {
        println!(
            "  {} {} slot={} (no tx info)",
            now.to_string().dimmed(),
            "TX".cyan().bold(),
            format_slot(tx.slot),
        );
    }
}

fn print_account(
    acct: &crate::yellowstone_proto::geyser::SubscribeUpdateAccount,
    filters: &[String],
) {
    let now = chrono::Local::now().format("%H:%M:%S%.3f");

    let filter_tag = if filters.is_empty() {
        String::new()
    } else {
        format!(" [{}]", filters.join(",").dimmed())
    };

    if let Some(ref info) = acct.account {
        let pubkey = bs58::encode(&info.pubkey).into_string();
        let owner = bs58::encode(&info.owner).into_string();
        let pubkey_short = truncate_addr(&pubkey);
        let owner_short = truncate_addr(&owner);

        let startup_tag = if acct.is_startup {
            format!(" {}", "STARTUP".yellow().dimmed())
        } else {
            String::new()
        };

        println!(
            "  {} {} slot={} key={} owner={} lamports={} data={}B{}{}{} ",
            now.to_string().dimmed(),
            "ACCT".magenta().bold(),
            format_slot(acct.slot),
            pubkey_short.magenta(),
            owner_short.dimmed(),
            format_lamports(info.lamports),
            info.data.len(),
            startup_tag,
            filter_tag,
            if info.executable { " EXE" } else { "" },
        );
    } else {
        println!(
            "  {} {} slot={} (no account info){}",
            now.to_string().dimmed(),
            "ACCT".magenta().bold(),
            format_slot(acct.slot),
            filter_tag,
        );
    }
}

fn print_slot(
    slot: &crate::yellowstone_proto::geyser::SubscribeUpdateSlot,
    filters: &[String],
) {
    let now = chrono::Local::now().format("%H:%M:%S%.3f");

    let filter_tag = if filters.is_empty() {
        String::new()
    } else {
        format!(" [{}]", filters.join(",").dimmed())
    };

    let status = match slot.status() {
        SlotStatus::SlotProcessed => "processed",
        SlotStatus::SlotConfirmed => "confirmed",
        SlotStatus::SlotFinalized => "finalized",
        SlotStatus::SlotFirstShredReceived => "first_shred",
        SlotStatus::SlotCompleted => "completed",
        SlotStatus::SlotCreatedBank => "created_bank",
        SlotStatus::SlotDead => "dead",
    };

    println!(
        "  {} {} slot={} status={}{}",
        now.to_string().dimmed(),
        "SLOT".blue().bold(),
        format_slot(slot.slot),
        status.green(),
        filter_tag,
    );
}

fn print_block_meta(
    meta: &crate::yellowstone_proto::geyser::SubscribeUpdateBlockMeta,
    filters: &[String],
) {
    let now = chrono::Local::now().format("%H:%M:%S%.3f");

    let filter_tag = if filters.is_empty() {
        String::new()
    } else {
        format!(" [{}]", filters.join(",").dimmed())
    };

    println!(
        "  {} {} slot={} hash={} txs={} parent={}{}",
        now.to_string().dimmed(),
        "BMETA".green().bold(),
        format_slot(meta.slot),
        truncate_addr(&meta.blockhash),
        meta.executed_transaction_count,
        format_slot(meta.parent_slot),
        filter_tag,
    );
}

fn print_stats(stats: &StreamStats) {
    eprintln!(
        "  {} uptime={} tx={} acct={} slots={} blocks={} pings={} reconnects={} slot={} ",
        "STATS".blue().bold(),
        stats.format_uptime(),
        stats.tx_count,
        stats.acct_count,
        stats.slot_count,
        stats.block_meta_count,
        stats.ping_count,
        stats.reconnect_count,
        format_slot(stats.latest_slot),
    );
}

fn print_summary(stats: &StreamStats) {
    eprintln!();
    eprintln!("  {} Stream Summary", "■".bold());
    eprintln!("  {} Uptime:       {}", "│".dimmed(), stats.format_uptime());
    eprintln!("  {} Transactions: {}", "│".dimmed(), stats.tx_count);
    eprintln!("  {} Accounts:     {}", "│".dimmed(), stats.acct_count);
    eprintln!("  {} Slots:        {}", "│".dimmed(), stats.slot_count);
    eprintln!("  {} Block metas:  {}", "│".dimmed(), stats.block_meta_count);
    eprintln!("  {} Pings:        {}", "│".dimmed(), stats.ping_count);
    eprintln!("  {} Reconnects:   {}", "│".dimmed(), stats.reconnect_count);
    eprintln!(
        "  {} Highest slot: {}",
        "│".dimmed(),
        format_slot(stats.highest_slot)
    );
    eprintln!();
}

fn truncate_addr(addr: &str) -> String {
    if addr.len() > 16 {
        format!("{}...{}", &addr[..6], &addr[addr.len() - 4..])
    } else {
        addr.to_string()
    }
}

fn format_slot(slot: u64) -> String {
    let s = slot.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

fn format_lamports(lamports: u64) -> String {
    let sol = lamports as f64 / 1_000_000_000.0;
    if sol >= 1.0 {
        format!("{:.4} SOL", sol)
    } else if sol >= 0.001 {
        format!("{:.6} SOL", sol)
    } else {
        format!("{} lamports", lamports)
    }
}

fn identify_program(program_id: &str) -> String {
    match program_id {
        "11111111111111111111111111111111" => "System Program".to_string(),
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA" => "Token Program".to_string(),
        "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb" => "Token-2022".to_string(),
        "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL" => "ATA Program".to_string(),
        "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4" => "Jupiter v6".to_string(),
        "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8" => "Raydium AMM".to_string(),
        "CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C" => "Raydium CPMM".to_string(),
        "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P" => "Pump.fun".to_string(),
        "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc" => "Orca Whirlpool".to_string(),
        "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr" => "Memo v2".to_string(),
        "ComputeBudget111111111111111111111111111111" => "Compute Budget".to_string(),
        "Vote111111111111111111111111111111111111111" => "Vote Program".to_string(),
        "Stake11111111111111111111111111111111111111" => "Stake Program".to_string(),
        _ => {
            if program_id.len() > 16 {
                format!("{}...{}", &program_id[..8], &program_id[program_id.len() - 4..])
            } else {
                program_id.to_string()
            }
        }
    }
}
