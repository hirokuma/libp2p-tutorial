mod greet;

use std::collections::HashMap;

use crate::CommandHandler;

pub fn register_handle<'a>() -> HashMap<&'a str, CommandHandler> {
    let mut handlers: HashMap<&str, CommandHandler> = HashMap::new();
    handlers.insert("greet", greet::handle);

    handlers
}
