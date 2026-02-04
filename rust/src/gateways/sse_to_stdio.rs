use std::sync::Arc;

use eventsource_stream::Eventsource;
use futures::{StreamExt};
use reqwest::Url;
use tokio::sync::{RwLock, mpsc};
use tokio_util::codec::{FramedRead, LinesCodec};

use crate::config::Config;
use crate::support::signals::install_signal_handlers;
use crate::runtime::{RuntimeApplyResult, RuntimeScope, RuntimeUpdateRequest};
use crate::runtime::store::RuntimeArgsStore;

pub async fn run(
    config: Config,
    runtime: RuntimeArgsStore,
    mut updates: mpsc::Receiver<RuntimeUpdateRequest>,
) -> Result<(), String> {
    let sse_url = config.sse.clone().ok_or("sse url is required")?;
    tracing::info!("  - sse: {sse_url}");
    tracing::info!(
        "  - Headers: {}",
        serde_json::to_string(&config.headers).unwrap_or_else(|_| "(none)".into())
    );
    tracing::info!("Connecting to SSE...");

    install_signal_handlers(None);

    let message_endpoint: Arc<RwLock<Option<Url>>> = Arc::new(RwLock::new(None));
    let headers = config.headers.clone();
    let sse_url_clone = sse_url.clone();
    let message_endpoint_clone = message_endpoint.clone();
    let runtime_clone = runtime.clone();

    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let mut req = client.get(&sse_url_clone);
        for (k, v) in &headers {
            req = req.header(k, v);
        }
        let response = match req.send().await {
            Ok(resp) => resp,
            Err(err) => {
                tracing::error!("SSE connection failed: {err}");
                return;
            }
        };
        let stream = response.bytes_stream().eventsource();
        tokio::pin!(stream);
        while let Some(event) = stream.next().await {
            match event {
                Ok(event) => {
                    if event.event == "endpoint" {
                        if let Ok(url) = Url::parse(&sse_url_clone) {
                            if let Ok(joined) = url.join(&event.data) {
                                *message_endpoint_clone.write().await = Some(joined.clone());
                                tracing::info!("Received message endpoint: {joined}");
                            }
                        }
                        continue;
                    }
                    if event.data.trim().is_empty() {
                        continue;
                    }
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&event.data) {
                        println!("{}", json);
                    }
                }
                Err(err) => {
                    tracing::error!("SSE error: {err}");
                    break;
                }
            }
        }
    });

    let runtime_store = runtime.clone();
    tokio::spawn(async move {
        while let Some(req) = updates.recv().await {
            let result = match req.update.scope {
                RuntimeScope::Global => {
                    let update_result = runtime_store.update_global(req.update.update).await;
                    if update_result.restart_needed {
                        RuntimeApplyResult::ok(
                            "Updated runtime args; env/CLI changes require restart of remote server",
                            false,
                        )
                    } else {
                        RuntimeApplyResult::ok("Updated runtime headers", false)
                    }
                }
                RuntimeScope::Session(_) => RuntimeApplyResult::error(
                    "Per-session runtime overrides are not supported for SSE→stdio",
                ),
            };
            let _ = req.respond_to.send(result);
        }
    });

    let stdin = tokio::io::stdin();
    let mut lines = FramedRead::new(stdin, LinesCodec::new());
    let http = reqwest::Client::new();

    while let Some(line) = lines.next().await {
        let line = line.map_err(|err| err.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(message) = serde_json::from_str::<serde_json::Value>(&line) else {
            tracing::error!("Invalid JSON from stdin: {line}");
            continue;
        };

        let endpoint = loop {
            if let Some(url) = message_endpoint.read().await.clone() {
                break url;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        };

        let runtime_args = runtime_clone.get_effective(None).await;
        let mut req = http.post(endpoint.clone()).json(&message);
        for (k, v) in runtime_args.headers.iter() {
            req = req.header(k, v);
        }

        let response = req.send().await.map_err(|err| err.to_string())?;
        if response.status().is_success() {
            if let Ok(json) = response.json::<serde_json::Value>().await {
                println!("{}", json);
            }
        } else {
            tracing::error!("SSE request failed with status {}", response.status());
        }
    }

    Ok(())
}
