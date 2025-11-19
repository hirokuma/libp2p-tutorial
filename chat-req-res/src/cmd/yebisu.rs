use anyhow::Result;

use crate::{RestReq, RestRes};
use tokio::sync::mpsc::Sender;

pub async fn handle(tx: Sender<String>, req: RestReq) -> Result<RestRes> {
    println!("Yebisu message: {}", req.params);
    tx.send(format!("yebisu handlerから: {}", req.params)).await?;
    Ok(RestRes{response: "Yebisu beer is excellent!".to_string()})
}
