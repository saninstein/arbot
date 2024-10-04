use std::sync::Arc;

#[derive(Debug)]
pub struct PriceTicker {
    pub timestamp: u128,
    pub instrument: Arc<Instrument>,
    pub bid: f64,
    pub bid_amount: f64,
    pub ask: f64,
    pub ask_amount: f64,
}

impl PriceTicker {
    pub fn is_prices_equals(&self, other: &Self) -> bool {
        other.bid == self.bid && other.ask == self.ask
    }

    pub fn copy(&self) -> Self {
        Self {
            timestamp: self.timestamp,
            instrument: Arc::clone(&self.instrument),
            bid: self.bid,
            bid_amount: self.bid_amount,
            ask: self.ask,
            ask_amount: self.ask_amount,
        }
    }
}

#[derive(Eq, Hash, PartialEq, Debug)]
pub struct Instrument {
    pub symbol: String,
    pub base: String,
    pub quote: String,
}
