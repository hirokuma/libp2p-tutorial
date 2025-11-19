use anyhow::Result;

use crate::{RestReq, RestRes};

pub fn handle(req: &RestReq) -> Result<RestRes> {
    println!("greeting message: {}", req.params);
    Ok(RestRes{response: "good-bye".to_string()})
}
