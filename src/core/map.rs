use std::collections::HashMap;
use std::sync::Arc;
use crate::core::dto::Instrument;

#[derive(Debug)]
pub struct InstrumentsMap {
    pub map: HashMap<String, Arc<Instrument>>,
}

impl InstrumentsMap {
    pub fn from_array_string(symbols: Vec<String>) -> Self {
        let mut map = HashMap::new();

        for s in symbols {
            let (base, quote) = s.split_once('/').unwrap();
            let symbol = s.replace("/", "");

            map.insert(
                symbol.clone(),
                Arc::new(Instrument {
                    symbol: symbol.clone(),
                    base: base.to_string(),
                    quote: quote.to_string(),
                }),
            );
        }

        Self { map }
    }
}
