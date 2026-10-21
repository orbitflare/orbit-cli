use crate::error::CliResult;
use colored::Colorize;

const BASE: &str = "https://docs.orbitflare.com";

const TOPICS: &[(&[&str], &str)] = &[
    (&["cli"], "cli"),
    (&["quickstart", "start"], "quickstart"),
    (&["auth", "authentication", "license"], "authentication"),
    (&["rpc", "http"], "rpc/http"),
    (&["ws", "websocket"], "rpc/websocket"),
    (&["jet", "jetstream"], "data-streaming/jetstream"),
    (
        &["grpc", "yellowstone", "geyser"],
        "data-streaming/yellowstone",
    ),
    (&["das", "assets"], "das-api/overview"),
    (&["history"], "fetching-data/getTransactionsForAddress"),
    (&["apex"], "apex/overview"),
    (&["shredstream"], "shredstream"),
    (&["sdk", "sdks"], "sdk/overview"),
    (&["api", "customer-api"], "api-documentation/welcome"),
    (&["regions", "infrastructure"], "infrastructure"),
];

fn resolve(topic: &[String]) -> String {
    if topic.is_empty() {
        return BASE.to_string();
    }
    let joined = topic.join("/");
    let path = TOPICS
        .iter()
        .find(|(names, _)| names.contains(&joined.to_lowercase().as_str()))
        .map(|(_, path)| path.to_string())
        .unwrap_or(joined);
    format!("{}/{}", BASE, path)
}

pub fn run(topic: Vec<String>) -> CliResult<()> {
    let url = resolve(&topic);

    println!("Opening documentation: {}", url.cyan());
    if let Err(e) = open::that(&url) {
        println!("{} Could not open browser: {}", "Warning:".yellow(), e);
        println!("Visit: {}", url);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_topic_aliases() {
        let t = |s: &str| resolve(&s.split(' ').map(String::from).collect::<Vec<_>>());
        assert_eq!(resolve(&[]), BASE);
        assert_eq!(t("jet"), format!("{BASE}/data-streaming/jetstream"));
        assert_eq!(t("DAS"), format!("{BASE}/das-api/overview"));
        assert_eq!(
            t("rpc methods getSlot"),
            format!("{BASE}/rpc/methods/getSlot")
        );
    }
}
