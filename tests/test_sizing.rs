#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use untitled::core::dto::{Instrument, OrderSide, PriceTicker};
    use untitled::core::order_sizing::{max_chain_amount_quote};

    fn create_instrument(
        symbol: &str,
        base: &str,
        quote: &str,
        amount_precision: usize,
        price_precision: usize,
        min_amount: f64,
        max_amount: f64,
        min_notional: f64,
        max_notional: f64
    ) -> Arc<Instrument> {
        Arc::new(Instrument {
            exchange: "test_exchange".to_string(),
            symbol: symbol.to_string(),
            base: base.to_string(),
            quote: quote.to_string(),
            amount_precision,
            price_precision,
            order_amount_min: min_amount,
            order_amount_max: max_amount,
            order_notional_min: min_notional,
            order_notional_max: max_notional,
        })
    }

    fn create_ticker(
        instrument: Arc<Instrument>,
        bid: f64,
        ask: f64,
        bid_amount: f64,
        ask_amount: f64
    ) -> PriceTicker {
        PriceTicker {
            timestamp: 0,
            instrument,
            bid,
            ask,
            bid_amount,
            ask_amount,
        }
    }

    #[test]
    fn test_complex_chain() {
        let btc_usdt = create_instrument(
            "BTC/USDT", "BTC", "USDT",
            6, 2,
            0.001, 10.0,  // Increased max_amount to allow for chain calculations
            10.0, 100000.0
        );
        let eth_btc = create_instrument(
            "ETH/BTC", "ETH", "BTC",
            6, 6,
            0.001, 10.0,  // Increased max_amount
            0.01, 100.0   // Increased max_notional
        );
        let eth_usdt = create_instrument(
            "ETH/USDT", "ETH", "USDT",
            4, 2,
            0.01, 100.0,
            10.0, 100000.0
        );

        let mut tickers_map = HashMap::new();
        tickers_map.insert(
            Arc::clone(&btc_usdt),
            create_ticker(Arc::clone(&btc_usdt), 69680.0, 69681.0, 2.0, 4.0)
        );
        tickers_map.insert(
            Arc::clone(&eth_btc),
            create_ticker(Arc::clone(&eth_btc), 0.0359, 0.03591, 8., 7.0)
        );
        tickers_map.insert(
            Arc::clone(&eth_usdt),
            create_ticker(Arc::clone(&eth_usdt), 2500.0, 2501.0, 10.0, 7.0)
        );

        let orders = vec![
            (Arc::clone(&btc_usdt), OrderSide::Buy),
            (Arc::clone(&eth_btc), OrderSide::Buy),
            (Arc::clone(&eth_usdt), OrderSide::Sell),
        ];

        let result = max_chain_amount_quote(&tickers_map, &orders, 0.000);
        assert!(result.is_some());
        let size = result.unwrap();
        assert_eq!(17515.71297, size)
    }

    #[test]
    fn test_complex_chain2() {
        let btc_usdt = create_instrument(
            "BTC/USDT", "BTC", "USDT",
            6, 2,
            0.001, 10.0,  // Increased max_amount to allow for chain calculations
            10.0, 100000.0
        );
        let btc_try = create_instrument(
            "BTC/TRY", "BTC", "TRY",
            6, 6,
            0.001, 10.0,  // Increased max_amount
            0.01, 100.0   // Increased max_notional
        );
        let usdt_try = create_instrument(
            "USDT/TRY", "USDT", "TRY",
            4, 2,
            0.01, 100.0,
            10.0, 100000.0
        );

        let mut tickers_map = HashMap::new();
        tickers_map.insert(
            Arc::clone(&btc_usdt),
            create_ticker(Arc::clone(&btc_usdt), 69680.0, 69681.0, 2.0, 4.0)
        );
        tickers_map.insert(
            Arc::clone(&btc_try),
            create_ticker(Arc::clone(&btc_try), 2403820., 2403845., 0.05, 0.02)
        );
        tickers_map.insert(
            Arc::clone(&usdt_try),
            create_ticker(Arc::clone(&usdt_try), 34.57, 34.58, 100000., 500000.)
        );

        let orders = vec![
            (Arc::clone(&btc_usdt), OrderSide::Buy),
            (Arc::clone(&btc_try), OrderSide::Sell),
            (Arc::clone(&usdt_try), OrderSide::Buy),
        ];

        let result = max_chain_amount_quote(&tickers_map, &orders, 0.000);
        assert!(result.is_some());
        let size = result.unwrap();
        assert_eq!(3484.05, size)
    }
}
