use anyhow::Result;

use crate::{RestReq, RestRes};
use tokio::sync::mpsc::Sender;

pub async fn handle(tx: Sender<String>, req: RestReq) -> Result<RestRes> {
    println!("greeting message: {}", req.params);
    tx.send(format!("greet handlerから: {}", req.params)).await?;
    Ok(RestRes{response: "good-bye".to_string()})
}
