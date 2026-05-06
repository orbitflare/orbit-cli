use crate::error::{CliError, CliResult};
use crate::output;
use colored::Colorize;
use comfy_table::{
    Cell, Color as TColor, ContentArrangement, Table, modifiers::UTF8_ROUND_CORNERS,
    presets::UTF8_FULL,
};
use orbitflare_sdk::proto::geyser::geyser_client::GeyserClient;
use orbitflare_sdk::proto::jetstream::jetstream_client::JetstreamClient;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tonic::transport::Channel;

struct PingResult {
    service: String,
    region: String,
    url: String,
    status: bool,
    latency_ms: f64,
    error: Option<String>,
}

pub async fn run(
    service: Option<String>,
    rpc_urls: &[String],
    jetstream_urls: &[String],
    grpc_urls: &[String],
) -> CliResult<()> {
    let filter = service.as_deref();

    let do_rpc = matches!(filter, None | Some("rpc"));
    let do_jetstream = matches!(filter, None | Some("jetstream") | Some("grpc"));
    let do_grpc = matches!(
        filter,
        None | Some("grpc") | Some("yellowstone") | Some("geyser")
    );

    if let Some(s) = filter {
        if !matches!(s, "rpc" | "jetstream" | "grpc" | "yellowstone" | "geyser") {
            return Err(
                format!("Unknown service: '{}'. Available: rpc, jetstream, grpc", s).into(),
            );
        }
    }

    let rpc_count = if do_rpc { rpc_urls.len() } else { 0 };
    let js_count = if do_jetstream {
        jetstream_urls.len()
    } else {
        0
    };
    let grpc_count = if do_grpc { grpc_urls.len() } else { 0 };
    let total = rpc_count + js_count + grpc_count;

    if total == 0 {
        println!();
        println!(
            "  {} No endpoints configured. Run {} to set up endpoints.",
            "!".yellow().bold(),
            "orbitflare config set <key> <url>".cyan(),
        );
        println!();
        if do_rpc {
            println!(
                "    {} orbitflare config set rpc.url http://your-rpc-endpoint",
                "→".dimmed()
            );
        }
        if do_jetstream {
            println!(
                "    {} orbitflare config set jetstream.url http://your-jetstream-endpoint",
                "→".dimmed()
            );
        }
        if do_grpc {
            println!(
                "    {} orbitflare config set grpc.url http://your-grpc-endpoint:10000",
                "→".dimmed()
            );
        }
        println!();
        return Ok(());
    }

    let quiet = output::is_quiet() || output::is_json();

    if !quiet {
        println!();
        if rpc_count > 0 {
            println!("  {} Found {} RPC endpoint(s)", "→".dimmed(), rpc_count);
        } else if do_rpc && filter.is_none() {
            println!("  {} RPC: not configured", "–".dimmed());
        }
        if js_count > 0 {
            println!(
                "  {} Found {} Jetstream endpoint(s)",
                "→".dimmed(),
                js_count
            );
        } else if do_jetstream && filter.is_none() {
            println!("  {} Jetstream: not configured", "–".dimmed());
        }
        if grpc_count > 0 {
            println!("  {} Found {} gRPC endpoint(s)", "→".dimmed(), grpc_count);
        } else if do_grpc && filter.is_none() {
            println!("  {} gRPC: not configured", "–".dimmed());
        }
        println!(
            "  {} Pinging {} endpoint(s) concurrently...",
            "→".dimmed(),
            total
        );
        println!();
    }

    let (tx, mut rx) = mpsc::channel::<PingResult>(total);

    if do_rpc {
        for url in rpc_urls {
            let tx = tx.clone();
            let url = url.clone();
            tokio::spawn(async move {
                let client = reqwest::Client::builder()
                    .timeout(Duration::from_secs(10))
                    .build()
                    .unwrap();
                let region = extract_region(&url);
                let start = Instant::now();
                let result = check_rpc(&client, &url).await;
                let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
                let _ = tx
                    .send(PingResult {
                        service: "RPC".to_string(),
                        region,
                        url: strip_api_key(&url),
                        status: result.is_ok(),
                        latency_ms,
                        error: result.err().map(|e| strip_api_key(&e.to_string())),
                    })
                    .await;
            });
        }
    }

    if do_jetstream {
        for url in jetstream_urls {
            let tx = tx.clone();
            let url = url.clone();
            tokio::spawn(async move {
                let region = extract_region(&url);
                let start = Instant::now();
                let result = check_jetstream(&url).await;
                let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
                let _ = tx
                    .send(PingResult {
                        service: "Jetstream".to_string(),
                        region,
                        url: url.clone(),
                        status: result.is_ok(),
                        latency_ms,
                        error: result.err().map(|e| strip_api_key(&e.to_string())),
                    })
                    .await;
            });
        }
    }

    if do_grpc {
        for url in grpc_urls {
            let tx = tx.clone();
            let url = url.clone();
            tokio::spawn(async move {
                let region = extract_region(&url);
                let start = Instant::now();
                let result = check_geyser(&url).await;
                let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
                let _ = tx
                    .send(PingResult {
                        service: "gRPC".to_string(),
                        region,
                        url: url.clone(),
                        status: result.is_ok(),
                        latency_ms,
                        error: result.err().map(|e| strip_api_key(&e.to_string())),
                    })
                    .await;
            });
        }
    }

    drop(tx);

    let mut results: Vec<PingResult> = Vec::new();
    let mut done = 0;

    if !quiet {
        println!();
        print_progress(done, total);
    }

    while let Some(r) = rx.recv().await {
        results.push(r);
        done += 1;
        if !quiet {
            print_progress(done, total);
        }
    }

    if !quiet {
        eprint!("\r\x1b[2K");
        println!("  {} All endpoints checked.\n", "✓".green().bold());
    }

    results.sort_by(|a, b| a.service.cmp(&b.service).then(a.region.cmp(&b.region)));

    if output::is_json() {
        let json_results: Vec<serde_json::Value> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "service": r.service,
                    "region": r.region,
                    "url": r.url,
                    "status": if r.status { "up" } else { "down" },
                    "latency_ms": r.latency_ms,
                    "error": r.error,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_results)?);
        return Ok(());
    }

    let services: Vec<String> = {
        let mut seen = Vec::new();
        for r in &results {
            if !seen.contains(&r.service) {
                seen.push(r.service.clone());
            }
        }
        seen
    };

    println!();

    for svc in &services {
        let svc_results: Vec<&PingResult> = results.iter().filter(|r| &r.service == svc).collect();

        println!("  {} {}", "●".cyan().bold(), svc.bold());

        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec!["Region", "Endpoint", "Status", "Latency / Error"]);

        for r in &svc_results {
            let status_cell = if r.status {
                Cell::new("✓ UP").fg(TColor::Green)
            } else {
                Cell::new("✗ DOWN").fg(TColor::Red)
            };

            let latency_cell = if r.status {
                Cell::new(format!("{:.0}ms", r.latency_ms)).fg(TColor::Green)
            } else {
                Cell::new(r.error.as_deref().unwrap_or("unknown")).fg(TColor::Red)
            };

            table.add_row(vec![
                Cell::new(r.region.to_uppercase()),
                Cell::new(&r.url),
                status_cell,
                latency_cell,
            ]);
        }

        println!("{table}");
        println!();
    }

    let up = results.iter().filter(|r| r.status).count();
    let down = results.len() - up;
    println!(
        "  {} {}/{} up, {} down\n",
        if down == 0 {
            "✓".green().bold()
        } else {
            "!".yellow().bold()
        },
        up,
        results.len(),
        down,
    );

    Ok(())
}

async fn check_rpc(client: &reqwest::Client, url: &str) -> Result<(), CliError> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getHealth",
    });
    let resp = match client.post(url).json(&body).send().await {
        Ok(r) => r,
        Err(e) => {
            if e.is_connect() {
                return Err("connection refused".into());
            } else if e.is_timeout() {
                return Err("connection timed out".into());
            } else {
                return Err(format!("{}", e).into());
            }
        }
    };
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
            let msg = json["error"]["message"]
                .as_str()
                .or_else(|| json["message"].as_str())
                .or_else(|| json["error"].as_str())
                .unwrap_or(status.canonical_reason().unwrap_or("unknown error"));
            return Err(format!("HTTP {} — {}", status.as_u16(), msg).into());
        }
        let reason = status.canonical_reason().unwrap_or("unknown error");
        return Err(format!("HTTP {} — {}", status.as_u16(), reason).into());
    }

    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| CliError::from("invalid JSON response"))?;
    if let Some(error) = json.get("error") {
        let msg = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unhealthy");
        return Err(format!("RPC unhealthy: {}", msg).into());
    }
    Ok(())
}

async fn check_jetstream(url: &str) -> Result<(), CliError> {
    let channel = connect_grpc(url).await?;
    let mut client = JetstreamClient::new(channel);
    client
        .ping(orbitflare_sdk::proto::jetstream::PingRequest { count: 1 })
        .await
        .map_err(|e| CliError::from(format!("ping failed: {}", e.message())))?;
    Ok(())
}

async fn check_geyser(url: &str) -> Result<(), CliError> {
    let channel = connect_grpc(url).await?;
    let mut client = GeyserClient::new(channel);
    client
        .ping(orbitflare_sdk::proto::geyser::PingRequest { count: 1 })
        .await
        .map_err(|e| CliError::from(format!("ping failed: {}", e.message())))?;
    Ok(())
}

async fn connect_grpc(url: &str) -> Result<Channel, CliError> {
    Channel::from_shared(url.to_string())
        .map_err(|_| CliError::from(format!("invalid endpoint URL: {}", url)))?
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(10))
        .connect()
        .await
        .map_err(|e| {
            if e.to_string().contains("timed out") {
                CliError::from("connection timed out")
            } else if e.to_string().contains("refused") {
                CliError::from("connection refused")
            } else {
                CliError::from(format!("connection failed: {}", e))
            }
        })
}

fn print_progress(done: usize, total: usize) {
    use std::io::Write;
    let width = 30;
    let filled = if total > 0 { (done * width) / total } else { 0 };
    let empty = width - filled;
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty),);
    eprint!(
        "\r  Pinging {} endpoints  {} {}/{}",
        total, bar, done, total,
    );
    let _ = std::io::stderr().flush();
}

fn extract_region(url: &str) -> String {
    let stripped = url
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    stripped.split('.').next().unwrap_or("?").to_string()
}

fn strip_api_key(s: &str) -> String {
    crate::strip_api_key(s)
}
