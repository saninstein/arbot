use std::net::TcpStream;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
use crossbeam_queue::ArrayQueue;
use itertools::Itertools;
use json::{object, JsonValue};
use tungstenite::stream::{MaybeTlsStream};
use tungstenite::{connect, Error, Message, WebSocket};
use crate::core::{
    dto::PriceTicker,
    map::InstrumentsMap,
    utils::{time},
};
use crate::core::dto::{Exchange, MonitoringEntity, MonitoringMessage, MonitoringStatus, DTO, TICKER_PRICE_NOT_CHANGED};

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

    latest_ticker_ts: u128,
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
            latest_ticker_ts: 0,
        }
    }

    fn connect(&mut self) {
        let (mut socket, response) = connect("wss://ws.bit2me.com/v1/trading").expect("Can't connect");
        log::info!("Connected to the server. Response HTTP code: {}", response.status());

        match socket.get_mut() {
            MaybeTlsStream::Rustls(ref mut t) => {
                // -- use either one or another
                //t.get_mut().set_nonblocking(true);
                t.get_mut().set_read_timeout(Some(Duration::from_millis(1000))).expect("Error: cannot set read-timeout to underlying stream");
            }
            // handle more cases as necessary, this one only focuses on native-tls
            _ => unimplemented!()
        }

        assert!((100..400).contains(&response.status().as_u16()));
        self.socket = Some(socket);
    }

    pub fn listen_from_tickers_split(
        queue: Arc<ArrayQueue<DTO>>,
        tickers: Vec<String>,
        instruments_map: Arc<InstrumentsMap>,
        channels_per_stream: usize,
        channels_per_request: usize,
    ) -> usize {
        assert_eq!(1, channels_per_request);
        let request_latest_ts = Arc::new(RwLock::new(0));

        let mut sockets_count = 0;
        for (_, channels) in tickers.iter()
            .map(String::to_string)
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
        assert_eq!(1, channels_per_request);
        let request_latest_ts = Arc::new(RwLock::new(0));

        let mut sockets_count = 0;
        for channels in tickers_groups.iter().map(|v| v.iter().map(String::to_string).collect_vec()) {
            Self::spawn_stream(sockets_count, channels, &queue, &instruments_map, channels_per_request, &request_latest_ts);
            sockets_count += 1
        }
        sockets_count
    }

    fn spawn_stream(socket_id: usize, channels: Vec<String>, queue: &Arc<ArrayQueue<DTO>>, instruments_map: &Arc<InstrumentsMap>, channels_per_request: usize, request_latest_ts: &Arc<RwLock<u128>>) {
        let queue_ref = Arc::clone(queue);
        let instruments_map_ref = Arc::clone(instruments_map);
        let request_latest_ts_ref = Arc::clone(request_latest_ts);

        thread::Builder::new().name(format!("{:?}_pt_{}", Exchange::Bit2me, socket_id)).spawn(move || {
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
        thread::sleep(Duration::from_millis(50));
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
                    MonitoringStatus::Error, MonitoringEntity::PriceTicker,
                    self.entity_id,
                ))
            ).expect("Can't push error message");

            reconnect_sleep += 15;
            log::warn!("Reconnect stream in {reconnect_sleep}s...");
            thread::sleep(Duration::from_secs(reconnect_sleep));
        }
    }

    fn handle_ping(&mut self, ts: u128, ping_payload: Vec<u8>) -> bool {
        // let ts_ms_their: u128 = String::from_utf8_lossy(&ping_payload).parse().unwrap();
        // let ts_ms_our: u128 = Duration::from_nanos(ts as u64).as_millis();
        // let lag = if ts_ms_our > ts_ms_their {
        //     ts_ms_our - ts_ms_their
        // } else {
        //     0
        // };
        //
        let price_ticker_lag = Duration::from_nanos((ts - self.latest_ticker_ts) as u64).as_millis();
        log::info!("Ping received. Latest ticker {}ms", price_ticker_lag);
        match self.socket.as_mut().unwrap().send(Message::Pong(ping_payload)) {
            Ok(_) => return true,
            Err(err) => {
                log::error!("Can't flush socket: {}", err);
                return false;
            }
        };
    }

    fn subscribe(&mut self) {
        let total = self.channels.len();
        for (i, symbol) in self.channels.clone().into_iter().enumerate() {
            // {
            //     "event":"subscribe",
            //     "symbol":"B2M/USDT",
            //     "subscription":{"name":"order-book"}
            // }

            log::info!("Subscribe to {symbol}");
            self.send_message_and_handle(
                object! { event: "subscribe", subscription: object! { name: "order-book" }, symbol: symbol.clone() },
                |_, data: JsonValue| assert_eq!("subscribed", data["result"])
            );
            log::info!("Subscribed {}/{}", i + 1, total);
        }

        log::info!("Subs checked")
    }

    fn send_message_and_handle(&mut self, data: JsonValue, result_handler: fn(&PriceTickerStream, JsonValue)) {
        self.send_json(data);
        loop {
            let msg = self.socket.as_mut().unwrap().read();
            match msg {
                Ok(Message::Text(a)) => {
                    // log::info!("Message during sub: {a}");
                    if a.starts_with("{\"event\":\"subscribe\"") {
                        // log::info!("Got the result: {}", a);
                        let data = json::parse(&a).expect("Can't parse json");
                        result_handler(self, data);
                        return;
                    }
                }
                Ok(Message::Ping(payload)) => {
                    if !self.handle_ping(time(), payload) {
                        return;
                    }
                },
                Err(err) => {
                    match err {
                        // Silently drop the error: Processing error: IO error: Resource temporarily unavailable (os error 11)
                        // That occurs when no messages are to be read
                        Error::Io(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            // thread::sleep(Duration::from_millis(100));
                        },
                        _ => panic!("{}", err),
                    }
                }

                t => log::warn!("Unexpected msg type: {t:?}")
            }
        }
    }

    fn parse_price_ticker(&mut self, ts: u128, raw: &str) -> Option<DTO> {
        let data = &json::parse(raw).expect("Can't parse json")["data"];
        let symbol = data["symbol"].as_str().expect("No symbol");
        let instrument_arc = self.instruments_map.get(&Exchange::Bit2me, symbol).expect(&format!("No instrument: {symbol}"));
        let bid = &data["bids"][0];
        let ask = &data["asks"][0];
        let bid_price;
        let bid_amount;
        let ask_price;
        let ask_amount;
        if bid.is_empty() {
            bid_price = TICKER_PRICE_NOT_CHANGED;
            bid_amount = TICKER_PRICE_NOT_CHANGED;
        } else {
            bid_price = bid[0].as_f64()?;
            bid_amount = bid[1].as_f64()?;
        }
        if ask.is_empty() {
            ask_price = TICKER_PRICE_NOT_CHANGED;
            ask_amount = TICKER_PRICE_NOT_CHANGED;
        } else {
            ask_price = ask[0].as_f64()?;
            ask_amount = ask[1].as_f64()?;
        }
        Some(DTO::PriceTicker(PriceTicker {
            timestamp: ts,
            instrument: Arc::clone(instrument_arc),
            bid: bid_price,
            bid_amount: bid_amount,
            ask: ask_price,
            ask_amount: ask_amount,
        }))
    }

    fn handle_raw_price_ticker(&mut self, ts: u128, raw: String) {
        if let Some(price_ticker) = self.parse_price_ticker(ts, &raw) {
            self.queue.push(price_ticker).expect("Can't add price ticker to queue");
        } else {
            panic!("Can't parse price ticker: {raw}");
        }
    }

    fn handle(&mut self) {
        loop {
            let result = self.socket.as_mut().unwrap().read();
            let ts = time();

            match result {
                Ok(msg) => match msg {
                    Message::Text(raw) => {
                        // log::info!("Received message: {raw}");
                        self.latest_ticker_ts = ts;
                        self.handle_raw_price_ticker(ts, raw);
                    }
                    Message::Ping(payload) => {
                        if !self.handle_ping(ts, payload) {
                            return;
                        }
                    }
                    msg => log::warn!("Unexpected msg: {msg}")
                }
                Err(err) => {
                    match err {
                        Error::Io(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        //     log::warn!("Would block");
                        },
                        _ => {
                            log::error!("Can't read socket: {}", err);
                            return;
                        },
                    }
                    // return;
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
}
