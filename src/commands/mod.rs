pub mod apex;
pub mod auth;
pub mod config_cmd;
pub mod das;
pub mod dashboard;
pub mod docs;
pub mod grpc;
pub mod jet;
pub mod pay;
pub mod ping;
pub mod plan;
pub mod rpc;
pub mod template;
pub mod ws;

pub fn shutdown_signal(duration: Option<u64>) -> tokio::sync::watch::Receiver<bool> {
    let (tx, rx) = tokio::sync::watch::channel(false);
    tokio::spawn(async move {
        match duration {
            Some(secs) => tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = tokio::time::sleep(std::time::Duration::from_secs(secs)) => {}
            },
            None => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
        let _ = tx.send(true);
    });
    rx
}
