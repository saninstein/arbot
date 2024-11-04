use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::slice::Iter;
use std::sync::{Arc, LazyLock};
use crate::core::dto::Exchange::{Binance, Bit2me};

static BLANK_STR: LazyLock<String> = LazyLock::new(|| "".to_string());

static BLANK_INSTRUMENT: LazyLock<Arc<Instrument>> = LazyLock::new(|| Arc::new(Instrument {
    exchange: Exchange::Any,
    symbol: BLANK_STR.clone(),
    base: BLANK_STR.clone(),
    quote: BLANK_STR.clone(),
    amount_precision: 0,
    price_precision: 0,
    order_amount_max: 0.0,
    order_amount_min: 0.0,
    order_notional_min: 0.0,
    order_notional_max: 0.0,
    maker_fee: 0.0,
    taker_fee: 0.0,
}));


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
    pub fn effective_bid(&self, fee: f64) -> f64 {
        self.bid * (1.0 - fee)
    }

    pub fn effective_ask(&self, fee: f64) -> f64 {
        self.ask * (1.0 + fee)
    }

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

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub enum Exchange {
    Any,
    Binance,
    Bit2me
}

impl Exchange {
    pub fn iterator() -> Iter<'static, Exchange> {
        static EXCHANGES: [Exchange; 2] = [Binance, Bit2me];
        EXCHANGES.iter()
    }

    pub fn from_str(exchange: &str) -> Exchange {
        match exchange {
            "binance" => Exchange::Binance,
            "bit2me" => Exchange::Bit2me,
            "any" => Exchange::Any,
            _ => panic!("Unknown exchange: {exchange}")
        }
    }
}

#[derive(Debug)]
pub struct Instrument {
    pub exchange: Exchange,
    pub symbol: String,
    pub base: String,
    pub quote: String,
    pub amount_precision: usize,
    pub price_precision: usize,
    pub order_amount_max: f64,
    pub order_amount_min: f64,
    pub order_notional_min: f64,
    pub order_notional_max: f64,
    pub maker_fee: f64,
    pub taker_fee: f64
}

impl Hash for Instrument {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.symbol.hash(state);
    }
}

impl PartialEq for Instrument {
    fn eq(&self, other: &Self) -> bool {
        self.symbol == other.symbol
    }
}

impl Eq for Instrument {}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum OrderSide {
    Buy,
    Sell
}

#[derive(Debug, PartialEq, Eq)]
pub enum OrderStatus {
    // created but not yet sent to exchange
    Scheduled,
    // http request has been sent by EMS
    ScheduledSent,
    // registered by matching engine (http response after `create_order` api call)
    New,
    // order has been confirmed to be placed in the orderbook
    Open,
    PartiallyFilled,
    Filled,
    // CANCEL operation scheduled inside strategy
    Canceling,
    CancelingSent,
    // order canceled on exchange
    Canceled,
    Error
}

#[derive(Debug, PartialEq, Eq)]
pub enum OrderType {
    Market,
    Limit,
    LimitMaker
}

pub enum TimeInForce {
    GTC,  //  An order will be on the book unless the order is canceled.
    IOC,  //  An order will try to fill the order as much as it can before the order expires.
    FOK
}


#[derive(Debug)]
pub struct Order {
    pub timestamp: u128,
    pub instrument: Arc<Instrument>,
    pub exchange_order_id: String,
    pub client_order_id: String,
    pub order_type: OrderType,
    pub side: OrderSide,
    pub status: OrderStatus,
    pub price: f64,
    pub amount: f64,
    pub amount_quote: f64,
    pub amount_filled: f64,
    pub fees: Vec<(String, f64)>,
    pub error: String
}

impl Order {
    pub fn new() -> Self {
        Self {
            timestamp: 0,
            instrument: Arc::clone(&BLANK_INSTRUMENT),
            exchange_order_id: "".to_string(),
            client_order_id: "".to_string(),
            order_type: OrderType::Market,
            side: OrderSide::Buy,
            status: OrderStatus::Scheduled,
            price: 0.0,
            amount: 0.0,
            amount_quote: 0.0,
            amount_filled: 0.0,
            fees: vec![],
            error: "".to_string(),
        }
    }
}


#[derive(Debug)]
pub struct Balance {
    pub timestamp: u128,
    pub amounts: HashMap<String, (f64, f64)>,  // (free, locked)
}


impl Balance {
    pub fn new(timestamp: u128) -> Self {
        Self { timestamp, amounts: HashMap::new() }
    }
}


#[derive(Debug)]
pub enum MonitoringStatus {
    Ok,
    Error
}


#[derive(Debug, Eq, Hash, PartialEq)]
pub enum MonitoringEntity {
    PriceTicker,
    OrderManagementSystem,
    AccountUpdate
}


#[derive(Debug)]
pub struct MonitoringMessage {
    pub timestamp: u128,
    pub status: MonitoringStatus,
    pub entity: MonitoringEntity,
    pub entity_id: usize
}

impl MonitoringMessage {
    pub fn new(
        timestamp: u128,
        status: MonitoringStatus,
        entity: MonitoringEntity,
        entity_id: usize
    ) -> Self {
        Self {timestamp, status, entity, entity_id }
    }
}

#[derive(Debug)]
pub enum DTO {
    PriceTicker(PriceTicker),
    Order(Order),
    Balance(Balance),
    MonitoringMessage(MonitoringMessage)
}
