use std::net::TcpStream;
use std::sync::{Arc, RwLock};
use std::{fs, thread};
use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;
use crossbeam_queue::ArrayQueue;
use itertools::Itertools;
use json::{object, JsonValue};
use petgraph::matrix_graph::Nullable;
use tungstenite::stream::{MaybeTlsStream, NoDelay};
use tungstenite::{connect, Error, Message, WebSocket};
use tungstenite::http::StatusCode;
use tungstenite::protocol::CloseFrame;
use crate::core::{
    dto::PriceTicker,
    map::InstrumentsMap,
    utils::{parse_f64_field, time},
};
use crate::core::dto::{MonitoringEntity, MonitoringMessage, MonitoringStatus, DTO};

#[allow(dead_code)]
pub struct PriceTickerStream {
    entity_id: usize,
    queue: Arc<ArrayQueue<DTO>>,
    instruments_map: Arc<InstrumentsMap>,
    channels: Vec<String>,
    channels_per_request: usize,

    socket: Option<WebSocket<MaybeTlsStream<TcpStream>>>,

    request_latency: u64,
    request_latest_ts: Arc<RwLock<u128>>,
    request_id: usize,

    latest_ticker_ts: u128
}

#[allow(dead_code)]
impl PriceTickerStream {
    pub fn new(
        entity_id: usize,
        queue: Arc<ArrayQueue<DTO>>,
        channels: Vec<String>,
        instruments_map: Arc<InstrumentsMap>,
        channels_per_request: usize,
        request_latency: u64,
        request_latest_ts: Arc<RwLock<u128>>,
    ) -> Self {
        Self {
            entity_id,
            queue,
            instruments_map,
            channels_per_request,
            channels,
            request_latency,
            request_latest_ts,
            socket: None,
            request_id: 0,
            latest_ticker_ts: 0
        }
    }

    fn connect(&mut self) {
        let (socket, response) = connect("wss://stream.binance.com:9443/ws").expect("Can't connect");
        log::info!("Connected to the server. Response HTTP code: {}", response.status());

        assert!((100..400).contains(&response.status().as_u16()));
        self.request_id = 0;
        self.socket = Some(socket);
    }

    pub fn ticker_to_channel(ticker: &String) -> String {
        let mut s = ticker.to_lowercase().replace("/", "");
        s.push_str("@bookTicker");
        s
    }

    pub fn listen_from_tickers_split(
        queue: Arc<ArrayQueue<DTO>>,
        tickers: Vec<String>,
        instruments_map: Arc<InstrumentsMap>,
        channels_per_stream: usize,
        channels_per_request: usize,
    ) -> usize {
        let request_latest_ts = Arc::new(RwLock::new(0));

        let mut sockets_count = 0;
        for (_, channels) in tickers.iter()
            .map(Self::ticker_to_channel)
            .chunks(channels_per_stream)
            .into_iter()
            .map(|chunk| chunk.collect_vec())
            .enumerate() {
            Self::spawn_stream(sockets_count, channels, &queue, &instruments_map, channels_per_request, &request_latest_ts);
            sockets_count += 1
        }
        sockets_count
    }

    pub fn listen_from_tickers_group(
        queue: Arc<ArrayQueue<DTO>>,
        tickers_groups: Vec<Vec<String>>,
        instruments_map: Arc<InstrumentsMap>,
        channels_per_request: usize,
    ) -> usize {
        let request_latest_ts = Arc::new(RwLock::new(0));

        let mut sockets_count = 0;
        for channels in tickers_groups.iter().map(|v| v.iter().map(Self::ticker_to_channel).collect_vec()) {
            Self::spawn_stream(sockets_count, channels, &queue, &instruments_map, channels_per_request, &request_latest_ts);
            sockets_count += 1
        }
        sockets_count
    }

    fn spawn_stream(socket_id: usize, channels: Vec<String>, queue: &Arc<ArrayQueue<DTO>>, instruments_map: &Arc<InstrumentsMap>, channels_per_request: usize, request_latest_ts: &Arc<RwLock<u128>>) {
        let queue_ref = Arc::clone(queue);
        let instruments_map_ref = Arc::clone(instruments_map);
        let request_latest_ts_ref = Arc::clone(request_latest_ts);

        thread::Builder::new().name(format!("stream_pt_{socket_id}")).spawn(move || {
            Self::new(
                socket_id,
                queue_ref,
                channels,
                instruments_map_ref,
                channels_per_request,
                250,
                request_latest_ts_ref,
            ).run()
        }).expect("Failed to spawn price ticker thread");
    }

    fn send_json(&mut self, data: JsonValue) {
        let mut ts = self.request_latest_ts.write().expect("Can't get the lock");
        self.socket.as_mut().unwrap().send(Message::Text(json::stringify(data))).expect("Can't send the data");
        thread::sleep(Duration::from_millis(250));
        *ts = time();
    }

    fn run(&mut self) {
        let mut reconnect_sleep = 0;
        loop {
            self.connect();
            self.subscribe();
            log::info!("Subscription done");
            self.handle();
            self.close_socket();
            self.queue.push(
                DTO::MonitoringMessage(MonitoringMessage::new(
                    time(),
                    MonitoringStatus::ERROR, MonitoringEntity::PRICE_TICKER,
                    self.entity_id.clone()
                ))
            ).expect("Can't push error message");

            reconnect_sleep += 15;
            log::warn!("Reconnect stream in {reconnect_sleep}s...");
            thread::sleep(Duration::from_secs(reconnect_sleep));
        }
    }

    fn handle_ping(&mut self, ts: u128, ping_payload: Vec<u8>) -> bool {
        let ts_ms_their: u128 = String::from_utf8_lossy(&ping_payload).parse().unwrap();
        let ts_ms_our: u128 =  Duration::from_nanos(ts as u64).as_millis();
        let lag = if ts_ms_our > ts_ms_their {
            ts_ms_our - ts_ms_their
        } else {
             0
        };

        let price_ticker_lag = Duration::from_nanos((ts - self.latest_ticker_ts) as u64).as_millis();
        log::info!("Ping received  their: {} our: {} lag: {}ms. Latest ticker {}ms", ts_ms_their, ts_ms_our, lag, price_ticker_lag);
        match self.socket.as_mut().unwrap().send(Message::Pong(ping_payload)) {
            Ok(_) => return true,
            Err(err) => {
                log::error!("Can't flush socket: {}", err);
                return false;
            }
        };
    }

    fn subscribe(&mut self) {
        for (i, items) in self.channels.clone()
            .chunks(self.channels_per_request)
            .into_iter()
            .enumerate()
        {
            log::info!("Subscribe the batch: {i}");
            self.send_message_and_handle(
                object! {method: "SUBSCRIBE", params: items},
                |_, data: JsonValue| assert!(data["result"].is_null()),
            );
            log::info!("Subscribed the batch: {i}");
        }

        self.send_message_and_handle(
            object! {method: "LIST_SUBSCRIPTIONS"},
            |this, data|
                assert_eq!(
                    HashSet::<String>::from_iter(
                        data["result"]
                            .members()
                            .map(|item| item.to_string())  // Ensure item is a string and convert
                    ),
                    HashSet::from_iter(this.channels.clone()),
                ),
        );
        log::info!("Subs checked")
    }

    fn handle_raw_price_ticker(&mut self, ts: u128, raw: String) {
        let data = &json::parse(&raw).expect("Can't parse json");
        let instrument_arc = self.instruments_map.map.get(data["s"].as_str().expect("No symbol")).expect("No instrument");
        let price_ticker = DTO::PriceTicker(PriceTicker {
            timestamp: ts,
            instrument: Arc::clone(instrument_arc),
            bid: parse_f64_field(data, "b"),
            bid_amount: parse_f64_field(data, "B"),
            ask: parse_f64_field(data, "a"),
            ask_amount: parse_f64_field(data, "A"),
        });

        self.queue.push(price_ticker).expect("Can't add price ticker to queue");
    }

    fn handle(&mut self) {
        loop {
            let result = self.socket.as_mut().unwrap().read();
            let ts = time();
            match result {
                Ok(msg) => match msg {
                    Message::Text(raw) => {
                        self.latest_ticker_ts = ts.clone();
                        self.handle_raw_price_ticker(ts, raw);
                    }
                    Message::Ping(payload ) => {
                        if !self.handle_ping(ts, payload) {
                            return;
                        }
                    }
                    msg => log::warn!("Unexpected msg: {msg}")
                }
                Err(err) => {
                    log::error!("Can't read socket: {}", err);
                    return;
                }
            };
        }
    }

    fn close_socket(&mut self) {
        if let Some(mut socket) = self.socket.take() {
            match socket.close(None) {
                Ok(_) => {
                    log::info!("Socket closed successfully");
                }
                Err(err) => {
                    log::warn!("Error during the socket closing: {}", err);
                }
            }
        }
    }

    fn next_id(&mut self) -> usize {
        let id = self.request_id.clone();
        self.request_id += 1;
        id
    }

    fn send_message_and_handle(&mut self, mut data: JsonValue, result_handler: fn(&PriceTickerStream, JsonValue)) {
        let id = self.next_id();
        data.insert("id", id).expect("Can't insert data");
        self.send_json(data);
        loop {
            let msg = self.socket.as_mut().unwrap().read().expect("Error reading message");
            match msg {
                Message::Text(a) => {
                    // log::info!("Message during sub: {a}");
                    if a.starts_with("{\"r") {
                        // log::info!("Got the result: {}", a);
                        let data = json::parse(&a).expect("Can't parse json");
                        let got_id = data["id"].as_usize().unwrap();
                        assert!(id == got_id);
                        result_handler(self, data);
                        return;
                    }
                }
                Message::Ping(payload) => {
                    if !self.handle_ping(time(), payload) {
                        return;
                    }
                }
                _ => log::warn!("Unexpected msg type")
            }
        }
    }
}

// pub fn dump_freq(
//     path: String,
//     tickers: Vec<String>,
//     instruments_map: Arc<InstrumentsMap>,
// ) {
//     let request_latest_ts = Arc::new(RwLock::new(0));
//     let queue = Arc::new(ArrayQueue::new(100_000));
//     for (i, channels) in tickers.iter()
//         .map(PriceTickerStream::ticker_to_channel)
//         .chunks(10)
//         .into_iter()
//         .map(|chunk| chunk.collect_vec())
//         .enumerate() {
//         if i < 9 {
//             continue;
//         }
//         log::info!("Process: {channels:?}");
//         // if i == 13 {
//         //     println!("{channels:?}")
//         // }
//         // continue;
//
//         let queue_ref = Arc::clone(&queue);
//         let instruments_map_ref = Arc::clone(&instruments_map);
//         let request_latest_ts_ref = Arc::clone(&request_latest_ts);
//
//         let t = thread::Builder::new().spawn(move || {
//             let mut stream = PriceTickerStream::new(
//                 queue_ref,
//                 channels,
//                 instruments_map_ref,
//                 25,
//                 250,
//                 request_latest_ts_ref,
//             );
//             stream.subscribe();
//             let ts_start = time();
//             let ts_end = ts_start + Duration::from_secs(10).as_nanos();
//
//             match stream.socket.get_mut() {
//                 MaybeTlsStream::NativeTls(t) => {
//                     // -- use either one or another
//                     //t.get_mut().set_nonblocking(true);
//                     t.get_mut().set_read_timeout(Some(Duration::from_millis(100))).expect("Error: cannot set read-timeout to underlying stream");
//                 },
//                 // handle more cases as necessary, this one only focuses on native-tls
//                 _ => unimplemented!()
//             }
//
//
//             while time() < ts_end {
//                 match stream.socket.read() {
//                     Ok(Message::Text(raw)) => {
//                         stream.handle_raw_price_ticker(time() - ts_start, raw)
//                     },
//                     Err(err) => {
//                         match err {
//                             // Silently drop the error: Processing error: IO error: Resource temporarily unavailable (os error 11)
//                             // That occurs when no messages are to be read
//                             Error::Io(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
//                                 // log::info!("Would block");
//                             },
//                             _ => panic!("{}", err),
//                         }
//                     },
//                     _ => {}
//                 }
//
//             }
//         }).expect("Failed to spawn price ticker thread");
//
//         let mut v = Vec::new();
//         while !t.is_finished() || !queue.is_empty() {
//             match queue.pop() {
//                 Some(price_ticker) => {
//                     let base = price_ticker.instrument.base.clone();
//                     let quote = price_ticker.instrument.quote.clone();
//                     v.push(object! {ts: price_ticker.timestamp.to_string(), symbol: format!("{base}/{quote}")});
//                 }
//                 None => {
//                     // log::info!("Empty queue");
//                     thread::sleep(Duration::from_nanos(10));
//                     // log::info!("Queue size {}", queue.len());
//                 }
//             }
//         }
//
//         fs::write(Path::new(&path).join(format!("{i}.json")), json::stringify(v)).expect("Can't write json");
//         log::info!("Done {i}")
//     }
// }
