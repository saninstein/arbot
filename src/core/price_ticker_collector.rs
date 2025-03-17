use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;
use csv::Writer;
use crate::core::api::{BalanceListener, BaseStrategy, MonitoringMessageListener, OrderListener, PriceTickerListener};
use crate::core::dto::{Balance, Exchange, Instrument, MonitoringMessage, Order, PriceTicker};

pub struct PriceTickerCollector {
    wtr: Writer<File>,
    rows_counter: usize
}

impl PriceTickerCollector {
    pub fn new(path: &str) -> Self {
        Self {
            wtr: csv::Writer::from_path(Path::new(path)).unwrap(),
            rows_counter: 0
        }
    }
}

impl PriceTickerListener for PriceTickerCollector {
    fn on_price_ticker(&mut self, price_ticker: &PriceTicker, tickers_map: &HashMap<Exchange, HashMap<Arc<Instrument>, PriceTicker>>) {
        let price_ticker = tickers_map.get(&price_ticker.instrument.exchange).unwrap().get(&price_ticker.instrument).unwrap();
        self.wtr.write_record(
            &[
                price_ticker.instrument.exchange.as_str().clone(), &price_ticker.instrument.symbol,
                &price_ticker.timestamp.to_string(),
                &price_ticker.bid.to_string(),
                &price_ticker.bid_amount.to_string(),
                &price_ticker.ask.to_string(),
                &price_ticker.ask_amount.to_string(),
            ],
        ).unwrap();
        self.rows_counter += 1;
        if self.rows_counter == 10_000 {
            log::info!("Flush");
            self.wtr.flush().unwrap();
            self.rows_counter = 0;
        }
    }
}

impl OrderListener for PriceTickerCollector {
    fn on_order(&mut self, order: &Order) {
    }
}

impl MonitoringMessageListener for PriceTickerCollector {
    fn on_monitoring_message(&mut self, message: &MonitoringMessage) {
    }
}

impl BalanceListener for PriceTickerCollector {
    fn on_balance(&mut self, order: &Balance) {
    }
}

impl BaseStrategy for PriceTickerCollector {

}
