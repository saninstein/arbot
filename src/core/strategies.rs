use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use petgraph::Graph;
use petgraph::graph::NodeIndex;
use petgraph::algo::find_negative_cycle;
use crate::core::api::{BalanceListener, BaseStrategy, MonitoringMessageListener, OrderListener, PriceTickerListener};
use crate::core::dto::{Balance, MonitoringMessage, Instrument, Order, OrderSide, PriceTicker, MonitoringEntity, MonitoringStatus};
use crate::core::oes::OrderExecutionSimulator;
use crate::core::utils::time;

pub struct ArbStrategy {
    graph: Graph::<String, f64>,
    edge_to_order_direction_map: HashMap<(NodeIndex, NodeIndex), (Arc<Instrument>, OrderSide)>,

    symbol_to_node_map: HashMap<String, NodeIndex>,
    node_to_symbol_map: HashMap<NodeIndex, String>,

    next_check_ts: u128,
    fee: f64,

    oes: OrderExecutionSimulator,

    // management
    managements_entities_errored_ids: HashMap<MonitoringEntity, HashSet<usize>>,
}

impl ArbStrategy {
    pub fn new() -> Self {

        let mut managements_entities_errored_ids = HashMap::new();
        managements_entities_errored_ids.insert(MonitoringEntity::PriceTicker, HashSet::new());
        managements_entities_errored_ids.insert(MonitoringEntity::OrderManagementSystem, HashSet::new());
        managements_entities_errored_ids.insert(MonitoringEntity::AccountUpdate, HashSet::new());

        Self {
            graph: Graph::new(),
            symbol_to_node_map: Default::default(),
            node_to_symbol_map: Default::default(),
            edge_to_order_direction_map: Default::default(),
            next_check_ts: 0,
            fee: 0.001, // 0.1% trading fee
            oes: OrderExecutionSimulator::new(Default::default(), 0.001),
            managements_entities_errored_ids
        }
    }

    pub fn get_node_by_symbol(&mut self, symbol: String) -> NodeIndex {
        match self.symbol_to_node_map.get(&symbol as &str) {
            Some(node) => {
                node.clone()
            }
            _ => {
                let node = self.graph.add_node(symbol.clone());
                self.symbol_to_node_map.insert(symbol.clone(), node.clone());
                self.node_to_symbol_map.insert(node, symbol);
                node.clone()
            }
        }
    }

    pub fn get_nodes_by_instrument(&mut self, instrument: &Arc<Instrument>) -> (NodeIndex, NodeIndex) {
        let base = self.get_node_by_symbol(instrument.base.clone());
        let quote = self.get_node_by_symbol(instrument.quote.clone());

        let edge = &(base.clone(), quote.clone());

        if !self.edge_to_order_direction_map.contains_key(edge) {
            self.edge_to_order_direction_map.insert(
                edge.clone(),
                (Arc::clone(instrument), OrderSide::Sell),
            );
            self.edge_to_order_direction_map.insert(
                (edge.1, edge.0).clone(),
                (Arc::clone(instrument), OrderSide::Buy),
            );
        }

        return (base, quote);
    }

    pub fn calculate_path_profit(&self, path: &Vec<NodeIndex>) -> f64 {
        let mut cycle_sum = 0.0;
        for window in path.windows(2) {
            let (u, v) = (window[0], window[1]);
            if let Some(edge) = self.graph.find_edge(u, v) {
                cycle_sum += self.graph.edge_weight(edge).unwrap();
            }
        }

        return (1.0 - cycle_sum.exp()) * 100.0;
    }

    pub fn find_arb_path(&mut self, symbol: &str, tickers_map: &HashMap<Arc<Instrument>, PriceTicker>) {
        let node_id = self.symbol_to_node_map.get(symbol).unwrap();
        match find_negative_cycle(&self.graph, node_id.clone()) {
            Some(vec) => {
                if vec.contains(node_id) {
                    let pos = vec.iter().position(|x| x == node_id).unwrap();
                    let mut path = Vec::new();

                    path.extend_from_slice(&vec[pos..]);
                    path.extend_from_slice(&vec[..pos]);
                    path.push(node_id.clone());

                    let res = path.iter().map(|&node_index| { self.node_to_symbol_map.get(&node_index).unwrap().as_str() }).collect::<Vec<&str>>().join("->");
                    let profit = self.calculate_path_profit(&path);
                    log::info!("Arb found {res} profit {profit}%");

                    // execute
                    // let mut orders_directions = vec![];
                    // for window in path.windows(2) {
                    //     let (base_node, quote_node) = (window[0], window[1]);
                    //     let dir = self.edge_to_order_direction_map.get(&(base_node, quote_node)).unwrap();
                    //     orders_directions.push(dir);
                    // }
                    //
                    // self.oes.balances.clear();
                    // self.oes.balances.insert("USDT".to_string(), 10_000.);
                    // self.oes.execute_orders_chain(orders_directions, tickers_map);
                    //
                    // let balances = &self.oes.balances;
                    // log::info!("Balance after execution {balances:?}");
                }
            }
            _ => {
                // println!("Negative cycle not found for {symbol}");
            }
        }
    }
}

impl OrderListener for ArbStrategy {
    fn on_order(&mut self, order: &Order) {

    }
}

impl BalanceListener for ArbStrategy {
    fn on_balance(&mut self, balance: &Balance) {
    }
}

impl MonitoringMessageListener for ArbStrategy {

    fn on_monitoring_message(&mut self, message: &MonitoringMessage) {
        let mut entities_ids = self.managements_entities_errored_ids.get_mut(&message.entity).unwrap();
        match message.status {
            MonitoringStatus::Ok => {
                entities_ids.remove(&message.entity_id);
            }
            MonitoringStatus::Error => {
                entities_ids.insert(message.entity_id.clone());

                match message.entity {
                    MonitoringEntity::PriceTicker => {
                        // unfortunately we should reset our graph and its dependencies
                        if self.graph.node_count() > 0 {
                            self.graph.clear();
                            self.edge_to_order_direction_map.clear();
                            self.symbol_to_node_map.clear();
                            self.node_to_symbol_map.clear();
                        }
                    }
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

        let (base, quote) = self.get_nodes_by_instrument(&price_ticker.instrument);

        self.graph.update_edge(base, quote, -price_ticker.effective_bid(self.fee).ln());
        self.graph.update_edge(quote, base, -(1. / price_ticker.effective_ask(self.fee)).ln());

        if self.next_check_ts > time() {
            return;
        }

        // let mut ts = time();

        // for symbol in vec!["USDT", "USDC", "DAI", "BNB", "FDUSD"] {
        for symbol in vec!["USDT"] {
            if self.symbol_to_node_map.contains_key(symbol) {
                self.find_arb_path(symbol, tickers_map);
            }
        }

        // ts = time() - ts;
        // log::info!("Calculated: {ts}");

        self.next_check_ts = time() + Duration::from_millis(5).as_nanos();

        // println!("{}", Dot::new(&self.graph));
    }
}

impl BaseStrategy for ArbStrategy {}
