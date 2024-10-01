use crate::core::dto::PriceTicker;

pub trait PriceTickerListener {
    fn on_price_ticker(&mut self, price_ticker: &PriceTicker);
}
