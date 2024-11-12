use std::collections::HashMap;
use std::sync::Arc;
use crate::core::dto::{Instrument, OrderSide, PriceTicker};

#[derive(Debug, Clone)]
pub struct SizingConfig {
    pub currency: String,
    pub min_order_size: f64,
    pub max_order_size: f64
}

impl SizingConfig {
    pub fn new(currency: String, min_order_size: f64, max_order_size: f64) -> Self {
        Self {currency, min_order_size, max_order_size}
    }

    pub fn adjust_value(&self, value: f64) -> Option<f64> {
        if value < self.min_order_size {
            return None;
        }

        if value > self.max_order_size {
            return Some(self.min_order_size);
        }

        Some(value)
    }
}


pub fn max_chain_amount_quote(
    tickers_map: &HashMap<Arc<Instrument>, PriceTicker>,
    orders_direction: &Vec<(Arc<Instrument>, OrderSide)>
) -> Option<f64> {
    // Initialize the maximum order size to a very large value

    let (instrument, side) = &orders_direction[orders_direction.len() - 1];
    let ticker = tickers_map.get(instrument)?;
    let mut amount_quote = match side {
        OrderSide::Buy => {
            ticker.ask_amount * ticker.effective_ask(instrument.taker_fee)
        },
        OrderSide::Sell => {
            ticker.bid_amount
        },
    };
    log::debug!("Amount quote for {instrument:?}: {amount_quote:?}");
    if amount_quote < instrument.order_notional_min {
        log::warn!("Notional filter error {:?} for {:?}", instrument, amount_quote);
        return None;
    }

    let mut prev_instrument = instrument;

    // Traverse the chain in reverse
    for (instrument, side) in orders_direction.iter().rev().skip(1) {
        let ticker = tickers_map.get(instrument)?;

        match side {
            OrderSide::Buy => {
                amount_quote = amount_quote.min(ticker.ask_amount) * ticker.effective_ask(instrument.taker_fee);
            },
            OrderSide::Sell => {
                if prev_instrument.base != instrument.base {
                    amount_quote /= ticker.effective_bid(instrument.taker_fee);
                }
                amount_quote = amount_quote.min(ticker.bid_amount);
            },
        };

        prev_instrument = instrument;
        log::debug!("Amount quote for {instrument:?}: {amount_quote:?}");
        if amount_quote < instrument.order_notional_min {
            log::warn!("Notional filter error {:?} for {:?}", instrument, amount_quote);
            return None;
        }
    }
    Some(amount_quote)
}

pub fn chain_amount_quote(
    sizing_config: &SizingConfig,
    tickers_map: &HashMap<Arc<Instrument>, PriceTicker>,
    orders_direction: &Vec<(Arc<Instrument>, OrderSide)>,
) -> Option<f64> {
    let amount_quote = max_chain_amount_quote(tickers_map, orders_direction)?;
    log::info!("max_chain_amount_quote: {amount_quote}");
    sizing_config.adjust_value(amount_quote)
}
