mod core;
mod draft;

use std::sync::Arc;
use std::{panic, process, thread};
use std::collections::HashSet;
use std::time::Duration;
use crossbeam_queue::ArrayQueue;
use itertools::Itertools;
use core::api::PriceTickerListener;
use core::handlers::{ArbStatPriceTickerListener, PriceTickerFilter};
use core::map::InstrumentsMap;
use crate::core::streams::{dump_freq, PriceTickerStream};
use crate::core::utils::{init_logger, read_tickers};

fn main() {
    init_logger();

    let orig_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // invoke the default handler and exit the process
        orig_hook(panic_info);
        process::exit(1);
    }));

    let tickers_groups = read_tickers("tickers.json".to_string());
    let symbols =  tickers_groups.clone().into_iter().flat_map(|inner| inner.into_iter()).collect();
    let queue = Arc::new(ArrayQueue::new(100_000));
    let instruments_map = Arc::new(InstrumentsMap::from_array_string(symbols));

    let sockets = PriceTickerStream::listen_from_tickers_group(
        Arc::clone(&queue),
        tickers_groups,
        Arc::clone(&instruments_map),
        128
    );

    log::info!("Sockets: {sockets}");


    let mut price_ticker_filter = PriceTickerFilter::new(vec![Box::new(ArbStatPriceTickerListener::new())]);

    loop {
        match queue.pop() {
            Some(price_ticker) => {
                price_ticker_filter.on_price_ticker(&price_ticker);
            }
            None => {
                // log::info!("Empty queue");
                thread::sleep(Duration::from_nanos(10));
                // log::info!("Queue size {}", queue.len());
            }
        }
    }
}
