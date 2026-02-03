use clap::{Arg, ArgAction, Command, ValueEnum};
use std::collections::HashMap;
use std::env;
use std::fmt;

use crate::types::HeadersMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputTransport {
    Stdio,
    Sse,
    Ws,
    StreamableHttp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LogLevel {
    Debug,
    Info,
    None,
}

#[derive(Debug, Clone)]
pub enum CorsConfig {
    Disabled,
    AllowAll,
    AllowList { raw: Vec<String> },
}

#[derive(Debug, Clone)]
pub struct Config {
    pub stdio: Option<String>,
    pub sse: Option<String>,
    pub streamable_http: Option<String>,
    pub output_transport: OutputTransport,
    pub port: u16,
    pub base_url: String,
    pub sse_path: String,
    pub message_path: String,
    pub streamable_http_path: String,
    pub log_level: LogLevel,
    pub cors: CorsConfig,
    pub health_endpoints: Vec<String>,
    pub headers: HeadersMap,
    pub stateful: bool,
    pub session_timeout: Option<u64>,
    pub protocol_version: String,
    pub runtime_prompt: bool,
    pub runtime_admin_port: Option<u16>,
}

#[derive(Debug)]
pub enum ConfigError {
    MissingTransport,
    MultipleTransports,
    InvalidSessionTimeout(String),
    InvalidRuntimePort(String),
    InvalidArg(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::MissingTransport => {
                write!(f, "You must specify one of --stdio, --sse, or --streamableHttp")
            }
            ConfigError::MultipleTransports => write!(
                f,
                "Specify only one of --stdio, --sse, or --streamableHttp"
            ),
            ConfigError::InvalidSessionTimeout(msg) => write!(f, "{msg}"),
            ConfigError::InvalidRuntimePort(msg) => write!(f, "{msg}"),
            ConfigError::InvalidArg(msg) => write!(f, "{msg}"),
        }
    }
}

pub fn parse_config() -> Result<Config, ConfigError> {
    let raw_args: Vec<String> = env::args().collect();
    let default_output = default_output_transport(&raw_args);
    let cors_input = parse_cors_flags(&raw_args);

    let matches = Command::new("supergateway")
        .arg(Arg::new("stdio").long("stdio").value_name("CMD"))
        .arg(Arg::new("sse").long("sse").value_name("URL"))
        .arg(
            Arg::new("streamableHttp")
                .long("streamableHttp")
                .value_name("URL"),
        )
        .arg(
            Arg::new("outputTransport")
                .long("outputTransport")
                .value_parser(clap::builder::EnumValueParser::<OutputTransport>::new())
                .value_name("stdio|sse|ws|streamableHttp"),
        )
        .arg(
            Arg::new("port")
                .long("port")
                .value_name("PORT")
                .default_value("8000"),
        )
        .arg(
            Arg::new("baseUrl")
                .long("baseUrl")
                .value_name("URL")
                .default_value(""),
        )
        .arg(
            Arg::new("ssePath")
                .long("ssePath")
                .value_name("PATH")
                .default_value("/sse"),
        )
        .arg(
            Arg::new("messagePath")
                .long("messagePath")
                .value_name("PATH")
                .default_value("/message"),
        )
        .arg(
            Arg::new("streamableHttpPath")
                .long("streamableHttpPath")
                .value_name("PATH")
                .default_value("/mcp"),
        )
        .arg(
            Arg::new("logLevel")
                .long("logLevel")
                .value_parser(clap::builder::EnumValueParser::<LogLevel>::new())
                .default_value("info"),
        )
        .arg(
            Arg::new("cors")
                .long("cors")
                .num_args(0..=1)
                .action(ArgAction::Append)
                .value_name("ORIGIN"),
        )
        .arg(
            Arg::new("healthEndpoint")
                .long("healthEndpoint")
                .action(ArgAction::Append)
                .value_name("PATH")
                .default_value(""),
        )
        .arg(
            Arg::new("header")
                .long("header")
                .action(ArgAction::Append)
                .value_name("HEADER"),
        )
        .arg(
            Arg::new("oauth2Bearer")
                .long("oauth2Bearer")
                .value_name("TOKEN"),
        )
        .arg(
            Arg::new("stateful")
                .long("stateful")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("sessionTimeout")
                .long("sessionTimeout")
                .value_name("MILLISECONDS"),
        )
        .arg(
            Arg::new("protocolVersion")
                .long("protocolVersion")
                .default_value("2024-11-05"),
        )
        .arg(
            Arg::new("runtimePrompt")
                .long("runtimePrompt")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("runtimeAdminPort")
                .long("runtimeAdminPort")
                .value_name("PORT"),
        )
        .get_matches();

    let stdio = matches.get_one::<String>("stdio").cloned();
    let sse = matches.get_one::<String>("sse").cloned();
    let streamable_http = matches
        .get_one::<String>("streamableHttp")
        .cloned();

    let active = [stdio.is_some(), sse.is_some(), streamable_http.is_some()]
        .iter()
        .filter(|v| **v)
        .count();
    if active == 0 {
        return Err(ConfigError::MissingTransport);
    }
    if active > 1 {
        return Err(ConfigError::MultipleTransports);
    }

    let output_transport = matches
        .get_one::<OutputTransport>("outputTransport")
        .copied()
        .or(default_output)
        .ok_or_else(|| {
            ConfigError::InvalidArg(
                "outputTransport must be specified or inferable from input transport".into(),
            )
        })?;

    let port = matches
        .get_one::<String>("port")
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(8000);
    let base_url = matches
        .get_one::<String>("baseUrl")
        .cloned()
        .unwrap_or_default();
    let sse_path = matches
        .get_one::<String>("ssePath")
        .cloned()
        .unwrap_or_else(|| "/sse".to_string());
    let message_path = matches
        .get_one::<String>("messagePath")
        .cloned()
        .unwrap_or_else(|| "/message".to_string());
    let streamable_http_path = matches
        .get_one::<String>("streamableHttpPath")
        .cloned()
        .unwrap_or_else(|| "/mcp".to_string());
    let log_level = matches
        .get_one::<LogLevel>("logLevel")
        .copied()
        .unwrap_or(LogLevel::Info);

    let health_endpoints: Vec<String> = matches
        .get_many::<String>("healthEndpoint")
        .map(|vals| {
            vals.filter(|v| !v.is_empty()).map(|v| v.to_string()).collect()
        })
        .unwrap_or_default();

    let header_values: Vec<String> = matches
        .get_many::<String>("header")
        .map(|vals| vals.map(|v| v.to_string()).collect())
        .unwrap_or_default();

    let oauth2_bearer = matches.get_one::<String>("oauth2Bearer").cloned();
    let headers = parse_headers(&header_values, oauth2_bearer.as_deref())?;

    let cors = if cors_input.present {
        if cors_input.allow_all {
            CorsConfig::AllowAll
        } else if !cors_input.values.is_empty() {
            CorsConfig::AllowList {
                raw: cors_input.values,
            }
        } else {
            CorsConfig::AllowAll
        }
    } else {
        CorsConfig::Disabled
    };

    let stateful = matches.get_flag("stateful");
    let session_timeout = if let Some(raw) = matches.get_one::<String>("sessionTimeout") {
        let val: i64 = raw.parse().map_err(|_| {
            ConfigError::InvalidSessionTimeout(format!(
                "sessionTimeout must be a positive number, received: {raw}"
            ))
        })?;
        if val <= 0 {
            return Err(ConfigError::InvalidSessionTimeout(format!(
                "sessionTimeout must be a positive number, received: {raw}"
            )));
        }
        Some(val as u64)
    } else {
        None
    };

    let protocol_version = matches
        .get_one::<String>("protocolVersion")
        .cloned()
        .unwrap_or_else(|| "2024-11-05".to_string());

    let runtime_prompt = matches.get_flag("runtimePrompt");
    let runtime_admin_port = if let Some(raw) = matches.get_one::<String>("runtimeAdminPort") {
        let val: i64 = raw.parse().map_err(|_| {
            ConfigError::InvalidRuntimePort(format!(
                "runtimeAdminPort must be a valid port, received: {raw}"
            ))
        })?;
        if val <= 0 || val > u16::MAX as i64 {
            return Err(ConfigError::InvalidRuntimePort(format!(
                "runtimeAdminPort must be in 1..=65535, received: {raw}"
            )));
        }
        Some(val as u16)
    } else {
        None
    };

    Ok(Config {
        stdio,
        sse,
        streamable_http,
        output_transport,
        port,
        base_url,
        sse_path,
        message_path,
        streamable_http_path,
        log_level,
        cors,
        health_endpoints,
        headers,
        stateful,
        session_timeout,
        protocol_version,
        runtime_prompt,
        runtime_admin_port,
    })
}

fn default_output_transport(args: &[String]) -> Option<OutputTransport> {
    if args.iter().any(|arg| arg == "--stdio") {
        return Some(OutputTransport::Sse);
    }
    if args.iter().any(|arg| arg == "--sse") {
        return Some(OutputTransport::Stdio);
    }
    if args.iter().any(|arg| arg == "--streamableHttp") {
        return Some(OutputTransport::Stdio);
    }
    None
}

#[derive(Default)]
struct CorsInput {
    present: bool,
    allow_all: bool,
    values: Vec<String>,
}

fn parse_cors_flags(args: &[String]) -> CorsInput {
    let mut input = CorsInput::default();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--cors" {
            input.present = true;
            let next = args.get(i + 1);
            if let Some(next_val) = next {
                if next_val.starts_with("--") {
                    input.allow_all = true;
                } else {
                    input.values.push(next_val.clone());
                    i += 1;
                }
            } else {
                input.allow_all = true;
            }
        }
        i += 1;
    }
    input
}

fn parse_headers(header_values: &[String], oauth2_bearer: Option<&str>) -> Result<HeadersMap, ConfigError> {
    let mut headers: HashMap<String, String> = HashMap::new();
    for raw in header_values {
        let Some((key, value)) = raw.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            continue;
        }
        headers.insert(key.to_string(), value.to_string());
    }
    if let Some(token) = oauth2_bearer {
        headers.insert("Authorization".to_string(), format!("Bearer {token}"));
    }
    Ok(headers)
}
