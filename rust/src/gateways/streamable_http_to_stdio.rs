use std::sync::Arc;

use eventsource_stream::Eventsource;
use futures::StreamExt;
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
    let streamable_http_url = config
        .streamable_http
        .clone()
        .ok_or("streamableHttp url is required")?;
    tracing::info!("  - streamableHttp: {streamable_http_url}");
    tracing::info!(
        "  - Headers: {}",
        serde_json::to_string(&config.headers).unwrap_or_else(|_| "(none)".into())
    );
    tracing::info!("Connecting to Streamable HTTP...");

    install_signal_handlers(None);

    let session_id: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));
    let session_for_sse = session_id.clone();
    let headers = config.headers.clone();

    let http = reqwest::Client::new();
    let session_clone = session_id.clone();
    let runtime_clone = runtime.clone();
    let headers_clone = headers.clone();
    let http_clone = http.clone();
    let url_clone = streamable_http_url.clone();
    tokio::spawn(async move {
        loop {
            let Some(sid) = session_for_sse.read().await.clone() else {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                continue;
            };
            let mut req = http_clone.get(&url_clone).header("Accept", "text/event-stream");
            for (k, v) in headers_clone.iter().chain(runtime_clone.get_effective(None).await.headers.iter()) {
                req = req.header(k, v);
            }
            req = req.header("Mcp-Session-Id", sid.clone());
            let response = match req.send().await {
                Ok(resp) => resp,
                Err(err) => {
                    tracing::error!("Streamable HTTP SSE connection failed: {err}");
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    continue;
                }
            };
            let stream = response.bytes_stream().eventsource();
            tokio::pin!(stream);
            while let Some(event) = stream.next().await {
                match event {
                    Ok(event) => {
                        if event.data.trim().is_empty() {
                            continue;
                        }
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&event.data) {
                            println!("{}", json);
                        }
                    }
                    Err(err) => {
                        tracing::error!("Streamable HTTP SSE error: {err}");
                        break;
                    }
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
                    "Per-session runtime overrides are not supported for StreamableHTTP→stdio",
                ),
            };
            let _ = req.respond_to.send(result);
        }
    });

    let mut lines = FramedRead::new(tokio::io::stdin(), LinesCodec::new());

    while let Some(line) = lines.next().await {
        let line = line.map_err(|err| err.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(message) = serde_json::from_str::<serde_json::Value>(&line) else {
            tracing::error!("Invalid JSON from stdin: {line}");
            continue;
        };

        let runtime_args = runtime.get_effective(None).await;
        let mut req = http.post(&streamable_http_url).json(&message);
        for (k, v) in headers.iter().chain(runtime_args.headers.iter()) {
            req = req.header(k, v);
        }
        if let Some(sid) = session_clone.read().await.clone() {
            req = req.header("Mcp-Session-Id", sid);
        }

        let response = req.send().await.map_err(|err| err.to_string())?;
        if let Some(sid) = response.headers().get("Mcp-Session-Id").and_then(|v| v.to_str().ok()) {
            *session_clone.write().await = Some(sid.to_string());
        }
        if response.status().is_success() {
            if let Ok(json) = response.json::<serde_json::Value>().await {
                println!("{}", json);
            }
        } else {
            tracing::error!(
                "Streamable HTTP request failed with status {}",
                response.status()
            );
        }
    }

    Ok(())
}
