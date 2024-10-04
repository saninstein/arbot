#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use uuid::Uuid;

    #[test]
    fn it_works() {
        let id = Uuid::new_v4().to_string();
        println!("{id}");
    }

    #[test]
    fn check_stream() {
        let symbols = vec!["ETH/BTC", "LTC/BTC", "BNB/BTC", "NEO/BTC", "QTUM/ETH", "EOS/ETH", "SNT/ETH", "BNT/ETH"];
        let channels_per_stream = symbols.iter()
            .map(|x| {
                let mut s = x.to_lowercase().replace("/", "");
                s.push_str("@bookTicker");
                s
            })
            .chunks(3)
            .into_iter()
            .map(|chunk| chunk.collect_vec())
            .collect_vec();

        println!("RES: {:?}", channels_per_stream);

    }

    // #[test]
    // fn test_graph_single_cycle_after_third_ticker() {
    //     let mut listener = ArbStatPriceTickerListener::new();
    //
    //     // First ticker: A -> USDT
    //     let price_ticker_a = PriceTicker {
    //         timestamp: 0,
    //         instrument: Rc::new(Instrument {
    //             symbol: "AUSDT".to_string(),
    //             base: "A".to_string(),
    //             quote: "USDT".to_string(),
    //         }),
    //         bid: 1.0,  // No arbitrage opportunity with a 1:1 rate
    //         bid_amount: 0.0,
    //         ask: 1.0,
    //         ask_amount: 0.0,
    //     };
    //
    //     // 100USDT -> 100A -> 111.111111B ->
    //
    //     // Second ticker: B -> USDT
    //     let price_ticker_b = PriceTicker {
    //         timestamp: 0,
    //         instrument: Rc::new(Instrument {
    //             symbol: "BUSDT".to_string(),
    //             base: "B".to_string(),
    //             quote: "USDT".to_string(),
    //         }),
    //         bid: 1.0,  // Again, no arbitrage here, same 1:1 rate
    //         bid_amount: 0.0,
    //         ask: 1.0,
    //         ask_amount: 0.0,
    //     };
    //
    //     // After these two, no cycle should be found
    //     listener.on_price_ticker(&price_ticker_a);
    //     listener.on_price_ticker(&price_ticker_b);
    //
    //     // This should print "Negative cycle not found"
    //     println!("After two tickers:");
    //     listener.on_price_ticker(&price_ticker_a);  // Checking with the same tickers
    //
    //     // Third ticker: A -> B
    //     let price_ticker_ab = PriceTicker {
    //         timestamp: 0,
    //         instrument: Rc::new(Instrument {
    //             symbol: "AB".to_string(),
    //             base: "A".to_string(),
    //             quote: "B".to_string(),
    //         }),
    //         bid: 1.1,  // Now, there's an opportunity for arbitrage
    //         bid_amount: 0.0,
    //         ask: 0.9,
    //         ask_amount: 0.0,
    //     };
    //
    //     // After adding this third ticker, a negative cycle should be found
    //     println!("After adding third ticker (should find cycle):");
    //     listener.on_price_ticker(&price_ticker_ab);
    // }
}
