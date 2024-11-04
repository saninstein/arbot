mod core;
mod draft;

use std::sync::Arc;
use std::{panic, process, thread};
use std::collections::HashSet;
use std::time::Duration;
use crossbeam_queue::ArrayQueue;
use core::api::PriceTickerListener;
use core::handlers::PriceTickerFilter;
use core::map::InstrumentsMap;
use crate::core::api::MonitoringMessageListener;
use crate::core::dto::{Exchange, MonitoringEntity, MonitoringMessage, MonitoringStatus, DTO};
use crate::core::oms::OMS;
use crate::core::strategies::ArbStrategy;
use crate::core::{streams};
use crate::core::order_sizing::SizingConfig;
use crate::core::utils::{init_logger, read_tickers, time};


fn main() {
    init_logger();

    let orig_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // invoke the default handler and exit the process
        orig_hook(panic_info);
        process::exit(1);
    }));


    let queue = Arc::new(ArrayQueue::new(200_000));
    let orders_queue = Arc::new(ArrayQueue::new(100_000));

    let instruments_map = Arc::new(InstrumentsMap::from_json("./data/spot_insts.json"));

    let mut sockets = streams::binance::PriceTickerStream::listen_from_tickers_group(
        Arc::clone(&queue),
        read_tickers("./data/tickers.json"),
        Arc::clone(&instruments_map),
        128
    );

    log::info!("Binance sockets: {sockets}");

    sockets = streams::bit2me::PriceTickerStream::listen_from_tickers_split(
        Arc::clone(&queue),
        HashSet::<String>::from_iter(
            instruments_map.map.get(&Exchange::Bit2me).unwrap().values().map(|x| x.symbol.clone())
        ).iter().map(|x| x.clone()).collect(),
        Arc::clone(&instruments_map),
        300,
        1
    );

    log::info!("Bit2me sockets: {sockets}");

    let empty_map = Default::default();

    let sizing_config = SizingConfig::new("USDT".to_string(), 20., 30.);

    let mut price_ticker_filter = PriceTickerFilter::new(
        vec![
            Box::new(ArbStrategy::new(Arc::clone(&orders_queue), Exchange::Binance, sizing_config.clone(), true)),
            Box::new(ArbStrategy::new(Arc::clone(&orders_queue), Exchange::Bit2me, sizing_config.clone(), true))
        ],
    );

    // oms isn't up yet
    queue.push(
        DTO::MonitoringMessage(MonitoringMessage::new(
            time(),
            MonitoringStatus::Error,
            MonitoringEntity::OrderManagementSystem,
            1,
        ))
    ).expect("Can't add message to queue");

    OMS::start(
        Arc::clone(&orders_queue),
        Arc::clone(&queue),
        Arc::clone(&instruments_map),
        ".creds/binance.pem".to_string(),
        "7YPfVLXzckzQyMnWicLQiWEyhiOPJwGCLR27ErnbhsJUPKO3TnfT9N28YU9qePSX".to_string()
    );

    loop {
        match queue.pop() {
            Some(dto) => {
                match dto {
                    DTO::PriceTicker(price_ticker) => {
                        price_ticker_filter.on_price_ticker(&price_ticker, &empty_map);
                    },
                    DTO::Order(order) => {
                        for l in &mut price_ticker_filter.listeners {
                            l.on_order(&order);
                        }
                    },
                    DTO::Balance(balance) => {
                        for l in &mut price_ticker_filter.listeners {
                            l.on_balance(&balance);
                        }
                    },
                    DTO::MonitoringMessage(msg) => {
                        price_ticker_filter.on_monitoring_message(&msg);
                        for l in &mut price_ticker_filter.listeners {
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
