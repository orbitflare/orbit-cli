use crate::cli::JetArgs;
use crate::error::CliResult;
use crate::output;
use colored::Colorize;
use orbitflare_sdk::RetryPolicy;
use orbitflare_sdk::jetstream::v2;
use orbitflare_sdk::proto::jetstream::v2::{
    SlotEvent, SlotStatus, TransactionMessage, subscribe_transactions_response::Payload,
};
use orbitflare_sdk::proto::jetstream::{
    SubscribeRequest, SubscribeRequestFilterTransactions, SubscribeRequestPing,
    subscribe_update::UpdateOneof,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Deserialize)]
struct StreamConfig {
    #[serde(default)]
    transactions: BTreeMap<String, TransactionFilter>,

    #[serde(default)]
    accounts: Option<BTreeMap<String, serde::de::IgnoredAny>>,

    #[serde(default = "default_30")]
    timeout_secs: u64,

    #[serde(default = "default_60")]
    tcp_keepalive_secs: u64,

    #[serde(default)]
    reconnect: ReconnectConfig,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            transactions: BTreeMap::new(),
            accounts: None,
            timeout_secs: 30,
            tcp_keepalive_secs: 60,
            reconnect: ReconnectConfig::default(),
        }
    }
}

#[derive(Deserialize, Default)]
struct TransactionFilter {
    #[serde(default)]
    account_include: Vec<String>,
    #[serde(default)]
    account_exclude: Vec<String>,
    #[serde(default)]
    account_required: Vec<String>,
    #[serde(default)]
    include_enrichment: bool,
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
    slot_count: u64,
    latest_slot: u64,
    highest_slot: u64,
}

impl StreamStats {
    fn new() -> Self {
        Self {
            started_at: Instant::now(),
            tx_count: 0,
            slot_count: 0,
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

fn parse_config(raw: &str) -> CliResult<StreamConfig> {
    let expanded = shellexpand::env(raw)
        .map_err(|e| format!("Failed to expand env vars in stream config: {}", e))?;
    let config: StreamConfig = serde_yml::from_str(&expanded)
        .map_err(|e| format!("Failed to parse stream config: {}", e))?;

    if config.accounts.as_ref().is_some_and(|a| !a.is_empty()) {
        return Err("Jetstream streams transactions only and has no account filters. Remove the 'accounts' section, or use `orbitflare grpc` (Yellowstone) for account updates.".into());
    }
    if config.transactions.is_empty() {
        return Err(
            "No subscriptions defined. Add 'transactions' filters to your stream config.".into(),
        );
    }
    Ok(config)
}

fn load_config(args: &JetArgs) -> CliResult<StreamConfig> {
    let mut config = match &args.config {
        Some(path) => {
            let raw = std::fs::read_to_string(path)
                .map_err(|e| format!("Failed to read stream config '{}': {}", path, e))?;
            parse_config(&raw)?
        }
        None => {
            let mut config = StreamConfig::default();
            config.transactions.insert(
                "cli".into(),
                TransactionFilter {
                    account_include: args.include.clone(),
                    account_exclude: args.exclude.clone(),
                    account_required: args.required.clone(),
                    include_enrichment: false,
                },
            );
            config
        }
    };
    if args.enrichment {
        for f in config.transactions.values_mut() {
            f.include_enrichment = true;
        }
    }
    Ok(config)
}

fn retry_policy(config: &ReconnectConfig) -> RetryPolicy {
    RetryPolicy {
        initial_delay: Duration::from_millis(config.initial_delay_ms),
        max_delay: Duration::from_millis(config.max_delay_ms),
        multiplier: config.multiplier,
        max_attempts: config.max_retries as u32,
    }
}

fn v2_client(endpoints: &[String], config: &StreamConfig) -> CliResult<v2::JetstreamClient> {
    let fallback_refs: Vec<&str> = endpoints[1..].iter().map(|s| s.as_str()).collect();
    Ok(v2::JetstreamClientBuilder::new()
        .url(&endpoints[0])
        .fallback_urls(&fallback_refs)
        .retry(retry_policy(&config.reconnect))
        .timeout_secs(config.timeout_secs)
        .keepalive_secs(config.tcp_keepalive_secs)
        .build()?)
}

pub async fn run(args: JetArgs, primary: &str, fallbacks: &[String]) -> CliResult<()> {
    let mut endpoints = vec![primary.to_string()];
    endpoints.extend(fallbacks.iter().cloned());

    if args.slots {
        return stream_slots(&args, &endpoints).await;
    }

    let config = load_config(&args)?;
    if args.v1 && config.transactions.values().any(|f| f.include_enrichment) {
        eprintln!(
            "  {} include_enrichment is ignored on Jetstream v1",
            "!".yellow()
        );
    }

    print_header(&endpoints, &config, args.v1);

    let stats = if args.v1 {
        stream_v1(&args, &endpoints, &config).await?
    } else {
        stream_v2(&args, &endpoints, &config).await?
    };
    print_summary(&stats);
    Ok(())
}

async fn stream_v2(
    args: &JetArgs,
    endpoints: &[String],
    config: &StreamConfig,
) -> CliResult<StreamStats> {
    let client = v2_client(endpoints, config)?;
    let filters = config
        .transactions
        .iter()
        .map(|(name, f)| {
            v2::TransactionFilter::new()
                .account_include(f.account_include.clone())
                .account_exclude(f.account_exclude.clone())
                .account_required(f.account_required.clone())
                .include_enrichment(f.include_enrichment)
                .with_id(name.clone())
        })
        .collect();
    let mut stream = client.subscribe_transactions(filters);
    print_connected();

    let mut shutdown_rx = super::shutdown_signal(args.duration);
    let mut stats = StreamStats::new();
    let mut stats_interval = tokio::time::interval(Duration::from_secs(60));
    stats_interval.tick().await;

    loop {
        tokio::select! {
            resp = stream.next() => {
                match resp {
                    Some(Ok(resp)) => match resp.payload {
                        Some(Payload::Transaction(ft)) => {
                            if let Some(tx) = ft.transaction {
                                stats.tx_count += 1;
                                stats.update_slot(tx.slot);
                                print_transaction_v2(&tx, &ft.filter_ids, resp.sequence);
                            }
                        }
                        Some(Payload::FilterValidation(r)) if !r.accepted => {
                            eprintln!(
                                "  {} Filter '{}' rejected: {}",
                                "✗".red().bold(),
                                r.filter_id,
                                r.rejection_reason
                            );
                        }
                        _ => {}
                    },
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
            _ = stats_interval.tick() => print_stats(&stats),
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    print_shutdown();
                    break;
                }
            }
        }
    }
    Ok(stats)
}

async fn stream_v1(
    args: &JetArgs,
    endpoints: &[String],
    config: &StreamConfig,
) -> CliResult<StreamStats> {
    let fallback_refs: Vec<&str> = endpoints[1..].iter().map(|s| s.as_str()).collect();
    let client = orbitflare_sdk::JetstreamClientBuilder::new()
        .url(&endpoints[0])
        .fallback_urls(&fallback_refs)
        .retry(retry_policy(&config.reconnect))
        .timeout_secs(config.timeout_secs)
        .keepalive_secs(config.tcp_keepalive_secs)
        .build()?;

    let transactions = config
        .transactions
        .iter()
        .map(|(name, f)| {
            (
                name.clone(),
                SubscribeRequestFilterTransactions {
                    account_include: f.account_include.clone(),
                    account_exclude: f.account_exclude.clone(),
                    account_required: f.account_required.clone(),
                },
            )
        })
        .collect();
    let mut stream = client.subscribe(SubscribeRequest {
        transactions,
        accounts: Default::default(),
        ping: Some(SubscribeRequestPing { id: 1 }),
    });
    print_connected();

    let mut shutdown_rx = super::shutdown_signal(args.duration);
    let mut stats = StreamStats::new();
    let mut stats_interval = tokio::time::interval(Duration::from_secs(60));
    stats_interval.tick().await;

    loop {
        tokio::select! {
            update = stream.next() => {
                match update {
                    Some(Ok(update)) => {
                        if let Some(UpdateOneof::Transaction(tx_update)) = update.update_oneof {
                            stats.tx_count += 1;
                            stats.update_slot(tx_update.slot);
                            print_transaction_v1(&tx_update, &update.filters);
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
            _ = stats_interval.tick() => print_stats(&stats),
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    print_shutdown();
                    break;
                }
            }
        }
    }
    Ok(stats)
}

async fn stream_slots(args: &JetArgs, endpoints: &[String]) -> CliResult<()> {
    let config = StreamConfig::default();
    if !output::is_json() {
        println!();
        println!("  {} OrbitFlare Jetstream slots", "●".green().bold());
        println!("  {} {}", "Endpoint:".dimmed(), endpoints[0].cyan());
        println!();
    }
    let client = v2_client(endpoints, &config)?;
    let mut stream = client.subscribe_slots();
    print_connected();

    let mut shutdown_rx = super::shutdown_signal(args.duration);
    let mut stats = StreamStats::new();
    loop {
        tokio::select! {
            ev = stream.next() => {
                match ev {
                    Some(Ok(ev)) => {
                        stats.slot_count += 1;
                        stats.update_slot(ev.slot);
                        print_slot_event(&ev);
                    }
                    Some(Err(e)) => {
                        eprintln!("  {} Stream error: {}", "✗".red().bold(), e);
                        break;
                    }
                    None => break,
                }
            }
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    print_shutdown();
                    break;
                }
            }
        }
    }
    print_summary(&stats);
    Ok(())
}

fn print_connected() {
    if output::is_json() {
        return;
    }
    eprintln!(
        "  {} {}",
        "✓".green().bold(),
        "Connected, streaming events".green()
    );
    eprintln!();
}

fn print_shutdown() {
    eprintln!();
    eprintln!("  {} Shutting down gracefully...", "■".yellow().bold());
}

fn print_header(endpoints: &[String], config: &StreamConfig, v1: bool) {
    if output::is_json() {
        return;
    }
    println!();
    println!("  {} OrbitFlare Jetstream", "●".green().bold());
    println!("  {} {}", "Endpoint:".dimmed(), endpoints[0].cyan());
    for (i, fb) in endpoints[1..].iter().enumerate() {
        println!("  {} fallback #{}: {}", "".dimmed(), i + 1, fb.cyan());
    }
    let enriched = config.transactions.values().any(|f| f.include_enrichment);
    println!(
        "  {} {}",
        "Protocol:".dimmed(),
        if v1 {
            "v1 (deprecated)".yellow().to_string()
        } else if enriched {
            "v2, enrichment on".to_string()
        } else {
            "v2".to_string()
        }
    );
    println!(
        "  {} {} filter(s)",
        "Transactions:".dimmed(),
        config.transactions.len()
    );
    for (name, filter) in &config.transactions {
        let rules: Vec<String> = [
            ("include", &filter.account_include),
            ("required", &filter.account_required),
            ("exclude", &filter.account_exclude),
        ]
        .iter()
        .filter(|(_, list)| !list.is_empty())
        .map(|(label, list)| {
            let addrs: Vec<String> = list.iter().map(|a| truncate_addr(a)).collect();
            format!("{}: {}", label, addrs.join(", "))
        })
        .collect();
        let detail = if rules.is_empty() {
            "(all transactions)".to_string()
        } else {
            rules.join("; ")
        };
        println!("    {} {} {}", "→".dimmed(), name.bold(), detail.dimmed());
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

fn tx_version(tx: &TransactionMessage) -> &'static str {
    if tx.config.is_some() {
        "v1"
    } else if tx.versioned {
        "v0"
    } else {
        "legacy"
    }
}

fn b58(bytes: &[u8]) -> String {
    bs58::encode(bytes).into_string()
}

fn short_sig(sig: &str) -> String {
    if sig.len() > 16 {
        format!("{}...{}", &sig[..8], &sig[sig.len() - 4..])
    } else {
        sig.to_string()
    }
}

fn is_enriched(tx: &TransactionMessage) -> bool {
    !tx.fee_payer.is_empty()
}

fn transaction_json(
    tx: &TransactionMessage,
    filters: &[String],
    sequence: u64,
) -> serde_json::Value {
    let mut v = json!({
        "sequence": sequence,
        "slot": tx.slot,
        "signature": b58(&tx.signature),
        "filters": filters,
        "version": tx_version(tx),
        "is_vote": tx.is_vote,
        "account_keys": tx.account_keys.iter().map(|k| b58(k)).collect::<Vec<_>>(),
        "instructions": tx.instructions.len(),
    });
    if is_enriched(tx) {
        v["enrichment"] = json!({
            "fee_payer": b58(&tx.fee_payer),
            "compute_unit_price": tx.compute_unit_price,
            "compute_limit": tx.compute_limit,
            "priority_fee_lamports": tx.priority_fee_lamports,
            "tx_size": tx.tx_size,
            "program_ids": tx.program_ids.iter().map(|k| b58(k)).collect::<Vec<_>>(),
            "writable_accounts": tx.writable_accounts.iter().map(|k| b58(k)).collect::<Vec<_>>(),
            "loaded_writable_addresses": tx.loaded_writable_addresses.iter().map(|k| b58(k)).collect::<Vec<_>>(),
            "loaded_readonly_addresses": tx.loaded_readonly_addresses.iter().map(|k| b58(k)).collect::<Vec<_>>(),
            "alt_resolution_incomplete": tx.alt_resolution_incomplete,
        });
    }
    v
}

fn print_transaction_v2(tx: &TransactionMessage, filters: &[String], sequence: u64) {
    if output::is_json() {
        println!("{}", transaction_json(tx, filters, sequence));
        return;
    }
    let now = chrono::Local::now().format("%H:%M:%S%.3f");
    let sig = b58(&tx.signature);
    let filter_tag = if filters.is_empty() {
        String::new()
    } else {
        format!(" [{}]", filters.join(",").dimmed())
    };
    println!(
        "  {} {} slot={} sig={} {} accts={} ix={}{}",
        now.to_string().dimmed(),
        "TX".cyan().bold(),
        format_slot(tx.slot),
        short_sig(&sig).cyan(),
        tx_version(tx).dimmed(),
        tx.account_keys.len()
            + tx.loaded_writable_addresses.len()
            + tx.loaded_readonly_addresses.len(),
        tx.instructions.len(),
        filter_tag,
    );
    if is_enriched(tx) {
        let fee = tx
            .priority_fee_lamports
            .map(format_lamports)
            .unwrap_or_else(|| "—".into());
        println!(
            "     {} payer={} cu_price={} cu_limit={} priority_fee={} size={}B",
            "├".dimmed(),
            truncate_addr(&b58(&tx.fee_payer)),
            tx.compute_unit_price,
            tx.compute_limit,
            fee,
            tx.tx_size,
        );
    }
    for (i, ix) in tx.instructions.iter().enumerate() {
        let program_id = tx
            .account_keys
            .get(ix.program_id_index as usize)
            .map(|k| b58(k))
            .unwrap_or_else(|| format!("idx:{}", ix.program_id_index));
        println!(
            "     {} ix[{}] {} ({} bytes data, {} accounts)",
            "├".dimmed(),
            i,
            identify_program(&program_id).yellow(),
            ix.data.len(),
            ix.accounts.len(),
        );
    }
}

fn print_transaction_v1(
    tx: &orbitflare_sdk::proto::jetstream::SubscribeUpdateTransaction,
    filters: &[String],
) {
    let Some(ref info) = tx.transaction else {
        return;
    };
    let sig = b58(&info.signature);
    if output::is_json() {
        println!(
            "{}",
            json!({
                "slot": tx.slot,
                "signature": sig,
                "filters": filters,
                "account_keys": info.account_keys.iter().map(|k| b58(k)).collect::<Vec<_>>(),
                "instructions": info.instructions.len(),
            })
        );
        return;
    }
    let now = chrono::Local::now().format("%H:%M:%S%.3f");
    let filter_tag = if filters.is_empty() {
        String::new()
    } else {
        format!(" [{}]", filters.join(",").dimmed())
    };
    let alt_tag = if info.address_table_lookups.is_empty() {
        String::new()
    } else {
        format!(" {}", "ALT".yellow().dimmed())
    };
    println!(
        "  {} {} slot={} sig={} accts={} ix={}{}{}",
        now.to_string().dimmed(),
        "TX".cyan().bold(),
        format_slot(tx.slot),
        short_sig(&sig).cyan(),
        info.account_keys.len(),
        info.instructions.len(),
        alt_tag,
        filter_tag,
    );
    for (i, ix) in info.instructions.iter().enumerate() {
        let program_id = info
            .account_keys
            .get(ix.program_id_index as usize)
            .map(|k| b58(k))
            .unwrap_or_else(|| format!("idx:{}", ix.program_id_index));
        println!(
            "     {} ix[{}] {} ({} bytes data, {} accounts)",
            "├".dimmed(),
            i,
            identify_program(&program_id).yellow(),
            ix.data.len(),
            ix.accounts.len(),
        );
    }
}

fn slot_status_label(status: i32) -> &'static str {
    match SlotStatus::try_from(status) {
        Ok(SlotStatus::Alive) => "alive",
        Ok(SlotStatus::Complete) => "complete",
        Ok(SlotStatus::Dead) => "dead",
        _ => "unknown",
    }
}

fn print_slot_event(ev: &SlotEvent) {
    let status = slot_status_label(ev.status);
    let leader = if ev.current_leader.is_empty() {
        None
    } else {
        Some(b58(&ev.current_leader))
    };
    if output::is_json() {
        println!(
            "{}",
            json!({
                "sequence": ev.sequence,
                "slot": ev.slot,
                "status": status,
                "parent_slot": ev.parent_slot,
                "leader": leader,
            })
        );
        return;
    }
    let now = chrono::Local::now().format("%H:%M:%S%.3f");
    let status_colored = match status {
        "complete" => status.green().to_string(),
        "dead" => status.red().to_string(),
        _ => status.yellow().to_string(),
    };
    println!(
        "  {} {} slot={} {} parent={} leader={}",
        now.to_string().dimmed(),
        "SLOT".magenta().bold(),
        format_slot(ev.slot),
        status_colored,
        format_slot(ev.parent_slot),
        leader
            .as_deref()
            .map(truncate_addr)
            .unwrap_or_else(|| "—".into()),
    );
}

fn print_stats(stats: &StreamStats) {
    eprintln!(
        "  {} uptime={} tx={} slots={} slot={} ",
        "STATS".blue().bold(),
        stats.format_uptime(),
        stats.tx_count,
        stats.slot_count,
        format_slot(stats.latest_slot),
    );
}

fn print_summary(stats: &StreamStats) {
    eprintln!();
    eprintln!("  {} Stream Summary", "■".bold());
    eprintln!("  {} Uptime:       {}", "│".dimmed(), stats.format_uptime());
    eprintln!("  {} Transactions: {}", "│".dimmed(), stats.tx_count);
    eprintln!("  {} Slot events:  {}", "│".dimmed(), stats.slot_count);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_account_filters() {
        let err = parse_config(
            "transactions:\n  t:\n    account_include: [\"A\"]\naccounts:\n  w:\n    account: [\"B\"]\n",
        )
        .err()
        .unwrap();
        assert!(err.to_string().contains("no account filters"));
    }

    #[test]
    fn parses_transaction_filters_with_enrichment() {
        let config = parse_config(
            "transactions:\n  pump:\n    account_required: [\"6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P\"]\n    include_enrichment: true\naccounts: {}\n",
        )
        .unwrap();
        let f = &config.transactions["pump"];
        assert!(f.include_enrichment);
        assert_eq!(f.account_required.len(), 1);
        assert_eq!(config.timeout_secs, 30);
    }

    #[test]
    fn requires_a_transaction_filter() {
        assert!(parse_config("timeout_secs: 10\n").is_err());
    }

    #[test]
    fn labels_transaction_versions() {
        let mut tx = TransactionMessage::default();
        assert_eq!(tx_version(&tx), "legacy");
        tx.versioned = true;
        assert_eq!(tx_version(&tx), "v0");
        tx.config = Some(Default::default());
        assert_eq!(tx_version(&tx), "v1");
    }

    #[test]
    fn labels_slot_statuses() {
        assert_eq!(slot_status_label(1), "alive");
        assert_eq!(slot_status_label(2), "complete");
        assert_eq!(slot_status_label(3), "dead");
        assert_eq!(slot_status_label(9), "unknown");
    }
}
