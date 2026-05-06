use crate::cli::ConfigCommands;
use crate::config::Config;
use crate::error::CliResult;
use crate::output;
use colored::Colorize;

pub fn run(cmd: ConfigCommands) -> CliResult<()> {
    match cmd {
        ConfigCommands::Show => show(),
        ConfigCommands::Set { key, value } => set(key, value),
        ConfigCommands::Remove { key, value } => remove(key, value),
        ConfigCommands::Reset => reset(),
    }
}

fn show() -> CliResult<()> {
    let config = Config::load()?;

    if output::is_json() {
        println!("{}", serde_json::to_string_pretty(&config)?);
        return Ok(());
    }

    let profile = config
        .auth
        .default_profile
        .as_ref()
        .and_then(|p| config.auth.profiles.get(p));

    let email_display = profile
        .map(|p| p.email.clone())
        .filter(|e| !e.is_empty())
        .unwrap_or_else(|| "(not set)".to_string());
    let profile_name = config.auth.default_profile.as_deref().unwrap_or("default");
    let api_key_display = crate::keyring_store::get_secret(profile_name, "api_key")
        .map(|k| mask_token(&k))
        .unwrap_or_else(|| "(not set)".to_string());

    println!();
    println!("  {}", "Auth".bold());
    println!("    {:<20} {}", "email:", email_display);
    println!("    {:<20} {}", "api_key:", api_key_display);

    println!();
    println!("  {}", "Network".bold());
    println!("    {:<20} {}", "default:", config.network.default);

    println!();
    println!("  {}", "Endpoints".bold());
    println!("    {:<20} {}", "rpc.url:", config.endpoints.rpc.url);
    println!("    {:<20} {}", "rpc.devnet_url:", config.endpoints.rpc.devnet_url);
    if !config.endpoints.rpc.fallback_url.is_empty() {
        for (i, fb) in config.endpoints.rpc.fallback_url.iter().enumerate() {
            println!("    {:<20} {}", format!("rpc.fallback[{}]:", i), fb);
        }
    }
    println!(
        "    {:<20} {}",
        "jetstream.url:", config.endpoints.jetstream.url
    );
    if !config.endpoints.jetstream.fallback_url.is_empty() {
        for (i, fb) in config.endpoints.jetstream.fallback_url.iter().enumerate() {
            println!(
                "    {:<20} {}",
                format!("jetstream.fallback[{}]:", i),
                fb
            );
        }
    }
    println!(
        "    {:<20} {}",
        "grpc.url:", config.endpoints.grpc.url
    );
    println!(
        "    {:<20} {}",
        "grpc.devnet_url:", config.endpoints.grpc.devnet_url
    );
    if !config.endpoints.grpc.fallback_url.is_empty() {
        for (i, fb) in config.endpoints.grpc.fallback_url.iter().enumerate() {
            println!(
                "    {:<20} {}",
                format!("grpc.fallback[{}]:", i),
                fb
            );
        }
    }
    println!();
    println!("  {}", "Preferences".bold());
    println!(
        "    {:<20} {}",
        "output_format:", config.preferences.output_format
    );
    println!("    {:<20} {}", "color:", config.preferences.color);
    println!(
        "    {:<20} {}",
        "commitment:", config.preferences.commitment
    );

    println!();
    println!("  {}", "Templates".bold());
    println!("    {:<20} {}", "repo:", config.templates.repo);

    println!();

    Ok(())
}

fn resolve_list<'a>(config: &'a mut Config, key: &str) -> Option<&'a mut Vec<String>> {
    match key {
        "rpc.fallback" => Some(&mut config.endpoints.rpc.fallback_url),
        "rpc.devnet_fallback" => Some(&mut config.endpoints.rpc.devnet_fallback_url),
        "jetstream.fallback" => Some(&mut config.endpoints.jetstream.fallback_url),
        "grpc.fallback" => Some(&mut config.endpoints.grpc.fallback_url),
        "grpc.devnet_fallback" => Some(&mut config.endpoints.grpc.devnet_fallback_url),
        _ => None,
    }
}

fn set(key: String, value: String) -> CliResult<()> {
    let mut config = Config::load()?;

    if let Some(list) = resolve_list(&mut config, &key) {
        if !list.contains(&value) {
            list.push(value.clone());
        }
    } else {
        match key.as_str() {
            "network" => config.network.default = value.clone(),
            "output_format" => config.preferences.output_format = value.clone(),
            "color" => config.preferences.color = value.parse().map_err(|e| format!("Invalid boolean value: {}", e))?,
            "commitment" => config.preferences.commitment = value.clone(),
            "theme" => config.preferences.theme = value.clone(),
            "rpc.url" | "rpc" => config.endpoints.rpc.url = value.clone(),
            "rpc.devnet_url" => config.endpoints.rpc.devnet_url = value.clone(),
            "grpc.devnet_url" => config.endpoints.grpc.devnet_url = value.clone(),
            "jetstream.url" | "jetstream" => config.endpoints.jetstream.url = value.clone(),
            "grpc.url" | "grpc" => config.endpoints.grpc.url = value.clone(),
            "templates.repo" | "repo" => config.templates.repo = value.clone(),
            _ => return Err(format!("Unknown config key: '{}'\nAvailable: network, output_format, color, commitment, theme, rpc.url, rpc.devnet_url, rpc.fallback, rpc.devnet_fallback, jetstream.url, jetstream.fallback, grpc.url, grpc.devnet_url, grpc.fallback, grpc.devnet_fallback, templates.repo", key).into()),
        }
    }

    config.save()?;
    println!("{} Set {} = {}", "✓".green().bold(), key, value);
    Ok(())
}

fn remove(key: String, value: String) -> CliResult<()> {
    let mut config = Config::load()?;

    if let Some(list) = resolve_list(&mut config, &key) {
        let before = list.len();
        list.retain(|u| u != &value);
        if list.len() == before {
            return Err(format!("'{}' not found in {}", value, key).into());
        }
    } else {
        return Err(format!("'{}' is not a list config. Only rpc.fallback, rpc.devnet_fallback, jetstream.fallback, grpc.fallback, grpc.devnet_fallback support remove.", key).into());
    }

    config.save()?;
    println!("{} Removed {} from {}", "✓".green().bold(), value, key);
    Ok(())
}

fn reset() -> CliResult<()> {
    let existing = Config::load()?;
    let config = Config {
        auth: existing.auth,
        ..Config::default()
    };
    config.save()?;
    println!("{} Configuration reset to defaults", "✓".green().bold());
    Ok(())
}

fn mask_token(token: &str) -> String {
    if token.len() <= 8 {
        return "****".to_string();
    }
    let prefix = &token[..std::cmp::min(7, token.len())];
    let suffix = &token[token.len().saturating_sub(4)..];
    format!("{}...{}", prefix, suffix)
}
