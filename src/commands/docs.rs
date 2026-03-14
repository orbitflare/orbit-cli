use crate::error::CliResult;
use colored::Colorize;

pub fn run(topic: Vec<String>) -> CliResult<()> {
    let url = if topic.is_empty() {
        "https://docs.orbitflare.com".to_string()
    } else {
        format!("https://docs.orbitflare.com/{}", topic.join("/"))
    };

    println!("Opening documentation: {}", url.cyan());
    if let Err(e) = open::that(&url) {
        println!("{} Could not open browser: {}", "Warning:".yellow(), e);
        println!("Visit: {}", url);
    }
    Ok(())
}
