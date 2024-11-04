use std::collections::{HashMap};
use std::sync::Arc;
use petgraph::Graph;
use petgraph::graph::NodeIndex;
use petgraph::algo::find_negative_cycle;
use crate::core::dto::{Exchange, Instrument, OrderSide, PriceTicker};


pub struct ArbGraph {
    graph: Graph<String, f64>,
    edge_to_order_direction_map: HashMap<(NodeIndex, NodeIndex), (Arc<Instrument>, OrderSide)>,

    symbol_to_node_map: HashMap<String, NodeIndex>,
    node_to_symbol_map: HashMap<NodeIndex, String>,
}

impl ArbGraph {
    pub fn new() -> Self {

        Self {
            graph: Graph::new(),
            symbol_to_node_map: Default::default(),
            node_to_symbol_map: Default::default(),
            edge_to_order_direction_map: Default::default(),
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


    pub fn find_arb_path(&mut self, currency: &str, verbose: bool) -> Option<Vec<NodeIndex>> {
        let node_id = self.symbol_to_node_map.get(currency)?;
        let vec = find_negative_cycle(&self.graph, node_id.clone())?;
        if !vec.contains(node_id) {
            return None;
        }

        let pos = vec.iter().position(|x| x == node_id).unwrap();
        let mut path = Vec::new();

        path.extend_from_slice(&vec[pos..]);
        path.extend_from_slice(&vec[..pos]);
        path.push(node_id.clone());

        if verbose {
            let res = path.iter().map(|&node_index| { self.node_to_symbol_map.get(&node_index).unwrap().as_str() }).collect::<Vec<&str>>().join("->");
            let profit = self.calculate_path_profit(&path);
            let mut exchange= Exchange::Any;
            for (instrument, _) in self.edge_to_order_direction_map.values() {
                exchange = instrument.exchange.clone();
                break;
            }

            log::info!("{exchange:?} Arb found {res} expected profit {profit}%");
        }

        Some(path)
    }

    pub fn reset(&mut self) {
        if self.graph.node_count() > 0 {
            self.graph.clear();
            self.edge_to_order_direction_map.clear();
            self.symbol_to_node_map.clear();
            self.node_to_symbol_map.clear();
        }
    }

    pub fn update(&mut self, price_ticker: &PriceTicker) {
        let (base, quote) = self.get_nodes_by_instrument(&price_ticker.instrument);
        self.graph.update_edge(base, quote, -price_ticker.effective_bid(price_ticker.instrument.taker_fee).ln());
        self.graph.update_edge(quote, base, -(1. / price_ticker.effective_ask(price_ticker.instrument.taker_fee)).ln());
    }

    pub fn contains_currency_data(&self, currency: &str) -> bool {
        self.symbol_to_node_map.contains_key(currency)
    }

    pub fn get_direction(&self, pair: &(NodeIndex, NodeIndex)) -> Option<(Arc<Instrument>, OrderSide)> {
        let dir = self.edge_to_order_direction_map.get(pair)?;
        Some(dir.clone())
    }
}
