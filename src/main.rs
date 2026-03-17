mod api_client;
mod cli;
mod commands;
mod config;
pub mod error;
mod jetstream_proto;
pub mod keyring_store;
pub mod output;
mod rpc_client;
mod yellowstone_proto;

use clap::Parser;
use cli::{Cli, Commands};
use colored::Colorize;
use error::CliResult;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.no_color {
        colored::control::set_override(false);
    }

    output::init(cli.json, cli.raw, cli.quiet);

    let result = dispatch(cli).await;

    if let Err(e) = result {
        eprintln!("{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }
}

fn apply_api_key(url: &str, api_key: Option<&str>) -> String {
    match api_key {
        Some(key) if !key.is_empty() && !url.contains("api_key=") && url.contains("orbitflare.com") => {
            let sep = if url.contains('?') { "&" } else { "?" };
            format!("{}{}api_key={}", url, sep, key)
        }
        _ => url.to_string(),
    }
}

pub fn strip_api_key(s: &str) -> String {
    let re_q = "?api_key=";
    let re_a = "&api_key=";
    let mut result = s.to_string();
    while let Some(start) = result.find(re_q).or_else(|| result.find(re_a)) {
        let value_start = start
            + if result[start..].starts_with(re_q) {
                re_q.len()
            } else {
                re_a.len()
            };
        let end = result[value_start..]
            .find(|c: char| c == '&' || c == ')' || c.is_whitespace())
            .map(|i| i + value_start)
            .unwrap_or(result.len());
        result = format!("{}{}", &result[..start], &result[end..]);
    }
    result
}

struct RpcContext {
    rpc_url: String,
    rpc_fallbacks: Vec<String>,
    commitment: String,
}

fn resolve_profile<'a>(profile_override: Option<&'a str>, config: &'a config::Config) -> Option<&'a str> {
    profile_override.or(config.auth.default_profile.as_deref())
}

fn resolve_rpc_context(
    network_flag: Option<&str>,
    rpc_url_flag: Option<&str>,
    commitment_flag: Option<&str>,
    fallback_urls: &[String],
    config: &config::Config,
    profile: Option<&str>,
) -> RpcContext {
    let license_key = profile
        .and_then(|p| keyring_store::get_secret(p, "license_key"))
        .or_else(|| std::env::var("ORBITFLARE_LICENSE_KEY").ok());

    let network = network_flag.unwrap_or(&config.network.default);

    let base_rpc_url = if let Some(url) = rpc_url_flag {
        url
    } else if network == "devnet" {
        if config.endpoints.rpc.devnet_url.is_empty() { &config.endpoints.rpc.url } else { &config.endpoints.rpc.devnet_url }
    } else {
        &config.endpoints.rpc.url
    };
    let rpc_url = apply_api_key(base_rpc_url, license_key.as_deref());
    let commitment = commitment_flag
        .unwrap_or(&config.preferences.commitment)
        .to_string();

    let rpc_fallbacks: Vec<String> = if !fallback_urls.is_empty() {
        fallback_urls.iter().map(|u| apply_api_key(u, license_key.as_deref())).collect()
    } else if network == "devnet" {
        config.endpoints.rpc.devnet_fallback_url.iter().map(|u| apply_api_key(u, license_key.as_deref())).collect()
    } else {
        config.endpoints.rpc.fallback_url.iter().map(|u| apply_api_key(u, license_key.as_deref())).collect()
    };

    RpcContext { rpc_url, rpc_fallbacks, commitment }
}

async fn dispatch(cli: Cli) -> CliResult<()> {
    let config = config::Config::load()?;

    let grpc_url = cli.grpc_url;
    let fallback_url = cli.fallback_url;
    let network = cli.network;
    let profile_override = cli.profile;

    let needs_rpc = matches!(
        cli.command,
        Commands::Ping { .. } | Commands::Rpc { .. }
    );
    let profile = resolve_profile(profile_override.as_deref(), &config);

    let rpc_ctx = if needs_rpc {
        Some(resolve_rpc_context(
            network.as_deref(),
            cli.rpc_url.as_deref(),
            cli.commitment.as_deref(),
            &fallback_url,
            &config,
            profile,
        ))
    } else {
        None
    };

    match cli.command {
        Commands::Auth { command } => commands::auth::run(command).await,
        Commands::Config { command } => commands::config_cmd::run(command),
        Commands::Ping { service } => {
            let ctx = rpc_ctx.unwrap();
            let mut ping_rpc_urls: Vec<String> = vec![ctx.rpc_url];
            ping_rpc_urls.extend(ctx.rpc_fallbacks);
            let ping_rpc_urls: Vec<String> = ping_rpc_urls.into_iter().filter(|u| !u.is_empty()).collect();

            let mut ping_js_urls = vec![
                grpc_url.as_deref()
                    .unwrap_or(&config.endpoints.jetstream.url)
                    .to_string()
            ];
            ping_js_urls.extend(config.endpoints.jetstream.fallback_url.clone());
            let ping_js_urls: Vec<String> = ping_js_urls.into_iter().filter(|u| !u.is_empty()).collect();

            let mut ping_grpc_urls = vec![config.endpoints.grpc.url.clone()];
            ping_grpc_urls.extend(config.endpoints.grpc.fallback_url.clone());
            let ping_grpc_urls: Vec<String> = ping_grpc_urls.into_iter().filter(|u| !u.is_empty()).collect();

            commands::ping::run(service, &ping_rpc_urls, &ping_js_urls, &ping_grpc_urls).await
        }
        Commands::Docs { topic } => commands::docs::run(topic),
        Commands::Jet(args) => {
            let grpc_fallbacks = if !fallback_url.is_empty() {
                fallback_url
            } else {
                config.endpoints.jetstream.fallback_url.clone()
            };
            commands::jet::run(args, &config, grpc_url.as_deref(), &grpc_fallbacks).await
        }
        Commands::Grpc(args) => {
            let is_devnet = network.as_deref().unwrap_or(&config.network.default) == "devnet";
            let primary = grpc_url.as_deref().map(|s| s.to_string()).unwrap_or_else(|| {
                if is_devnet && !config.endpoints.grpc.devnet_url.is_empty() {
                    config.endpoints.grpc.devnet_url.clone()
                } else {
                    config.endpoints.grpc.url.clone()
                }
            });
            let grpc_fallbacks = if !fallback_url.is_empty() {
                fallback_url
            } else if is_devnet {
                config.endpoints.grpc.devnet_fallback_url.clone()
            } else {
                config.endpoints.grpc.fallback_url.clone()
            };
            commands::grpc::run(args, &config, Some(&primary), &grpc_fallbacks).await
        }
        Commands::Rpc { command } => {
            let ctx = rpc_ctx.unwrap();
            let rpc = rpc_client::RpcClient::new(ctx.rpc_url, ctx.rpc_fallbacks, ctx.commitment);
            commands::rpc::run(command, &rpc).await
        }
        Commands::Template(args) => {
            commands::template::run(args, &config).await
        }
        Commands::Dashboard => {
            commands::dashboard::run().await
        }
    }
}
