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

    fn update(&mut self, price_ticker: &PriceTicker) -> bool {
        let mut result = true;
        let exchange_tickers_map = self.tickers_map.get_mut(&price_ticker.instrument.exchange).unwrap();
        match exchange_tickers_map.get(&price_ticker.instrument) {
            Some(p) => {
                result = !p.is_prices_equals(price_ticker);
                exchange_tickers_map.get_mut(&price_ticker.instrument).unwrap().update(price_ticker);
            }
            None => {
                exchange_tickers_map.insert(Arc::clone(&price_ticker.instrument), price_ticker.copy());
            }
        };
        result
    }
}

impl PriceTickerListener for PriceTickerFilter {
    fn on_price_ticker(&mut self, price_ticker: &PriceTicker, _: &HashMap<Exchange, HashMap<Arc<Instrument>, PriceTicker>>) {
        if self.update(price_ticker) {
            for listener in self.listeners.iter_mut() {
                listener.on_price_ticker(price_ticker, &self.tickers_map);
            }
        }
    }
}

impl MonitoringMessageListener for PriceTickerFilter {
    fn on_monitoring_message(&mut self, message: &MonitoringMessage) {
        match message.entity {
            MonitoringEntity::PriceTicker => match message.status {
                MonitoringStatus::Error => {
                    self.tickers_map.clear();
                    for exchange in Exchange::iterator() {
                        self.tickers_map.insert((*exchange).clone(), HashMap::default());
                    }
                },
                _ => {}
            }
            _ => {}
        }
    }
}
