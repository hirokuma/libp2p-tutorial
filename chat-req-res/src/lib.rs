pub mod cmd;
pub mod server;

use std::pin::Pin;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;

// pub(crate) type CommandHandler = fn(tx: Sender<String>, req: &RestReq) -> Result<RestRes>;
pub(crate) type CommandHandler = Box<dyn Fn(Sender<String>, RestReq) -> Pin<Box<dyn Future<Output = Result<RestRes>> + Send>> + Send + Sync>;

#[derive(Serialize, Deserialize)]
pub struct RestReq {
    pub command: String,
    pub params: String,
}

impl RestReq {
    // "command"に.to_string()を書くのが面倒だっただけ
    pub fn new(command: impl Into<String>, params: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            params: params.into(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct RestRes {
    pub response: String,
}
