use json::JsonValue;
use std::time::SystemTime;

pub fn time() -> u128 {
    match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(n) => n.as_millis(),
        Err(_) => panic!("SystemTime before UNIX EPOCH!"),
    }
}

pub fn parse_f64_field(data: &JsonValue, field: &str) -> f64 {
    data[field]
        .as_str()
        .expect(&format!("Missing '{field}' field or not a string"))
        .parse()
        .expect(&format!("Field '{field}' can't parse to the f64"))
}
