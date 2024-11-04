use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use crate::core::dto::{Exchange, Instrument};

#[derive(Debug)]
pub struct InstrumentsMap {
    pub map: HashMap<Exchange, HashMap<String, Arc<Instrument>>>,
}

impl InstrumentsMap {
    pub fn from_json(path: &str) -> Self {
        let raw = fs::read_to_string(path).expect("Failed to read tickers file");
        let data = json::parse(&raw).expect("Failed to parse tickers");

        let mut map = HashMap::new();

        for exchange in Exchange::iterator() {
            map.insert((*exchange).clone(), HashMap::default());
        }

        for raw in data.members() {
            let exchange = Exchange::from_str(raw["exchange"].as_str().unwrap());
            let exchange_map = map.get_mut(&exchange).unwrap();

            let symbol = raw["symbol"].as_str().unwrap().to_string();
            let base = raw["base"].as_str().unwrap().to_string();
            let quote = raw["quote"].as_str().unwrap().to_string();
            let instrument = Arc::new(Instrument {
                exchange: exchange,
                symbol: symbol.clone(),
                base: base.clone(),
                quote: quote.clone(),
                amount_precision: raw["amount_precision"].as_usize().unwrap(),
                price_precision: raw["price_precision"].as_usize().unwrap(),
                order_amount_max: raw["order_amount_max"].as_f64().unwrap(),
                order_amount_min: raw["order_amount_min"].as_f64().unwrap(),
                order_notional_min: raw["order_notional_min"].as_f64().unwrap(),
                order_notional_max: raw["order_notional_max"].as_f64().unwrap(),
                maker_fee: raw["maker_fee"].as_f64().unwrap(),
                taker_fee: raw["taker_fee"].as_f64().unwrap(),
            });
            exchange_map.insert(symbol.clone(), Arc::clone(&instrument));
            exchange_map.insert(format!("{base}/{quote}"), Arc::clone(&instrument));
            exchange_map.insert(format!("{base}{quote}"), Arc::clone(&instrument));
            exchange_map.insert(format!("{base}/{quote}").to_lowercase(), Arc::clone(&instrument));
            exchange_map.insert(format!("{base}{quote}").to_lowercase(), Arc::clone(&instrument));
        }
        Self { map }
    }

    pub fn get(&self, exchange: &Exchange, symbol: &str) -> Option<&Arc<Instrument>> {
        self.map.get(exchange)?.get(symbol)
    }
}
