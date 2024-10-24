use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use crate::core::dto::Instrument;

#[derive(Debug)]
pub struct InstrumentsMap {
    pub map: HashMap<String, Arc<Instrument>>,
}

impl InstrumentsMap {
    pub fn from_json(path: &str) -> Self {
        let raw = fs::read_to_string(path).expect("Failed to read tickers file");
        let data = json::parse(&raw).expect("Failed to parse tickers");

        let mut map = HashMap::new();
        for raw in data.members() {
            let symbol = raw["symbol"].as_str().unwrap().to_string();
            map.insert(
                symbol.clone(),
                Arc::new(Instrument {
                    exchange: "BINANCE".to_string(),
                    symbol: symbol,
                    base: raw["base"].as_str().unwrap().to_string(),
                    quote: raw["quote"].as_str().unwrap().to_string(),
                    amount_precision: raw["amount_precision"].as_usize().unwrap(),
                    price_precision: raw["price_precision"].as_usize().unwrap(),
                    order_amount_max: raw["order_amount_max"].as_f64().unwrap(),
                    order_amount_min: raw["order_amount_min"].as_f64().unwrap(),
                    order_notional_min: raw["order_notional_min"].as_f64().unwrap(),
                    order_notional_max: raw["order_notional_max"].as_f64().unwrap(),
                }),
            );
        }
        Self { map }
    }
}
