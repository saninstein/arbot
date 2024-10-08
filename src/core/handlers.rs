use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use petgraph::Graph;
use petgraph::graph::NodeIndex;
use petgraph::algo::find_negative_cycle;
use crate::core::dto::{MonitoringEntity, MonitoringMessage, MonitoringStatus, OrderSide, OrderType};
use crate::{
    core::api::PriceTickerListener,
    core::dto::PriceTicker,
    core::utils::time,
};
use crate::core::api::{BaseStrategy, MonitoringMessageListener};
use crate::core::dto::{Instrument, Order, OrderStatus};
use crate::core::oes::OrderExecutionSimulator;

pub struct PriceTickerFilter {
    pub listeners: Vec<Box<dyn BaseStrategy>>,
    tickers_map: HashMap<Arc<Instrument>, PriceTicker>
}

impl PriceTickerFilter {
    pub fn new(listeners: Vec<Box<dyn BaseStrategy>>) -> Self {
        Self { tickers_map: Default::default(), listeners }
    }

    fn update_and_notify_listeners(&mut self, price_ticker: &PriceTicker) {
        self.tickers_map.insert(Arc::clone(&price_ticker.instrument), price_ticker.copy());
        // let keys = self.tickers_map.len();
        // log::info!("Symbols: {keys}");
        for listener in self.listeners.iter_mut() {
            listener.on_price_ticker(price_ticker, &self.tickers_map);
        }
    }
}

impl PriceTickerListener for PriceTickerFilter {
    fn on_price_ticker(&mut self, price_ticker: &PriceTicker, _: &HashMap<Arc<Instrument>, PriceTicker>) {
        match self.tickers_map.get(&price_ticker.instrument) {
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
            MonitoringEntity::PRICE_TICKER => match message.status {
                MonitoringStatus::ERROR => self.tickers_map.clear(),
                _ => {}
            }
            _ => {}
        }
    }
}

#[allow(dead_code)]
struct DummyPriceTickerListener {}

impl PriceTickerListener for DummyPriceTickerListener {
    fn on_price_ticker(&mut self, price_ticker: &PriceTicker, _: &HashMap<Arc<Instrument>, PriceTicker>) {
        log::info!("Dummy listener: {price_ticker:?}")
    }
}
