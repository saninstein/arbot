use std::collections::HashMap;
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

#[derive(Eq, Hash, PartialEq, Debug)]
pub struct Instrument {
    pub symbol: String,
    pub base: String,
    pub quote: String,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum OrderSide {
    BUY, SELL
}

#[derive(Debug, PartialEq, Eq)]
pub enum OrderStatus {
    // created but not yet sent to exchange
    SCHEDULED,
    // http request has been sent by EMS
    SCHEDULED_SENT,
    // registered by matching engine (http response after `create_order` api call)
    NEW,
    // order has been confirmed to be placed in the orderbook
    OPEN,
    PARTIALLY_FILLED,
    FILLED,
    // CANCEL operation scheduled inside strategy
    CANCELING,
    CANCELING_SENT,
    // order canceled on exchange
    CANCELED,
    ERROR
}

#[derive(Debug, PartialEq, Eq)]
pub enum OrderType {
    MARKET,
    LIMIT,
    LIMIT_MAKER
}

pub enum TimeInForce {
    GTC,  //  An order will be on the book unless the order is canceled.
    IOC,  //  An order will try to fill the order as much as it can before the order expires.
    FOK
}


#[derive(Debug)]
pub struct Order {
    pub instrument: Arc<Instrument>,
    pub order_type: OrderType,
    pub side: OrderSide,
    pub status: OrderStatus,
    pub price: f64,
    pub amount: f64,
    pub amount_quote: f64,
    pub amount_filled: f64,
}

impl Order {
    pub fn new(
        instrument: Arc<Instrument>,
        order_type: OrderType,
        side: OrderSide,
        status: OrderStatus,
        price: f64,
        amount: f64,
        amount_quote: f64,
        amount_filled: f64
    ) -> Self {
        Self {
            instrument,
            order_type,
            side,
            status,
            price,
            amount,
            amount_quote,
            amount_filled,
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
    OK,
    ERROR
}


#[derive(Debug, Eq, Hash, PartialEq)]
pub enum MonitoringEntity {
    PRICE_TICKER, ORDER_MANAGEMENT_SYSTEM, ACCOUNT_UPDATE
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
