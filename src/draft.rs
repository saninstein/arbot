#[cfg(test)]
mod tests {
    use std::backtrace::Backtrace;
    use std::collections::HashMap;
    use std::fs::File;
    use std::sync::Arc;
    use itertools::Itertools;
    use uuid::Uuid;
    use crate::core::dto::{Instrument, Order, OrderSide, OrderStatus, OrderType, PriceTicker};
    use crate::core::oes::OrderExecutionSimulator;

    #[test]
    fn it_works() {
        let id = Uuid::new_v4().to_string();
        println!("{id}");
    }

    #[test]
    fn check_stream() {

        // let symbols = vec!["ETH/BTC", "LTC/BTC", "BNB/BTC", "NEO/BTC", "QTUM/ETH", "EOS/ETH", "SNT/ETH", "BNT/ETH"];
        // let channels_per_stream = symbols.iter()
        //     .map(|x| {
        //         let mut s = x.to_lowercase().replace("/", "");
        //         s.push_str("@bookTicker");
        //         s
        //     })
        //     .chunks(3)
        //     .into_iter()
        //     .map(|chunk| chunk.collect_vec())
        //     .collect_vec();
        //
        // println!("RES: {:?}", channels_per_stream);

    }
}
