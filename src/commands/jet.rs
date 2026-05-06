use crate::cli::JetArgs;
use crate::config::Config;
use crate::error::CliResult;
use colored::Colorize;
use orbitflare_sdk::proto::jetstream::{
    SubscribeRequest, SubscribeRequestFilterAccounts, SubscribeRequestFilterTransactions,
    SubscribeRequestPing, subscribe_update::UpdateOneof,
};
use orbitflare_sdk::{JetstreamClientBuilder, RetryPolicy};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::signal;

#[derive(Deserialize)]
struct StreamConfig {
    #[serde(default)]
    transactions: HashMap<String, TransactionFilter>,

    #[serde(default)]
    accounts: HashMap<String, AccountFilter>,

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
    latest_slot: u64,
    highest_slot: u64,
}

impl StreamStats {
    fn new() -> Self {
        Self {
            started_at: Instant::now(),
            tx_count: 0,
            acct_count: 0,
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
    args: JetArgs,
    config: &Config,
    grpc_override: Option<&str>,
    fallback_overrides: &[String],
) -> CliResult<()> {
    let raw = std::fs::read_to_string(&args.config)
        .map_err(|e| format!("Failed to read stream config '{}': {}", args.config, e))?;

    let expanded = shellexpand::env(&raw)
        .map_err(|e| format!("Failed to expand env vars in stream config: {}", e))?;

    let stream_config: StreamConfig = serde_yml::from_str(&expanded)
        .map_err(|e| format!("Failed to parse stream config: {}", e))?;

    let primary = grpc_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| config.endpoints.jetstream.url.clone());

    if primary.is_empty() {
        return Err("No Jetstream endpoint configured. Set endpoints.jetstream.url in ~/.orbitflare/config.yml".into());
    }

    let mut endpoints = vec![primary];
    if !fallback_overrides.is_empty() {
        endpoints.extend(fallback_overrides.iter().cloned());
    } else {
        endpoints.extend(config.endpoints.jetstream.fallback_url.clone());
    }

    if stream_config.transactions.is_empty() && stream_config.accounts.is_empty() {
        return Err(
            "No subscriptions defined. Add 'transactions' or 'accounts' filters to your stream config."
                .into(),
        );
    }

    let subscribe_request = build_subscribe_request(&stream_config);

    print_header(&endpoints, &stream_config);

    let fallback_refs: Vec<&str> = endpoints[1..].iter().map(|s| s.as_str()).collect();
    let retry = RetryPolicy {
        initial_delay: Duration::from_millis(stream_config.reconnect.initial_delay_ms),
        max_delay: Duration::from_millis(stream_config.reconnect.max_delay_ms),
        multiplier: stream_config.reconnect.multiplier,
        max_attempts: stream_config.reconnect.max_retries as u32,
    };

    let client = JetstreamClientBuilder::new()
        .url(&endpoints[0])
        .fallback_urls(&fallback_refs)
        .retry(retry)
        .timeout_secs(stream_config.timeout_secs)
        .keepalive_secs(stream_config.tcp_keepalive_secs)
        .build()?;

    let mut stream = client.subscribe(subscribe_request);

    eprintln!(
        "  {} {}",
        "✓".green().bold(),
        "Connected, streaming events".green()
    );
    eprintln!();

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
    tokio::spawn(async move {
        let _ = signal::ctrl_c().await;
        let _ = shutdown_tx.send(true);
    });

    let mut stats = StreamStats::new();
    let mut stats_interval = tokio::time::interval(Duration::from_secs(60));
    stats_interval.tick().await;

    loop {
        tokio::select! {
            update = stream.next() => {
                match update {
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
                            _ => {}
                        }
                    }
                    Some(Err(e)) => {
                        eprintln!("  {} Stream error: {}", "✗".red().bold(), e);
                        break;
                    }
                    None => {
                        eprintln!("  {} Stream ended", "■".yellow().bold());
                        break;
                    }
                }
            }
            _ = stats_interval.tick() => {
                print_stats(&stats);
            }
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    eprintln!();
                    eprintln!("  {} Shutting down gracefully...", "■".yellow().bold());
                    break;
                }
            }
        }
    }

    print_summary(&stats);
    Ok(())
}

fn print_header(endpoints: &[String], config: &StreamConfig) {
    println!();
    println!("  {} OrbitFlare Jetstream", "●".green().bold());
    println!("  {} {}", "Endpoint:".dimmed(), endpoints[0].cyan());
    if endpoints.len() > 1 {
        for (i, fb) in endpoints[1..].iter().enumerate() {
            println!("  {} fallback #{}: {}", "".dimmed(), i + 1, fb.cyan());
        }
    }

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
            if filter.account_include.is_empty() {
                println!("    {} {} (all transactions)", "→".dimmed(), name.bold());
            } else {
                for addr in &filter.account_include {
                    println!(
                        "    {} {} → {}",
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
            };
            (name.clone(), f)
        })
        .collect();

    SubscribeRequest {
        transactions,
        accounts,
        ping: Some(SubscribeRequestPing { id: 1 }),
    }
}

fn print_transaction(
    tx: &orbitflare_sdk::proto::jetstream::SubscribeUpdateTransaction,
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

        let num_accounts = info.account_keys.len();
        let num_instructions = info.instructions.len();
        let has_alt = !info.address_table_lookups.is_empty();

        let filter_tag = if filters.is_empty() {
            String::new()
        } else {
            format!(" [{}]", filters.join(",").dimmed())
        };

        let alt_tag = if has_alt {
            format!(" {}", "ALT".yellow().dimmed())
        } else {
            String::new()
        };

        println!(
            "  {} {} slot={} sig={} accts={} ix={}{}{} ",
            now.to_string().dimmed(),
            "TX".cyan().bold(),
            format_slot(tx.slot),
            sig_short.cyan(),
            num_accounts,
            num_instructions,
            alt_tag,
            filter_tag,
        );

        for (i, ix) in info.instructions.iter().enumerate() {
            let program_idx = ix.program_id_index as usize;
            let program_id = if program_idx < info.account_keys.len() {
                bs58::encode(&info.account_keys[program_idx]).into_string()
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
            "  {} {} slot={} (no tx info)",
            now.to_string().dimmed(),
            "TX".cyan().bold(),
            format_slot(tx.slot),
        );
    }
}

fn print_account(
    acct: &orbitflare_sdk::proto::jetstream::SubscribeUpdateAccount,
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

fn print_stats(stats: &StreamStats) {
    eprintln!(
        "  {} uptime={} tx={} acct={} slot={} ",
        "STATS".blue().bold(),
        stats.format_uptime(),
        stats.tx_count,
        stats.acct_count,
        format_slot(stats.latest_slot),
    );
}

fn print_summary(stats: &StreamStats) {
    eprintln!();
    eprintln!("  {} Stream Summary", "■".bold());
    eprintln!("  {} Uptime:       {}", "│".dimmed(), stats.format_uptime());
    eprintln!("  {} Transactions: {}", "│".dimmed(), stats.tx_count);
    eprintln!("  {} Accounts:     {}", "│".dimmed(), stats.acct_count);
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
                format!(
                    "{}...{}",
                    &program_id[..8],
                    &program_id[program_id.len() - 4..]
                )
            } else {
                program_id.to_string()
            }
        }
    }
}
