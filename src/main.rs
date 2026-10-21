mod api_client;
mod cli;
mod commands;
mod config;
pub mod error;
pub mod keyring_store;
pub mod output;
mod regions;

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
        Some(key)
            if !key.is_empty() && !url.contains("api_key=") && url.contains("orbitflare.com") =>
        {
            let (base, sep) = if url.contains('?') {
                (url.to_string(), "&")
            } else if url_has_path(url) {
                (url.to_string(), "?")
            } else {
                (format!("{}/", url), "?")
            };
            format!("{}{}api_key={}", base, sep, key)
        }
        _ => url.to_string(),
    }
}

fn url_has_path(url: &str) -> bool {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    after_scheme.contains('/')
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
    api_key: Option<String>,
}

impl RpcContext {
    fn rpc_url_with_key(&self) -> String {
        apply_api_key(&self.rpc_url, self.api_key.as_deref())
    }

    fn fallback_urls_with_key(&self) -> Vec<String> {
        self.rpc_fallbacks
            .iter()
            .map(|u| apply_api_key(u, self.api_key.as_deref()))
            .collect()
    }
}

fn resolve_profile<'a>(
    profile_override: Option<&'a str>,
    config: &'a config::Config,
) -> Option<&'a str> {
    profile_override.or(config.auth.default_profile.as_deref())
}

fn resolve_region(region_flag: Option<&str>, config: &config::Config) -> CliResult<String> {
    let region = region_flag
        .or(Some(config.network.region.as_str()).filter(|r| !r.is_empty()))
        .unwrap_or(regions::DEFAULT_REGION);
    Ok(regions::validate(region)?)
}

fn resolve_rpc_context(
    network: &str,
    region: &str,
    rpc_url_flag: Option<&str>,
    commitment_flag: Option<&str>,
    fallback_urls: &[String],
    config: &config::Config,
    profile: Option<&str>,
) -> CliResult<RpcContext> {
    let license_key = profile
        .and_then(|p| keyring_store::get_secret(p, "license_key"))
        .or_else(|| std::env::var("ORBITFLARE_LICENSE_KEY").ok());

    let rpc_url = if let Some(url) = rpc_url_flag {
        url.to_string()
    } else if network == "devnet" {
        if config.endpoints.rpc.devnet_url.is_empty() {
            regions::DEVNET_RPC_URL.to_string()
        } else {
            config.endpoints.rpc.devnet_url.clone()
        }
    } else if !config.endpoints.rpc.url.is_empty() {
        config.endpoints.rpc.url.clone()
    } else {
        regions::rpc_url(region)?
    };
    let commitment = commitment_flag
        .unwrap_or(&config.preferences.commitment)
        .to_string();

    let rpc_fallbacks: Vec<String> = if !fallback_urls.is_empty() {
        fallback_urls.to_vec()
    } else if network == "devnet" {
        config.endpoints.rpc.devnet_fallback_url.clone()
    } else {
        config.endpoints.rpc.fallback_url.clone()
    };

    let api_key = license_key.filter(|_| rpc_url.contains("orbitflare.com"));

    Ok(RpcContext {
        rpc_url,
        rpc_fallbacks,
        commitment,
        api_key,
    })
}

fn resolve_grpc_url(
    network: &str,
    region: &str,
    grpc_url_flag: Option<&str>,
    config: &config::Config,
) -> CliResult<String> {
    if let Some(url) = grpc_url_flag {
        return Ok(url.to_string());
    }
    if network == "devnet" {
        return Ok(if config.endpoints.grpc.devnet_url.is_empty() {
            regions::DEVNET_GRPC_URL.to_string()
        } else {
            config.endpoints.grpc.devnet_url.clone()
        });
    }
    if !config.endpoints.grpc.url.is_empty() {
        return Ok(config.endpoints.grpc.url.clone());
    }
    Ok(regions::grpc_url(region)?)
}

fn resolve_jetstream_url(
    network: &str,
    region: &str,
    url_flag: Option<&str>,
    config: &config::Config,
) -> CliResult<String> {
    if let Some(url) = url_flag {
        return Ok(url.to_string());
    }
    if network == "devnet" {
        return Err("Jetstream streams mainnet only and has no devnet endpoint.".into());
    }
    if !config.endpoints.jetstream.url.is_empty() {
        return Ok(config.endpoints.jetstream.url.clone());
    }
    Ok(regions::jetstream_url(region))
}

async fn dispatch(cli: Cli) -> CliResult<()> {
    let config = config::Config::load()?;

    let grpc_url = cli.grpc_url;
    let jetstream_url = cli.jetstream_url;
    let ws_url = cli.ws_url;
    let fallback_url = cli.fallback_url;
    let network = cli
        .network
        .unwrap_or_else(|| config.network.default.clone());
    regions::validate_network(&network)?;
    let region = resolve_region(cli.region.as_deref(), &config)?;
    let profile_override = cli.profile;

    let needs_rpc = matches!(
        cli.command,
        Commands::Ping { .. }
            | Commands::Rpc { .. }
            | Commands::Das { .. }
            | Commands::Ws { .. }
            | Commands::Apex { .. }
            | Commands::Dashboard
    );
    let profile = resolve_profile(profile_override.as_deref(), &config);

    let rpc_ctx = if needs_rpc {
        Some(resolve_rpc_context(
            &network,
            &region,
            cli.rpc_url.as_deref(),
            cli.commitment.as_deref(),
            &fallback_url,
            &config,
            profile,
        )?)
    } else {
        None
    };

    match cli.command {
        Commands::Auth { command } => commands::auth::run(command).await,
        Commands::Config { command } => commands::config_cmd::run(command),
        Commands::Ping { service } => {
            let ctx = rpc_ctx.unwrap();
            let mut ping_rpc_urls: Vec<String> = vec![ctx.rpc_url_with_key()];
            ping_rpc_urls.extend(ctx.fallback_urls_with_key());
            let ping_rpc_urls: Vec<String> = ping_rpc_urls
                .into_iter()
                .filter(|u| !u.is_empty())
                .collect();

            let mut ping_js_urls: Vec<String> = Vec::new();
            if let Ok(url) =
                resolve_jetstream_url(&network, &region, jetstream_url.as_deref(), &config)
            {
                ping_js_urls.push(url);
                ping_js_urls.extend(config.endpoints.jetstream.fallback_url.clone());
            }
            let ping_js_urls: Vec<String> =
                ping_js_urls.into_iter().filter(|u| !u.is_empty()).collect();

            let mut ping_grpc_urls = vec![resolve_grpc_url(
                &network,
                &region,
                grpc_url.as_deref(),
                &config,
            )?];
            ping_grpc_urls.extend(if network == "devnet" {
                config.endpoints.grpc.devnet_fallback_url.clone()
            } else {
                config.endpoints.grpc.fallback_url.clone()
            });
            let ping_grpc_urls: Vec<String> = ping_grpc_urls
                .into_iter()
                .filter(|u| !u.is_empty())
                .collect();

            commands::ping::run(service, &ping_rpc_urls, &ping_js_urls, &ping_grpc_urls).await
        }
        Commands::Docs { topic } => commands::docs::run(topic),
        Commands::Jet(args) => {
            let primary = resolve_jetstream_url(
                &network,
                &region,
                jetstream_url.as_deref().or(grpc_url.as_deref()),
                &config,
            )?;
            let fallbacks = if !fallback_url.is_empty() {
                fallback_url
            } else {
                config.endpoints.jetstream.fallback_url.clone()
            };
            commands::jet::run(args, &primary, &fallbacks).await
        }
        Commands::Grpc(args) => {
            let primary = resolve_grpc_url(&network, &region, grpc_url.as_deref(), &config)?;
            let grpc_fallbacks = if !fallback_url.is_empty() {
                fallback_url
            } else if network == "devnet" {
                config.endpoints.grpc.devnet_fallback_url.clone()
            } else {
                config.endpoints.grpc.fallback_url.clone()
            };
            commands::grpc::run(args, &primary, &grpc_fallbacks).await
        }
        Commands::Rpc { command } => {
            let rpc = build_rpc_client(&rpc_ctx.unwrap())?;
            commands::rpc::run(command, &rpc).await
        }
        Commands::Das { command } => {
            let rpc = build_rpc_client(&rpc_ctx.unwrap())?;
            commands::das::run(command, &rpc).await
        }
        Commands::Apex { command } => {
            let ctx = rpc_ctx.unwrap();
            let rpc = commands::apex::needs_rpc(&command)
                .then(|| build_rpc_client(&ctx))
                .transpose()?;
            let apex_key = keyring_store::get_secret(profile.unwrap_or("default"), "apex_key")
                .or_else(|| std::env::var("ORBITFLARE_APEX_KEY").ok())
                .filter(|k| !k.is_empty());
            commands::apex::run(command, apex_key.as_deref(), rpc.as_ref()).await
        }
        Commands::Template(args) => commands::template::run(args, &config).await,
        Commands::Plan { command } => commands::plan::run(command, profile).await,
        Commands::Pay { command } => commands::pay::run(command, profile).await,
        Commands::Dashboard => {
            let ctx = rpc_ctx.unwrap();
            let rpc = if ctx.api_key.is_some() {
                build_rpc_client(&ctx).ok()
            } else {
                None
            };
            commands::dashboard::run(rpc).await
        }
        Commands::Ws { command } => {
            let ctx = rpc_ctx.unwrap();
            let ws = ws_url.unwrap_or_else(|| derive_ws_url(&ctx.rpc_url));
            commands::ws::run(command, &ws, &ctx.commitment, ctx.api_key.as_deref()).await
        }
    }
}

fn build_rpc_client(ctx: &RpcContext) -> CliResult<orbitflare_sdk::RpcClient> {
    let fallback_refs: Vec<&str> = ctx.rpc_fallbacks.iter().map(|s| s.as_str()).collect();
    let mut builder = orbitflare_sdk::RpcClientBuilder::new()
        .url(&ctx.rpc_url)
        .fallback_urls(&fallback_refs)
        .commitment(&ctx.commitment);
    if let Some(ref key) = ctx.api_key {
        builder = builder.api_key(key);
    }
    Ok(builder.build()?)
}

fn derive_ws_url(rpc_url: &str) -> String {
    if let Some(rest) = rpc_url.strip_prefix("https://") {
        format!("wss://{}", rest)
    } else if let Some(rest) = rpc_url.strip_prefix("http://") {
        format!("ws://{}", rest)
    } else {
        rpc_url.to_string()
    }
}
