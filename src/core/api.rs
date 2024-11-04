use std::collections::HashMap;
use std::sync::Arc;
use crate::core::dto::{Balance, Exchange, Instrument, MonitoringMessage, Order, PriceTicker};

pub trait PriceTickerListener {
    fn on_price_ticker(&mut self, price_ticker: &PriceTicker, tickers_map: &HashMap<Exchange, HashMap<Arc<Instrument>, PriceTicker>>);
}


pub trait OrderListener {
    fn on_order(&mut self, order: &Order);
}


pub trait BalanceListener {
    fn on_balance(&mut self, order: &Balance);
}


pub trait MonitoringMessageListener {
    fn on_monitoring_message(&mut self, message: &MonitoringMessage);
}

pub trait BaseStrategy: PriceTickerListener + OrderListener + BalanceListener + MonitoringMessageListener {}
