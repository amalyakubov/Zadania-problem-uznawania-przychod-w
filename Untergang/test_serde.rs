use serde_json::json; fn main() { let id = crate::client::ClientId::Individual("123".to_string()); println!("{}", serde_json::to_string(&id).unwrap()); }
