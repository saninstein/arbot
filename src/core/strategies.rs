use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use crossbeam_queue::ArrayQueue;
use uuid::Uuid;
use crate::core::api::{BalanceListener, BaseStrategy, MonitoringMessageListener, OrderListener, PriceTickerListener};
use crate::core::dto::{Balance, MonitoringMessage, Instrument, Order, OrderSide, PriceTicker, MonitoringEntity, MonitoringStatus, OrderStatus, DTO, Exchange};
use crate::core::order_sizing::{chain_amount_quote, SizingConfig};
use crate::core::price_tickers_graph::ArbGraph;
use crate::core::utils::{round, time, RoundingMode};

pub struct ArbStrategy {
    exchange: Exchange,
    graph: ArbGraph,
    next_check_ts: u128,

    sizing_config: SizingConfig,
    orders_direction: Vec<(Arc<Instrument>, OrderSide)>,
    tickers: HashMap<Arc<Instrument>, PriceTicker>,
    monitoring_only: bool,
    out_queue: Arc<ArrayQueue<DTO>>,

    // cooldown
    skips_until_cooldown: usize,
    skips_in_a_row: usize,
    cooldown_duration: Duration,

    // management
    managements_entities_errored_ids: HashMap<MonitoringEntity, HashSet<usize>>,
}

impl ArbStrategy {
    pub fn new(out_queue: Arc<ArrayQueue<DTO>>, exchange: Exchange, sizing_config: SizingConfig, monitoring_only: bool) -> Self {

        let mut managements_entities_errored_ids = HashMap::new();
        managements_entities_errored_ids.insert(MonitoringEntity::PriceTicker, HashSet::new());
        managements_entities_errored_ids.insert(MonitoringEntity::OrderManagementSystem, HashSet::new());
        managements_entities_errored_ids.insert(MonitoringEntity::AccountUpdate, HashSet::new());
        Self {
            graph: ArbGraph::new(),
            next_check_ts: 0,
            orders_direction: vec![],
            managements_entities_errored_ids,
            out_queue,
            skips_until_cooldown: 3,
            skips_in_a_row: 0,
            exchange,
            monitoring_only,
            sizing_config,
            cooldown_duration: Duration::from_millis(5),
            tickers: HashMap::default(),
        }
    }

    fn push_order(&mut self, order: Order) {
        self.out_queue.push(DTO::Order(order)).unwrap()
    }

    fn create_order_from_direction(&self, mut amount: f64, amount_quote: f64) -> Order {
        let mut order = Order::new();
        order.instrument = Arc::clone(&self.orders_direction[0].0);
        order.side = self.orders_direction[0].1.clone();
        if amount == 0. {
            let price_ticker = self.tickers.get(&order.instrument).unwrap();
            amount = amount_quote / if order.side == OrderSide::Buy {
                price_ticker.ask
            } else {
                price_ticker.bid
            };
        }
        order.amount = round(amount, order.instrument.amount_precision, RoundingMode::Down);
        order.client_order_id = Uuid::new_v4().to_string();
        order
    }

    fn skip(&mut self) {
        if self.skips_in_a_row == self.skips_until_cooldown {
            log::warn!("Skip triggered");
            self.skips_in_a_row = 0;
            self.cooldown_with_duration(Duration::from_secs(3));
        } else {
            self.skips_in_a_row += 1;
        }
    }

    fn cooldown(&mut self) {
        self.next_check_ts = time() + self.cooldown_duration.as_nanos();
    }

    fn cooldown_with_duration(&mut self, duration: Duration) {
        self.next_check_ts = time() + duration.as_nanos();
    }

}

impl OrderListener for ArbStrategy {
    fn on_order(&mut self, order: &Order) {
        if self.monitoring_only {
            return;
        }
        log::info!("Order received {order:?}");
        if self.orders_direction.is_empty() {
            log::warn!("Unexpected order");
            return;
        }

        if !(order.instrument == self.orders_direction[0].0 && order.side == self.orders_direction[0].1) {
            panic!("Missmatch order");
        }

        match &order.status {
            OrderStatus::Error => {
                panic!("Errored order status");
            },

            OrderStatus::Filled => {
                self.orders_direction.remove(0);
                if self.orders_direction.is_empty() {
                    log::info!("Filled orders directions");
                    // panic!("The good one.");
                    return;
                }

                let balance = order.balance().expect("Should be balance");

                let (amount, amount_quote) = if balance.0 == self.orders_direction[0].0.base {
                    (balance.1, 0.)
                } else if balance.0 == self.orders_direction[0].0.quote {
                    (0., balance.1)
                } else {
                    panic!("Invalid sequence: {:?}", self.orders_direction[0]);
                };

                self.push_order(
                    self.create_order_from_direction(amount, amount_quote)
                );
            }
            t => {
                log::info!("Unexpected order status: {t:?}");
            }
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
                entities_ids.insert(message.entity_id);

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
    fn on_price_ticker(&mut self, price_ticker: &PriceTicker, tickers_map: &HashMap<Exchange, HashMap<Arc<Instrument>, PriceTicker>>) {
        if price_ticker.instrument.exchange != self.exchange {
            return; // single exchange strategy
        }
        let tickers_map = tickers_map.get(&self.exchange).unwrap();
        if !self.managements_entities_errored_ids[&MonitoringEntity::PriceTicker].is_empty() {
            return; // we have the broken price ticker stream, since we reset graph might be ok.
        }
        let price_ticker = tickers_map.get(&price_ticker.instrument).unwrap();

        self.graph.update(price_ticker);

        if !self.managements_entities_errored_ids[&MonitoringEntity::OrderManagementSystem].is_empty() {
            return; // we have the broken OMS.
        }

        if !self.orders_direction.is_empty() {  // processing of the path
            if self.tickers.contains_key(&price_ticker.instrument) {
                self.tickers.get_mut(&price_ticker.instrument).unwrap().update(price_ticker);
            }
            return;
        }

        if self.next_check_ts > time() {
            return;
        }

        if self.graph.contains_currency_data(&self.sizing_config.currency) {
            if let Some(path) = self.graph.find_arb_path(&self.sizing_config.currency, true) {
                for window in path.windows(2) {
                    if let Some(dir) = self.graph.get_direction(&(window[0], window[1])) {
                        self.orders_direction.push(dir);
                    } else {
                        self.orders_direction.clear();
                        self.skip();
                        return;
                    }
                }

                if let Some(enter_amount) = chain_amount_quote(&self.sizing_config, tickers_map, &self.orders_direction) {
                    // send first order
                    if !self.monitoring_only {
                        for (instrument, _) in &self.orders_direction {
                            self.tickers.insert(
                                Arc::clone(instrument), tickers_map.get(instrument).unwrap().copy()
                            );
                        }

                        let enter_order;
                        if &self.orders_direction[0].0.base.to_uppercase() == &self.sizing_config.currency.to_uppercase() {
                            enter_order = self.create_order_from_direction(enter_amount, 0.);
                        } else {
                            enter_order = self.create_order_from_direction(0., enter_amount);
                        }
                        self.push_order(enter_order);
                    }
                    self.skips_in_a_row = 0;
                } else {
                    log::warn!("No initial order size");
                    self.orders_direction.clear();
                    self.skip();
                    return;
                }

                if self.monitoring_only {
                    self.orders_direction.clear();
                }
            }
            self.cooldown();
        }
    }
}

impl BaseStrategy for ArbStrategy {}
