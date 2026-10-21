use crate::cli::TemplateArgs;
use crate::config::Config;
use crate::error::CliResult;
use colored::Colorize;
use serde::Deserialize;

#[derive(Debug, Deserialize, serde::Serialize, Clone)]
struct TemplateInfo {
    name: String,
    description: String,
    lang: String,
    level: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    repo: String,
    #[serde(default)]
    setup_steps: Vec<String>,
}

fn raw_url(repo: &str) -> Option<String> {
    let stripped = repo.trim_end_matches('/');
    let path = stripped.strip_prefix("https://github.com/")?;
    Some(format!(
        "https://raw.githubusercontent.com/{}/main/templates.json",
        path
    ))
}

async fn fetch_templates(config: &Config) -> CliResult<Vec<TemplateInfo>> {
    let cache_path = Config::dir().join("templates_cache.json");

    if let Some(url) = raw_url(&config.templates.repo) {
        match reqwest::Client::new()
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                let body = resp.text().await?;
                let templates: Vec<TemplateInfo> = serde_json::from_str(&body)?;
                let _ = std::fs::create_dir_all(Config::dir());
                let _ = std::fs::write(&cache_path, &body);
                return Ok(templates);
            }
            Ok(resp) => {
                eprintln!(
                    "  {} Could not fetch template registry (HTTP {}), using cache.",
                    "!".yellow(),
                    resp.status()
                );
            }
            Err(e) => {
                eprintln!(
                    "  {} Could not reach template registry ({}), using cache.",
                    "!".yellow(),
                    e
                );
            }
        }
    }

    if cache_path.exists() {
        let body = std::fs::read_to_string(&cache_path)?;
        let templates: Vec<TemplateInfo> = serde_json::from_str(&body)?;
        return Ok(templates);
    }

    Err("No templates available. Check your network connection and try again.".into())
}

pub async fn run(args: TemplateArgs, config: &Config) -> CliResult<()> {
    if args.list {
        let templates = fetch_templates(config).await?;
        return list(&templates, args.filter.as_deref());
    }
    if let Some(ref name) = args.view {
        let templates = fetch_templates(config).await?;
        return view(name, &templates, config);
    }
    if let Some(ref name) = args.install {
        let templates = fetch_templates(config).await?;
        return install(name, args.dir, &templates, config).await;
    }

    println!("Usage: orbitflare template [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --list              List all available templates");
    println!("  --filter <lang>     Filter by language (rust|ts)");
    println!("  --view <name>       View template details");
    println!("  --install <name>    Install a template");
    println!("  --dir <path>        Target directory for installation");
    Ok(())
}

fn normalize_lang(input: &str) -> String {
    match input.to_lowercase().as_str() {
        "typescript" | "ts" => "ts".into(),
        "rust" | "rs" => "rust".into(),
        other => other.to_string(),
    }
}

fn list(templates: &[TemplateInfo], filter: Option<&str>) -> CliResult<()> {
    let filtered: Vec<&TemplateInfo> = match filter {
        Some(lang) => {
            let norm = normalize_lang(lang);
            templates
                .iter()
                .filter(|t| t.lang.to_lowercase().contains(&norm))
                .collect()
        }
        None => templates.iter().collect(),
    };

    if crate::output::is_json() {
        println!("{}", serde_json::to_string_pretty(&filtered)?);
        return Ok(());
    }

    println!(
        "\n  {:<22} {:<10} {:<13} {}",
        "Template".bold(),
        "Lang".bold(),
        "Level".bold(),
        "Description".bold()
    );
    println!("  {}", "─".repeat(78));
    for t in &filtered {
        println!(
            "  {:<22} {:<10} {:<13} {}",
            t.name, t.lang, t.level, t.description
        );
    }
    if let Some(lang) = filter {
        println!(
            "\n  {} of {} templates match filter '{}'.",
            filtered.len(),
            templates.len(),
            lang
        );
    } else {
        println!("\n  {} templates available.", templates.len());
    }
    println!(
        "  Install with: {}",
        "orbitflare template --install <name>".cyan()
    );
    println!();
    Ok(())
}

fn view(name: &str, templates: &[TemplateInfo], config: &Config) -> CliResult<()> {
    let t = templates.iter().find(|t| t.name == name);
    match t {
        Some(t) => {
            println!();
            println!("  {}", t.name.bold().cyan());
            println!("  Language:    {}", t.lang);
            println!("  Level:       {}", t.level);
            println!("  Description: {}", t.description);
            if !t.tags.is_empty() {
                println!("  Tags:        {}", t.tags.join(", "));
            }
            println!();
            let source = if t.repo.is_empty() {
                format!("{}/tree/main/{}", config.templates.repo, t.name)
            } else {
                t.repo.clone()
            };
            println!("  {} {}", "Source:".bold(), source);
            println!();
            println!("  {} This template includes:", "Contents:".bold());
            if t.lang.contains("rust") {
                println!("    - Cargo.toml with all dependencies");
                println!("    - src/ with full project structure");
            }
            if t.lang.contains("ts") {
                println!("    - package.json with all dependencies");
                println!("    - src/ with TypeScript project structure");
            }
            if !t.lang.contains("rust") && !t.lang.contains("ts") {
                println!("    - Full project structure");
            }
            println!("    - .env.example with required environment variables");
            println!("    - README.md with setup instructions");
            println!();
            println!(
                "  Install with: {}",
                format!("orbitflare template --install {}", t.name).cyan()
            );
            println!();
        }
        None => {
            let available: Vec<&str> = templates.iter().map(|t| t.name.as_str()).collect();
            return Err(format!(
                "Unknown template: '{}'. Available: {}",
                name,
                available.join(", ")
            )
            .into());
        }
    }
    Ok(())
}

async fn install(
    name: &str,
    dir: Option<String>,
    templates: &[TemplateInfo],
    config: &Config,
) -> CliResult<()> {
    let t = match templates.iter().find(|t| t.name == name) {
        Some(t) => t,
        None => {
            let available: Vec<&str> = templates.iter().map(|t| t.name.as_str()).collect();
            return Err(format!(
                "Unknown template: '{}'. Available: {}",
                name,
                available.join(", ")
            )
            .into());
        }
    };

    let target_dir = dir.unwrap_or_else(|| format!("./{}", name));

    if std::path::Path::new(&target_dir).exists() {
        return Err(format!(
            "Directory '{}' already exists. Use --dir to specify a different path.",
            target_dir
        )
        .into());
    }

    println!();
    println!(
        "  {} Installing template: {}",
        "●".green().bold(),
        name.cyan().bold()
    );
    println!("  {} {}", "Target:".dimmed(), target_dir);
    println!(
        "  {} {}",
        "Source:".dimmed(),
        config.templates.repo.dimmed()
    );
    println!();

    let temp_dir = std::env::temp_dir().join(format!("orbitflare-template-{}", std::process::id()));

    let _ = std::fs::remove_dir_all(&temp_dir);

    println!("  {} Cloning templates repository...", "→".dimmed());

    let output = tokio::process::Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "--single-branch",
            &config.templates.repo,
            temp_dir.to_str().unwrap(),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .await?;

    if !output.status.success() {
        let _ = std::fs::remove_dir_all(&temp_dir);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Failed to clone templates repository.\nEnsure git is installed and you have access to: {}\n{}",
            config.templates.repo,
            stderr.trim()
        )
        .into());
    }

    let src = temp_dir.join(name);
    if !src.exists() {
        let available: Vec<String> = std::fs::read_dir(&temp_dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
                    .filter(|e| e.file_name() != ".git")
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect()
            })
            .unwrap_or_default();

        let _ = std::fs::remove_dir_all(&temp_dir);
        return Err(format!(
            "Template '{}' not found in repository.\nAvailable: {}",
            name,
            available.join(", ")
        )
        .into());
    }

    println!("  {} Copying project files...", "→".dimmed());
    copy_dir_recursive(&src, &std::path::PathBuf::from(&target_dir))?;

    let _ = std::fs::remove_dir_all(&temp_dir);

    let file_count = count_files(&std::path::PathBuf::from(&target_dir));

    println!(
        "  {} Created {} files in {}",
        "✓".green().bold(),
        file_count,
        target_dir.cyan()
    );
    println!();
    println!("  {}", "Next steps:".bold());
    println!("    cd {}", target_dir);
    if t.setup_steps.is_empty() {
        if t.lang.contains("rust") {
            println!("    cargo build");
        }
        if t.lang.contains("ts") {
            println!("    npm install");
        }
    } else {
        for step in &t.setup_steps {
            println!("    {}", step);
        }
    }
    println!();

    Ok(())
}
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> CliResult<()> {
    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = dst.join(entry.file_name());

        if file_type.is_dir() {
            let dir_name = entry.file_name();
            if dir_name == ".git" || dir_name == "target" || dir_name == "node_modules" {
                continue;
            }
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }

    Ok(())
}

fn count_files(path: &std::path::Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                count += count_files(&entry.path());
            } else {
                count += 1;
            }
        }
    }
    count
}
