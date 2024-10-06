use std::{fs, thread};
use json::JsonValue;
use std::time::SystemTime;
use std::io::Write;

pub fn time() -> u128 {
    match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(n) => n.as_nanos(),
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

pub fn init_logger() {
    env_logger::builder()
        .format(|buf, record| {
            let ts = buf.timestamp_millis();
            let style = buf.default_level_style(record.level());
            let level = record.level();
            let current_thread = thread::current();
            let thread_name = current_thread.name().unwrap_or("");
            let target = record.target();
            let args = record.args();
            writeln!(
                buf,
                "[{ts} {style}{level}{style:#} {thread_name}] {target}] {args}",
            )
        })
        .init();
}

pub fn read_tickers(path: String) -> Vec<Vec<String>> {
    let raw = fs::read_to_string(path).expect("Failed to read tickers file");
    let data = json::parse(&raw).expect("Failed to parse tickers");
    let mut result = Vec::new();

    for group in data.members() {
        let mut v = Vec::new();
        for item in group.members() {
            v.push(item.to_string());
        }
        result.push(v);
    }
    result
}
