use crate::cli::WsCommands;
use crate::error::CliResult;
use crate::output;
use colored::Colorize;
use orbitflare_sdk::{WsClientBuilder, WsSubscription};
use serde_json::Value;
use tokio::signal;

pub async fn run(
    cmd: WsCommands,
    ws_url: &str,
    commitment: &str,
    api_key: Option<&str>,
) -> CliResult<()> {
    print_header(ws_url, &cmd, commitment);

    let mut builder = WsClientBuilder::new().url(ws_url);
    if let Some(key) = api_key {
        builder = builder.api_key(key);
    }
    let client = builder.build().await?;

    let sub = match &cmd {
        WsCommands::Slot => client.slot_subscribe().await?,
        WsCommands::Account { address } => client.account_subscribe(address, commitment).await?,
        WsCommands::Logs { mentions } => {
            let refs: Vec<&str> = mentions.iter().map(|s| s.as_str()).collect();
            client.logs_subscribe(&refs, commitment).await?
        }
        WsCommands::Signature { signature } => {
            client.signature_subscribe(signature, commitment).await?
        }
    };

    eprintln!(
        "  {} {}",
        "✓".green().bold(),
        "Subscribed, streaming events".green()
    );
    eprintln!();

    run_loop(sub, &cmd).await;
    Ok(())
}

async fn run_loop(mut sub: WsSubscription, cmd: &WsCommands) {
    let mut count: u64 = 0;
    loop {
        tokio::select! {
            event = sub.next() => {
                match event {
                    Some(v) => {
                        count += 1;
                        print_event(cmd, &v);
                    }
                    None => {
                        eprintln!("  {} Stream ended", "■".yellow().bold());
                        break;
                    }
                }
            }
            _ = signal::ctrl_c() => {
                eprintln!();
                eprintln!("  {} Shutting down ({} events received)...", "■".yellow().bold(), count);
                sub.unsubscribe().await;
                break;
            }
        }
    }
}

fn print_header(url: &str, cmd: &WsCommands, commitment: &str) {
    println!();
    println!("  {} Solana WebSocket", "●".green().bold());
    println!("  {} {}", "Endpoint:".dimmed(), url.cyan());
    match cmd {
        WsCommands::Slot => {
            println!("  {} slotSubscribe", "Method:".dimmed());
        }
        WsCommands::Account { address } => {
            println!("  {} accountSubscribe", "Method:".dimmed());
            println!("  {} {}", "Address:".dimmed(), address.cyan());
            println!("  {} {}", "Commitment:".dimmed(), commitment.yellow());
        }
        WsCommands::Logs { mentions } => {
            println!("  {} logsSubscribe", "Method:".dimmed());
            if mentions.is_empty() {
                println!("  {} all", "Filter:".dimmed());
            } else {
                println!("  {} mentions={:?}", "Filter:".dimmed(), mentions);
            }
            println!("  {} {}", "Commitment:".dimmed(), commitment.yellow());
        }
        WsCommands::Signature { signature } => {
            println!("  {} signatureSubscribe", "Method:".dimmed());
            println!("  {} {}", "Signature:".dimmed(), signature.cyan());
            println!("  {} {}", "Commitment:".dimmed(), commitment.yellow());
        }
    }
    println!();
    println!(
        "  {} {}",
        "▸".yellow(),
        "Press Ctrl+C to stop streaming".dimmed()
    );
    println!();
}

fn print_event(cmd: &WsCommands, v: &Value) {
    if output::is_json() || output::is_raw() {
        println!(
            "{}",
            serde_json::to_string(v).unwrap_or_else(|_| "{}".to_string())
        );
        return;
    }

    let now = chrono::Local::now().format("%H:%M:%S%.3f");

    match cmd {
        WsCommands::Slot => {
            let slot = v["slot"].as_u64().unwrap_or(0);
            let parent = v["parent"].as_u64().unwrap_or(0);
            let root = v["root"].as_u64().unwrap_or(0);
            println!(
                "  {} {} slot={} parent={} root={}",
                now.to_string().dimmed(),
                "SLOT".blue().bold(),
                format_num(slot),
                format_num(parent),
                format_num(root),
            );
        }
        WsCommands::Account { .. } => {
            let lamports = v["lamports"].as_u64().unwrap_or(0);
            let owner = v["owner"].as_str().unwrap_or("?");
            let exec = v["executable"].as_bool().unwrap_or(false);
            println!(
                "  {} {} owner={} lamports={}{}",
                now.to_string().dimmed(),
                "ACCT".magenta().bold(),
                truncate(owner),
                format_lamports(lamports),
                if exec { " EXE" } else { "" },
            );
        }
        WsCommands::Logs { .. } => {
            let sig = v["signature"].as_str().unwrap_or("?");
            let sig_short = if sig.len() > 16 {
                format!("{}...{}", &sig[..8], &sig[sig.len() - 4..])
            } else {
                sig.to_string()
            };
            let err = &v["err"];
            let status = if err.is_null() {
                "✓".green()
            } else {
                "✗".red()
            };
            let empty: Vec<Value> = Vec::new();
            let logs = v["logs"].as_array().unwrap_or(&empty);
            println!(
                "  {} {} {} sig={} logs={}",
                now.to_string().dimmed(),
                "LOG".cyan().bold(),
                status,
                sig_short.cyan(),
                logs.len(),
            );
            for log in logs.iter().take(5) {
                if let Some(s) = log.as_str() {
                    println!("     {} {}", "├".dimmed(), s.dimmed());
                }
            }
            if logs.len() > 5 {
                println!("     {} ... {} more", "├".dimmed(), logs.len() - 5);
            }
        }
        WsCommands::Signature { .. } => {
            let err = &v["err"];
            let status = if err.is_null() {
                "✓ confirmed".green()
            } else {
                format!("✗ failed: {}", err).red()
            };
            println!(
                "  {} {} {}",
                now.to_string().dimmed(),
                "SIG".cyan().bold(),
                status,
            );
        }
    }
}

fn format_num(n: u64) -> String {
    let s = n.to_string();
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

fn truncate(addr: &str) -> String {
    if addr.len() > 16 {
        format!("{}...{}", &addr[..6], &addr[addr.len() - 4..])
    } else {
        addr.to_string()
    }
}
