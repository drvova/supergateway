use crate::config::{LogLevel, OutputTransport};

#[derive(Clone)]
pub struct Logger {
    level: LogLevel,
    output_transport: OutputTransport,
}

impl Logger {
    pub fn new(level: LogLevel, output_transport: OutputTransport) -> Self {
        Self {
            level,
            output_transport,
        }
    }

    pub fn info<S: AsRef<str>>(&self, msg: S) {
        if matches!(self.level, LogLevel::None) {
            return;
        }
        let line = format!("[supergateway] {}", msg.as_ref());
        if matches!(self.output_transport, OutputTransport::Stdio) {
            eprintln!("{}", line);
        } else {
            println!("{}", line);
        }
    }

    pub fn debug<S: AsRef<str>>(&self, msg: S) {
        if !matches!(self.level, LogLevel::Debug) {
            return;
        }
        let line = format!("[supergateway] {}", msg.as_ref());
        if matches!(self.output_transport, OutputTransport::Stdio) {
            eprintln!("{}", line);
        } else {
            println!("{}", line);
        }
    }

    pub fn error<S: AsRef<str>>(&self, msg: S) {
        if matches!(self.level, LogLevel::None) {
            return;
        }
        let line = format!("[supergateway] {}", msg.as_ref());
        eprintln!("{}", line);
    }
}
