mod config;
mod gateways;
mod support;
mod runtime;
mod types;

use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};
use futures::future::BoxFuture;

use crate::config::{parse_config, OutputTransport};
use crate::gateways::{
    sse_to_stdio, stdio_to_sse, stdio_to_streamable_http, stdio_to_ws,
    streamable_http_to_stdio,
};
use crate::support::logger::Logger;
use crate::runtime::{RuntimeApplyResult, RuntimeUpdate, RuntimeUpdateRequest};
use crate::runtime::admin::spawn_admin_server;
use crate::runtime::prompt::spawn_prompt;
use crate::runtime::store::RuntimeArgsStore;
use crate::types::RuntimeArgs;

#[tokio::main]
async fn main() {
    let config = match parse_config() {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("[supergateway] Error: {err}");
            std::process::exit(1);
        }
    };

    let logger = Logger::new(config.log_level, config.output_transport);
    logger.info("Starting...");
    logger.info(
        "Supergateway is supported by Supermachine (hosted MCPs) - https://supermachine.ai",
    );
    logger.info(format!("  - outputTransport: {:?}", config.output_transport));

    let runtime_store = RuntimeArgsStore::new(RuntimeArgs {
        headers: config.headers.clone(),
        ..Default::default()
    });

    let (update_tx, update_rx) = mpsc::channel::<RuntimeUpdateRequest>(32);

    if config.runtime_prompt {
        let mut prompt_rx = spawn_prompt(logger.clone());
        let update_tx = update_tx.clone();
        let logger_clone = logger.clone();
        tokio::spawn(async move {
            while let Some(update) = prompt_rx.recv().await {
                let (resp_tx, resp_rx) = oneshot::channel();
                if update_tx
                    .send(RuntimeUpdateRequest {
                        update,
                        respond_to: resp_tx,
                    })
                    .await
                    .is_err()
                {
                    logger_clone.error("Runtime update channel closed");
                    break;
                }
                if let Ok(result) = resp_rx.await {
                    logger_clone.info(format!("Runtime update: {}", result.message));
                }
            }
        });
    }

    if let Some(port) = config.runtime_admin_port {
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        let update_tx = update_tx.clone();
        let handler: Arc<dyn Fn(RuntimeUpdate) -> BoxFuture<'static, RuntimeApplyResult> + Send + Sync> =
            Arc::new(move |update: RuntimeUpdate| {
            let update_tx = update_tx.clone();
            Box::pin(async move {
                let (resp_tx, resp_rx) = oneshot::channel();
                if update_tx
                    .send(RuntimeUpdateRequest {
                        update,
                        respond_to: resp_tx,
                    })
                    .await
                    .is_err()
                {
                    return RuntimeApplyResult::error("Runtime update channel closed");
                }
                resp_rx.await.unwrap_or_else(|_| {
                    RuntimeApplyResult::error("Runtime update handler failed")
                })
            }) as BoxFuture<'static, RuntimeApplyResult>
        });
        let runtime_clone = runtime_store.clone();
        let logger_clone = logger.clone();
        tokio::spawn(async move {
            spawn_admin_server(addr, runtime_clone, handler, logger_clone).await;
        });
    }

    let result = if config.stdio.is_some() {
        match config.output_transport {
            OutputTransport::Sse => stdio_to_sse::run(config, logger, runtime_store, update_rx).await,
            OutputTransport::Ws => stdio_to_ws::run(config, logger, runtime_store, update_rx).await,
            OutputTransport::StreamableHttp => {
                stdio_to_streamable_http::run(config, logger, runtime_store, update_rx).await
            }
            OutputTransport::Stdio => Err("stdio→stdio is not supported".to_string()),
        }
    } else if config.sse.is_some() {
        match config.output_transport {
            OutputTransport::Stdio => sse_to_stdio::run(config, logger, runtime_store, update_rx).await,
            _ => Err("sse→output transport not supported".to_string()),
        }
    } else if config.streamable_http.is_some() {
        match config.output_transport {
            OutputTransport::Stdio => {
                streamable_http_to_stdio::run(config, logger, runtime_store, update_rx).await
            }
            _ => Err("streamableHttp→output transport not supported".to_string()),
        }
    } else {
        Err("Invalid input transport".to_string())
    };

    if let Err(err) = result {
        eprintln!("[supergateway] Fatal error: {err}");
        std::process::exit(1);
    }
}
