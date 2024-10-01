use std::collections::HashMap;
use log;
use petgraph::Graph;
use petgraph::graph::NodeIndex;
use petgraph::algo::find_negative_cycle;
use crate::{
    core::api::PriceTickerListener,
    core::dto::PriceTicker,
};
use crate::core::utils::time;

pub struct PriceTickerFilter {
    listeners: Vec<Box<dyn PriceTickerListener>>,
    latest_map: HashMap<String, PriceTicker>,
}

impl PriceTickerFilter {
    pub(crate) fn new(listeners: Vec<Box<dyn PriceTickerListener>>) -> Self {
        Self {
            listeners,
            latest_map: HashMap::new(),
        }
    }

    fn update_and_notify_listeners(&mut self, price_ticker: &PriceTicker) {
        self.latest_map.insert(price_ticker.instrument.symbol.clone(), price_ticker.copy());
        let keys = self.latest_map.len();
        // log::info!("Symbols: {keys}");
        for listener in self.listeners.iter_mut() {
            listener.on_price_ticker(price_ticker);
        }
    }
}

impl PriceTickerListener for PriceTickerFilter {
    fn on_price_ticker(&mut self, price_ticker: &PriceTicker) {
        match self.latest_map.get(&price_ticker.instrument.symbol) {
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

struct DummyPriceTickerListener {}

impl PriceTickerListener for DummyPriceTickerListener {
    fn on_price_ticker(&mut self, price_ticker: &PriceTicker) {
        // log::info!("Dummy listener: {price_ticker:?}")
    }
}

pub struct ArbStatPriceTickerListener {
    graph: Graph::<String, f64>,
    symbol_to_node_map: HashMap<String, NodeIndex>,
    node_to_symbol_map: HashMap<NodeIndex, String>,
    next_check_ts: u128,
    fee: f64
}

impl ArbStatPriceTickerListener {
    pub fn new() -> Self {
        Self {
            graph: Graph::new(),
            symbol_to_node_map: Default::default(),
            node_to_symbol_map: Default::default(),
            next_check_ts: 0,
            fee: 0.001 // 0.1% trading fee
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

    pub fn calculate_path_profit(&self, path: &Vec<NodeIndex>) -> f64 {
        let mut cycle_sum = 0.0;
        for i in 0..path.len() {
            let u = path[i];
            let v = path[(i + 1) % path.len()];
            if let Some(edge) = self.graph.find_edge(u, v) {
                cycle_sum += self.graph.edge_weight(edge).unwrap();
            }
        }

        return (1.0 - cycle_sum.exp()) * 100.0;;
    }

    pub fn find_arb_path(&self, symbol: &str) {
        let node_id = self.symbol_to_node_map.get(symbol).unwrap();
        match find_negative_cycle(&self.graph, node_id.clone()) {
            Some(mut vec) => {
                if vec.last().unwrap() == node_id {
                    let mut path = vec![node_id.clone()];
                    path.append(&mut vec);

                    let res = path.iter().map(|&node_index| { self.node_to_symbol_map.get(&node_index).unwrap().as_str() }).collect::<Vec<&str>>().join("->");
                    let profit = self.calculate_path_profit(&vec);
                    log::info!("Negative cycle found {res} profit {profit}%");
                }
            }
            _ => {
                // println!("Negative cycle not found for {symbol}");
            }
        }
    }
}

impl PriceTickerListener for ArbStatPriceTickerListener {
    fn on_price_ticker(&mut self, price_ticker: &PriceTicker) {
        let base = self.get_node_by_symbol(price_ticker.instrument.base.clone());
        let quote = self.get_node_by_symbol(price_ticker.instrument.quote.clone());

        let effective_bid = price_ticker.bid * (1.0 - self.fee); // Adjust bid price (selling base)
        let effective_ask = price_ticker.ask * (1.0 + self.fee); // Adjust ask price (buying base)

        self.graph.update_edge(base, quote, -effective_bid.ln());
        self.graph.update_edge(quote, base, -(1. / effective_ask).ln());

        if self.next_check_ts > time() {
            return;
        }

        let mut ts = time();

        // TODO: arb oportunity persistance

        for symbol in vec!["USDT", "USDC", "DAI", "BNB", "FDUSD"] {
            if self.symbol_to_node_map.contains_key(symbol) {
                self.find_arb_path(symbol);
            }
        }

        ts = time() - ts;
        // log::info!("Calculated: {ts}");

        self.next_check_ts = time() + 5;

        // println!("{}", Dot::new(&self.graph));
    }
}
