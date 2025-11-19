pub mod cmd;
pub mod server;

use anyhow::Result;
use serde::{Deserialize, Serialize};

pub type CommandHandler = fn(req: &RestReq) -> Result<RestRes>;

#[derive(Serialize, Deserialize)]
pub struct RestReq {
    pub command: String,
    pub params: String,
}

impl RestReq {
    // "command"に.to_string()を書くのが面倒だっただけ
    pub fn new(command: impl Into<String>, params: String) -> Self {
        Self {
            command: command.into(),
            params,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct RestRes {
    pub response: String,
}
