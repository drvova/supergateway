use std::sync::Arc;

use tokio::signal::unix::{signal, SignalKind};

use crate::support::logger::Logger;

pub fn install_signal_handlers(logger: Logger, cleanup: Option<Arc<dyn Fn() + Send + Sync>>) {
    let handler = move |name: &'static str, logger: Logger, cleanup: Option<Arc<dyn Fn() + Send + Sync>>| {
        tokio::spawn(async move {
            if let Ok(mut sig) = signal(match name {
                "SIGINT" => SignalKind::interrupt(),
                "SIGTERM" => SignalKind::terminate(),
                _ => SignalKind::hangup(),
            }) {
                sig.recv().await;
                logger.info(format!("Caught {name}. Exiting..."));
                if let Some(cleanup) = cleanup {
                    cleanup();
                }
                std::process::exit(0);
            }
        });
    };

    handler("SIGINT", logger.clone(), cleanup.clone());
    handler("SIGTERM", logger.clone(), cleanup.clone());
    handler("SIGHUP", logger, cleanup);
}
