use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use crate::core::dto::OrderType;
use crate::core::dto::{Instrument, Order, OrderSide, OrderStatus, PriceTicker};

pub struct OrderExecutionSimulator {
    pub balances: HashMap<String, f64>,
    fee: f64
}

impl OrderExecutionSimulator {
    pub fn new(
        balances: HashMap<String, f64>,
        fee: f64,
    ) -> Self {
        Self { balances, fee }
    }

    pub fn execute_orders_chain(&mut self, orders_direction: Vec<&(Arc<Instrument>, OrderSide)>, tickers_map: &HashMap<Arc<Instrument>, PriceTicker>) -> Vec<Order>  {
        let mut executed_orders = Vec::new();
        log::info!("orders_direction: {orders_direction:?}");
        for (instrument, order_side) in orders_direction {
            let amount;
            let amount_quote;

            let balances = &self.balances;
            log::info!("instrument: {instrument:?} order_side: {order_side:?} balance: {balances:?}");

            if *order_side == OrderSide::BUY {  // we have quote on balance
                amount = 0.;
                amount_quote = self.balances.get(&instrument.quote).unwrap().clone();
            } else {  // we have base on balance
                amount = self.balances.get(&instrument.base).unwrap().clone();
                amount_quote = 0.;
            }

            // let order = Order::new(
            //     Arc::clone(&instrument),
            //     OrderType::MARKET,
            //     order_side.clone(),
            //     OrderStatus::NEW,
            //     0.,
            //     amount,
            //     amount_quote,
            //     0.
            // );

            // executed_orders.append(
            //     &mut self.execute_market_orders(vec![order], tickers_map)
            // );
        }

        executed_orders
    }

    /// Simulate execution of a batch of MARKET orders, update balances, and return the updated orders
    pub fn execute_market_orders(&mut self, orders: Vec<Order>, tickers_map: &HashMap<Arc<Instrument>, PriceTicker>) -> Vec<Order> {
        let mut executed_orders = Vec::new();
        for mut order in orders {
            // Find the price ticker for the instrument in the order
            if let Some(ticker) = tickers_map.get(&order.instrument) {
                // Handle the case where amount is 0 but amount_quote is provided
                if order.amount == 0.0 && order.amount_quote > 0.0 {
                    // Calculate amount from amount_quote based on the current price
                    if order.side == OrderSide::BUY {
                        order.amount = order.amount_quote / ticker.ask; // Calculate how much to buy
                    } else {
                        order.amount = order.amount_quote / ticker.bid; // Calculate how much to sell
                    }
                }
                match order.side {
                    OrderSide::BUY => {
                        // BUY: Fill at the ask price
                        let total_cost = order.amount * ticker.ask; // amount * ask price in quote currency

                        // Check if the user has enough balance in the quote currency (e.g., USDT)
                        let quote_balance = self
                            .balances
                            .entry(order.instrument.quote.clone())
                            .or_insert(0.0);
                        if *quote_balance >= total_cost {
                            *quote_balance -= total_cost; // Deduct the total cost from the quote balance
                            let base_balance = self
                                .balances
                                .entry(order.instrument.base.clone())
                                .or_insert(0.0);
                            *base_balance += order.amount * (1. - self.fee); // Add the bought base amount to the balance
                            order.amount_filled = order.amount; // Fully fill the order
                            order.amount_quote = total_cost;
                            order.status = OrderStatus::FILLED;
                        } else {
                            order.status = OrderStatus::CANCELED; // Not enough balance, cancel the order
                        }
                    }
                    OrderSide::SELL => {
                        // SELL: Fill at the bid price
                        let total_proceeds = order.amount * ticker.bid; // amount * bid price in quote currency

                        // Check if the user has enough balance in the base currency (e.g., BTC)
                        let base_balance = self
                            .balances
                            .entry(order.instrument.base.clone())
                            .or_insert(0.0);
                        if *base_balance >= order.amount {
                            *base_balance -= order.amount; // Deduct the sold amount from the base balance
                            let quote_balance = self
                                .balances
                                .entry(order.instrument.quote.clone())
                                .or_insert(0.0);
                            *quote_balance += total_proceeds * (1. - self.fee); // Add the proceeds to the quote balance
                            order.amount_filled = order.amount; // Fully fill the order
                            order.amount_quote = total_proceeds;
                            order.status = OrderStatus::FILLED;
                        } else {
                            order.status = OrderStatus::CANCELED; // Not enough base currency, cancel the order
                        }
                    }
                }
            } else {
                // If no ticker data is found for the instrument, mark the order as CANCELED
                order.status = OrderStatus::CANCELED;
            }
            // let balances = &self.balances;
            // log::info!("OrderExecutionSimulator balance after order execution: {balances:?}");
            executed_orders.push(order);
        }

        executed_orders
    }
}
