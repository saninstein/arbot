use std::collections::HashMap;
use std::sync::Arc;
use crate::core::dto::{Exchange, MonitoringEntity, MonitoringMessage, MonitoringStatus};
use crate::{
    core::api::PriceTickerListener,
    core::dto::PriceTicker,
};
use crate::core::api::{BaseStrategy, MonitoringMessageListener};
use crate::core::dto::{Instrument};

pub struct PriceTickerFilter {
    pub listeners: Vec<Box<dyn BaseStrategy>>,
    tickers_map: HashMap<Exchange, HashMap<Arc<Instrument>, PriceTicker>>
}

impl PriceTickerFilter {
    pub fn new(listeners: Vec<Box<dyn BaseStrategy>>) -> Self {
        let mut tickers_map = HashMap::new();
        for exchange in Exchange::iterator() {
            tickers_map.insert((*exchange).clone(), HashMap::default());
        }
        Self { tickers_map, listeners }
    }

    fn update_and_notify_listeners(&mut self, price_ticker: &PriceTicker) {
        let exchange_tickers_map = self.tickers_map.get_mut(&price_ticker.instrument.exchange).unwrap();
        exchange_tickers_map.insert(Arc::clone(&price_ticker.instrument), price_ticker.copy());
        // let keys = self.tickers_map.len();
        // log::info!("Symbols: {keys}");
        for listener in self.listeners.iter_mut() {
            listener.on_price_ticker(price_ticker, &self.tickers_map);
        }
    }
}

impl PriceTickerListener for PriceTickerFilter {
    fn on_price_ticker(&mut self, price_ticker: &PriceTicker, _: &HashMap<Exchange, HashMap<Arc<Instrument>, PriceTicker>>) {
        match self.tickers_map.get(&price_ticker.instrument.exchange).unwrap().get(&price_ticker.instrument) {
            Some(p) => {
                if !p.is_prices_equals(price_ticker) {
                    self.update_and_notify_listeners(price_ticker)
                }
            }
            None => {
                self.update_and_notify_listeners(price_ticker)
            }
        }
    }
}

impl MonitoringMessageListener for PriceTickerFilter {
    fn on_monitoring_message(&mut self, message: &MonitoringMessage) {
        match message.entity {
            MonitoringEntity::PriceTicker => match message.status {
                MonitoringStatus::Error => self.tickers_map.clear(),
                _ => {}
            }
            _ => {}
        }
    }
}

#[allow(dead_code)]
struct DummyPriceTickerListener {}

impl PriceTickerListener for DummyPriceTickerListener {
    fn on_price_ticker(&mut self, price_ticker: &PriceTicker, _: &HashMap<Exchange, HashMap<Arc<Instrument>, PriceTicker>>) {
        log::info!("Dummy listener: {price_ticker:?}")
    }
}
