use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use crossbeam_queue::ArrayQueue;
use uuid::Uuid;
use crate::core::api::{BalanceListener, BaseStrategy, MonitoringMessageListener, OrderListener, PriceTickerListener};
use crate::core::dto::{Balance, MonitoringMessage, Instrument, Order, OrderSide, PriceTicker, MonitoringEntity, MonitoringStatus, OrderStatus, DTO};
use crate::core::order_sizing::{chain_amount_quote, SizingConfig};
use crate::core::price_tickers_graph::ArbGraph;
use crate::core::utils::{round, time, RoundingMode};

pub struct ArbStrategy {
    graph: ArbGraph,
    next_check_ts: u128,
    fee: f64,

    sizing_config: SizingConfig,
    orders_direction: Vec<(Arc<Instrument>, OrderSide)>,

    out_queue: Arc<ArrayQueue<DTO>>,

    // management
    managements_entities_errored_ids: HashMap<MonitoringEntity, HashSet<usize>>,
}

impl ArbStrategy {
    pub fn new(out_queue: Arc<ArrayQueue<DTO>>) -> Self {

        let mut managements_entities_errored_ids = HashMap::new();
        managements_entities_errored_ids.insert(MonitoringEntity::PriceTicker, HashSet::new());
        managements_entities_errored_ids.insert(MonitoringEntity::OrderManagementSystem, HashSet::new());
        managements_entities_errored_ids.insert(MonitoringEntity::AccountUpdate, HashSet::new());
        let fee = 0.001;
        Self {
            graph: ArbGraph::new(fee),
            next_check_ts: 0,
            sizing_config: SizingConfig::new("USDT".to_string(), 20., 30.),
            orders_direction: vec![],
            managements_entities_errored_ids,
            out_queue,
            fee
        }
    }

    fn push_order(&mut self, order: Order) {
        self.out_queue.push(DTO::Order(order)).unwrap()
    }

    fn create_order_from_direction(&self, amount: f64, amount_quote: f64) -> Order {
        let mut order = Order::new();
        order.instrument = Arc::clone(&self.orders_direction[0].0);
        order.side = self.orders_direction[0].1.clone();
        order.amount = round(amount, order.instrument.amount_precision, RoundingMode::Down);
        order.amount_quote = round(amount_quote, order.instrument.price_precision, RoundingMode::Down);
        order.client_order_id = Uuid::new_v4().to_string();
        order
    }
}

impl OrderListener for ArbStrategy {
    fn on_order(&mut self, order: &Order) {
        log::info!("Order received {order:?}");
        if self.orders_direction.is_empty() {
            panic!("Unexpected order");
        }

        if !(order.instrument == self.orders_direction[0].0 && order.side == self.orders_direction[0].1) {
            panic!("Missmatch order");
        }

        match order.status {
            OrderStatus::Error => {
                panic!("Errored order status");
            },

            OrderStatus::Filled => {
                self.orders_direction.remove(0);
                if self.orders_direction.is_empty() {
                    log::info!("Filled orders directions");
                    return;
                }

                let (amount, amount_quote) = if order.instrument.base == self.orders_direction[0].0.base {
                    (order.amount, 0.)
                } else {
                    (0., order.amount)
                };

                self.push_order(
                    self.create_order_from_direction(amount, amount_quote)
                );
            }
            _ => {}
        };
    }
}

impl BalanceListener for ArbStrategy {
    fn on_balance(&mut self, _balance: &Balance) {}
}

impl MonitoringMessageListener for ArbStrategy {

    fn on_monitoring_message(&mut self, message: &MonitoringMessage) {
        log::info!("Monitoring message received {message:?}");
        let entities_ids = self.managements_entities_errored_ids.get_mut(&message.entity).unwrap();
        match message.status {
            MonitoringStatus::Ok => {
                entities_ids.remove(&message.entity_id);
            }
            MonitoringStatus::Error => {
                entities_ids.insert(message.entity_id.clone());

                match message.entity {
                    MonitoringEntity::PriceTicker => {
                        // unfortunately we should reset our graph and its dependencies
                        self.graph.reset();
                    },
                    _ => {}
                }
            }
        }
    }
}

impl PriceTickerListener for ArbStrategy {
    fn on_price_ticker(&mut self, price_ticker: &PriceTicker, tickers_map: &HashMap<Arc<Instrument>, PriceTicker>) {
        if !self.managements_entities_errored_ids[&MonitoringEntity::PriceTicker].is_empty() {
            return; // we have the broken price ticker stream, since we reset graph might be ok.
        }

        self.graph.update(price_ticker);

        if !self.managements_entities_errored_ids[&MonitoringEntity::OrderManagementSystem].is_empty() {
            return; // we have the broken OMS.
        }

        if self.next_check_ts > time() {
            return;
        }

        if self.orders_direction.is_empty() && self.graph.contains_currency_data(&self.sizing_config.currency) {
            if let Some(path) = self.graph.find_arb_path(&self.sizing_config.currency, true) {
                for window in path.windows(2) {
                    if let Some(dir) = self.graph.get_direction(&(window[0], window[1])) {
                        self.orders_direction.push(dir);
                    } else {
                        self.orders_direction.clear();
                        return;
                    }
                }

                if let Some(amount_quote) = chain_amount_quote(&self.sizing_config, tickers_map, &self.orders_direction, self.fee) {
                    // send first order
                    self.push_order(
                        self.create_order_from_direction(
                            0., amount_quote
                        )
                    );
                } else {
                    log::warn!("No initial order size");
                    self.orders_direction.clear();
                    return;
                }
            }

        }

        self.next_check_ts = time() + Duration::from_millis(5).as_nanos();
    }
}

impl BaseStrategy for ArbStrategy {}
