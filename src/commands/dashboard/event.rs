use std::time::Duration;
use tokio::sync::mpsc;

use ratatui::crossterm::event::{self, Event as CrosstermEvent, KeyEvent};

#[derive(Debug)]
pub enum Event {
    Key(KeyEvent),
    Tick,
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<Event>,
    _tick_handle: tokio::task::JoinHandle<()>,
    _key_handle: tokio::task::JoinHandle<()>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        let tick_tx = tx.clone();
        let _tick_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tick_rate);
            loop {
                interval.tick().await;
                if tick_tx.send(Event::Tick).is_err() {
                    break;
                }
            }
        });

        let key_tx = tx;
        let _key_handle = tokio::spawn(async move {
            loop {
                let maybe_event = tokio::task::spawn_blocking(|| {
                    if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                        event::read().ok()
                    } else {
                        None
                    }
                })
                .await;

                if let Ok(Some(CrosstermEvent::Key(key))) = maybe_event
                    && key_tx.send(Event::Key(key)).is_err() {
                        break;
                }
            }
        });

        Self {
            rx,
            _tick_handle,
            _key_handle,
        }
    }

    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }
}
