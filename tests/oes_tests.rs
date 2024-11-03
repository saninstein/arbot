// use std::collections::HashMap;
// use std::rc::Rc;
// use std::sync::Arc;
// use untitled::core::dto::{Instrument, Order, OrderSide, OrderStatus, OrderType, PriceTicker};
// use untitled::core::oes::OrderExecutionSimulator;
//
// fn setup_instruments_and_tickers() -> (HashMap<Arc<Instrument>, PriceTicker>, Arc<Instrument>, Arc<Instrument>) {
//     // Create instruments
//     let btc_usd = Arc::new(Instrument {
//         symbol: "BTC/USD".to_string(),
//         base: "BTC".to_string(),
//         quote: "USD".to_string(),
//     });
//
//     let eth_usd = Arc::new(Instrument {
//         symbol: "ETH/USD".to_string(),
//         base: "ETH".to_string(),
//         quote: "USD".to_string(),
//     });
//
//     // Create price tickers (current prices)
//     let mut tickers: HashMap<Arc<Instrument>, PriceTicker> = HashMap::new();
//     tickers.insert(
//         btc_usd.clone(),
//         PriceTicker {
//             timestamp: 1628600000000,
//             instrument: btc_usd.clone(),
//             bid: 40000.0,
//             bid_amount: 1.0,
//             ask: 40100.0,
//             ask_amount: 1.0,
//         },
//     );
//
//     tickers.insert(
//         eth_usd.clone(),
//         PriceTicker {
//             timestamp: 1628600000000,
//             instrument: eth_usd.clone(),
//             bid: 2500.0,
//             bid_amount: 1.0,
//             ask: 2510.0,
//             ask_amount: 1.0,
//         },
//     );
//
//     (tickers, btc_usd, eth_usd)
// }
//
// #[test]
// fn test_execute_buy_order_with_sufficient_funds() {
//     let (tickers, btc_usd, _eth_usd) = setup_instruments_and_tickers();
//
//     // Initial balances
//     let mut balances: HashMap<String, f64> = HashMap::new();
//     balances.insert("USD".to_string(), 50000.0);
//     balances.insert("BTC".to_string(), 0.5);
//
//     // Create the simulator
//     let mut simulator = OrderExecutionSimulator::new(balances, 0.);
//
//     // Create a BUY market order for 0.1 BTC
//     let order = Order::new(
//         btc_usd.clone(),
//         OrderType::MARKET,
//         OrderSide::BUY,
//         OrderStatus::NEW,
//         0.0,    // Price (not used for market orders)
//         0.1,    // Amount in base currency (BTC)
//         0.0,    // Amount in quote currency (calculated during execution)
//         0.0,    // Amount filled
//     );
//
//     // Execute the order
//     let executed_orders = simulator.execute_market_orders(vec![order], &tickers);
//
//     // Check the balances after execution
//     assert_eq!(simulator.balances.get("USD").unwrap(), &45990.0); // 50000 - 4010 (price * amount)
//     assert_eq!(simulator.balances.get("BTC").unwrap(), &0.6);     // 0.5 + 0.1 BTC
//
//     // Check the order status and amount filled
//     assert_eq!(executed_orders[0].status, OrderStatus::FILLED);
//     assert_eq!(executed_orders[0].amount_filled, 0.1);
//     assert_eq!(executed_orders[0].amount_quote, 4010.0);
// }
//
//
// #[test]
// fn test_execute_buy_order_with_sufficient_funds_amount_quote() {
//     let (tickers, btc_usd, _eth_usd) = setup_instruments_and_tickers();
//
//     // Initial balances
//     let mut balances: HashMap<String, f64> = HashMap::new();
//     balances.insert("USD".to_string(), 4010.0);
//     balances.insert("BTC".to_string(), 0.5);
//
//     // Create the simulator
//     let mut simulator = OrderExecutionSimulator::new(balances, 0.1);
//
//     // Create a BUY market order for 0.1 BTC
//     let order = Order::new(
//         btc_usd.clone(),
//         OrderType::MARKET,
//         OrderSide::BUY,
//         OrderStatus::NEW,
//         0.0,    // Price (not used for market orders)
//         0.0,    // Amount in base currency (BTC)
//         4010.,    // Amount in quote currency (calculated during execution)
//         0.0,    // Amount filled
//     );
//
//     // Execute the order
//     let executed_orders = simulator.execute_market_orders(vec![order], &tickers);
//
//     // Check the balances after execution
//     assert_eq!(simulator.balances.get("USD").unwrap(), &0.); // 50000 - 4010 (price * amount) - 401 (fee)
//     assert_eq!(simulator.balances.get("BTC").unwrap(), &0.59);     // 0.5 + 0.09 BTC (0.1BTC - 10%fee)
//
//     // Check the order status and amount filled
//     assert_eq!(executed_orders[0].status, OrderStatus::FILLED);
//     assert_eq!(executed_orders[0].amount_filled, 0.1);
//     assert_eq!(executed_orders[0].amount_quote, 4010.);
// }
//
// #[test]
// fn test_execute_sell_order_with_sufficient_funds() {
//     let (tickers, _btc_usd, eth_usd) = setup_instruments_and_tickers();
//
//     // Initial balances
//     let mut balances: HashMap<String, f64> = HashMap::new();
//     balances.insert("USD".to_string(), 50000.0);
//     balances.insert("ETH".to_string(), 2.0);
//
//     // Create the simulator
//     let mut simulator = OrderExecutionSimulator::new(balances, 0.);
//
//     // Create a SELL market order for 1.0 ETH
//     let order = Order::new(
//         eth_usd.clone(),
//         OrderType::MARKET,
//         OrderSide::SELL,
//         OrderStatus::NEW,
//         0.0,    // Price (not used for market orders)
//         1.0,    // Amount in base currency (ETH)
//         0.0,    // Amount in quote currency (calculated during execution)
//         0.0,    // Amount filled
//     );
//
//     // Execute the order
//     let executed_orders = simulator.execute_market_orders(vec![order], &tickers);
//
//     // Check the balances after execution
//     assert_eq!(simulator.balances.get("USD").unwrap(), &52500.0); // 50000 + 2500 (price * amount)
//     assert_eq!(simulator.balances.get("ETH").unwrap(), &1.0);     // 2.0 - 1.0 ETH
//
//     // Check the order status and amount filled
//     assert_eq!(executed_orders[0].status, OrderStatus::FILLED);
//     assert_eq!(executed_orders[0].amount_filled, 1.0);
//     assert_eq!(executed_orders[0].amount_quote, 2500.0);
// }
//
// #[test]
// fn test_execute_buy_order_with_insufficient_funds() {
//     let (tickers, btc_usd, _eth_usd) = setup_instruments_and_tickers();
//
//     // Initial balances
//     let mut balances: HashMap<String, f64> = HashMap::new();
//     balances.insert("USD".to_string(), 1000.0); // Not enough to buy 0.1 BTC
//     balances.insert("BTC".to_string(), 0.5);
//
//     // Create the simulator
//     let mut simulator = OrderExecutionSimulator::new(balances, 0.);
//
//     // Create a BUY market order for 0.1 BTC
//     let order = Order::new(
//         btc_usd.clone(),
//         OrderType::MARKET,
//         OrderSide::BUY,
//         OrderStatus::NEW,
//         0.0,    // Price (not used for market orders)
//         0.1,    // Amount in base currency (BTC)
//         0.0,    // Amount in quote currency (calculated during execution)
//         0.0,    // Amount filled
//     );
//
//     // Execute the order
//     let executed_orders = simulator.execute_market_orders(vec![order], &tickers);
//
//     // Check the balances (unchanged)
//     assert_eq!(simulator.balances.get("USD").unwrap(), &1000.0);
//     assert_eq!(simulator.balances.get("BTC").unwrap(), &0.5);
//
//     // Check the order status (canceled) and amount filled (0)
//     assert_eq!(executed_orders[0].status, OrderStatus::CANCELED);
//     assert_eq!(executed_orders[0].amount_filled, 0.0);
//     assert_eq!(executed_orders[0].amount_quote, 0.0);
// }
//
// #[test]
// fn test_execute_sell_order_with_insufficient_funds() {
//     let (tickers, _btc_usd, eth_usd) = setup_instruments_and_tickers();
//
//     // Initial balances
//     let mut balances: HashMap<String, f64> = HashMap::new();
//     balances.insert("USD".to_string(), 50000.0);
//     balances.insert("ETH".to_string(), 0.5); // Not enough to sell 1.0 ETH
//
//     // Create the simulator
//     let mut simulator = OrderExecutionSimulator::new(balances, 0.);
//
//     // Create a SELL market order for 1.0 ETH
//     let order = Order::new(
//         eth_usd.clone(),
//         OrderType::MARKET,
//         OrderSide::SELL,
//         OrderStatus::NEW,
//         0.0,    // Price (not used for market orders)
//         1.0,    // Amount in base currency (ETH)
//         0.0,    // Amount in quote currency (calculated during execution)
//         0.0,    // Amount filled
//     );
//
//     // Execute the order
//     let executed_orders = simulator.execute_market_orders(vec![order], &tickers);
//
//     // Check the balances (unchanged)
//     assert_eq!(simulator.balances.get("USD").unwrap(), &50000.0);
//     assert_eq!(simulator.balances.get("ETH").unwrap(), &0.5);
//
//     // Check the order status (canceled) and amount filled (0)
//     assert_eq!(executed_orders[0].status, OrderStatus::CANCELED);
//     assert_eq!(executed_orders[0].amount_filled, 0.0);
//     assert_eq!(executed_orders[0].amount_quote, 0.0);
// }
//
//
// #[test]
// fn test_execute_orders_chain() {
//     let (tickers, btc_usd, _eth_usd) = setup_instruments_and_tickers();
//
//     // Initial balances
//     let mut balances: HashMap<String, f64> = HashMap::new();
//     balances.insert("USD".to_string(), 50000.0);
//     // balances.insert("BTC".to_string(), 0.);
//
//     // Create the simulator
//     let mut simulator = OrderExecutionSimulator::new(balances, 0.);
//
//     let binding1 = (Arc::clone(&btc_usd), OrderSide::BUY);
//     let binding2 = (Arc::clone(&btc_usd), OrderSide::SELL);
//     let mut orders_directions = vec![
//         &binding1, &binding2
//     ];
//
//     simulator.execute_orders_chain(
//         orders_directions,
//         &tickers,
//     );
//
//     assert_eq!(simulator.balances.get("USD").unwrap(), &49875.31172069826);
// }
