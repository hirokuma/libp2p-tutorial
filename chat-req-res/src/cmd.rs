mod greet;
mod yebisu;

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use tokio::sync::mpsc::Sender;

use crate::{
    CommandHandler, RestReq, RestRes,
};
use anyhow::Result;

fn wrap<F, Fut>(f: F) -> CommandHandler
where
    F: Fn(Sender<String>, RestReq) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<RestRes>> + Send + 'static,
{
    let f = Arc::new(f);
    Box::new(move |tx, req| {
        let f = f.clone();
        Box::pin(async move {
            (f)(tx, req).await
        })
    })
}

pub fn register_handle<'a>() -> HashMap<&'a str, CommandHandler> {
    let mut handlers: HashMap<&str, CommandHandler> = HashMap::new();

    for (name, handler) in vec![
        ("greet", wrap(greet::handle)),
        ("yebisu", wrap(yebisu::handle)),
    ] {
        handlers.insert(name, handler);
    }

    handlers
}
