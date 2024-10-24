mod core;
mod draft;

use std::sync::Arc;
use std::{panic, process, thread};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;
use crossbeam_queue::ArrayQueue;
use itertools::Itertools;
use core::api::PriceTickerListener;
use core::handlers::PriceTickerFilter;
use core::map::InstrumentsMap;
use crate::core::api::MonitoringMessageListener;
use crate::core::dto::{Instrument, PriceTicker, DTO};
use crate::core::strategies::ArbStrategy;
use crate::core::streams::{PriceTickerStream};
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
    let queue = Arc::new(ArrayQueue::new(100_000));
    let instruments_map = Arc::new(InstrumentsMap::from_json("/Users/alex/PycharmProjects/instruments_service/spot_insts.json"));

    let sockets = PriceTickerStream::listen_from_tickers_group(
        Arc::clone(&queue),
        tickers_groups,
        Arc::clone(&instruments_map),
        128
    );

    log::info!("Sockets: {sockets}");
    let empty_map = Default::default();
    let mut price_ticker_filter = PriceTickerFilter::new(
        vec![Box::new(ArbStrategy::new())],
    );

    loop {
        match queue.pop() {
            Some(dto) => {
                match dto {
                    DTO::PriceTicker(price_ticker) => {
                        price_ticker_filter.on_price_ticker(&price_ticker, &empty_map);
                    },
                    DTO::Order(order) => {
                        for mut l in &mut price_ticker_filter.listeners {
                            l.on_order(&order);
                        }
                    },
                    DTO::Balance(balance) => {
                        for mut l in &mut price_ticker_filter.listeners {
                            l.on_balance(&balance);
                        }
                    },
                    DTO::MonitoringMessage(msg) => {
                        price_ticker_filter.on_monitoring_message(&msg);
                        for mut l in &mut price_ticker_filter.listeners {
                            l.on_monitoring_message(&msg);
                        }
                    }
                }
            }
            None => {
                // log::info!("Empty queue");
                thread::sleep(Duration::from_millis(1));
                // log::info!("Queue size {}", queue.len());
            }
        }
    }
}
