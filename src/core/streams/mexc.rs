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
    utils::{parse_f64_field, time},
};
use crate::core::dto::{Exchange, MonitoringEntity, MonitoringMessage, MonitoringStatus, DTO};

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

    latest_ticker_ts: u128,

    next_ping_ts: u128,
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
            latest_ticker_ts: 0,
            next_ping_ts: time() + Duration::from_secs(25).as_nanos(),
        }
    }

    fn connect(&mut self) {
        let (mut socket, response) = connect("wss://wbs.mexc.com/ws").expect("Can't connect");
        log::info!("Connected to the server. Response HTTP code: {}", response.status());

        match socket.get_mut() {
            MaybeTlsStream::Rustls(ref mut t) => {
                // -- use either one or another
                //t.get_mut().set_nonblocking(true);
                t.get_mut().set_read_timeout(Some(Duration::from_millis(10000))).expect("Error: cannot set read-timeout to underlying stream");
            }
            // handle more cases as necessary, this one only focuses on native-tls
            _ => unimplemented!()
        }

        assert!((100..400).contains(&response.status().as_u16()));
        self.request_id = 0;
        self.socket = Some(socket);
    }

    pub fn ticker_to_channel(ticker: &String) -> String {
        let mut s = "spot@public.bookTicker.v3.api@".to_string();
        s.push_str(&ticker.to_uppercase().replace("/", ""));
        s
    }

    pub fn listen_from_tickers_split(
        queue: Arc<ArrayQueue<DTO>>,
        tickers: Vec<String>,
        instruments_map: Arc<InstrumentsMap>,
        channels_per_stream: usize,
        channels_per_request: usize,
    ) -> usize {
        assert!(channels_per_stream <= 30);
        assert!(channels_per_request <= 30);
        let request_latest_ts = Arc::new(RwLock::new(0));

        let mut sockets_count = 0;
        for (_, channels) in tickers.iter()
            .map(Self::ticker_to_channel)
            .chunks(channels_per_stream)
            .into_iter()
            .map(|chunk| chunk.collect_vec())
            .enumerate() {
            Self::spawn_stream(sockets_count, channels, &queue, &instruments_map, channels_per_request, &request_latest_ts);
            thread::sleep(Duration::from_millis(1000));
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

        thread::Builder::new().name(format!("{:?}_pt_{}", Exchange::Mexc, socket_id)).spawn(move || {
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
        self.send_json_with_sleep(data, Duration::from_millis(250));
    }

    fn send_json_with_sleep(&mut self, data: JsonValue, duration: Duration) {
        let mut ts = self.request_latest_ts.write().expect("Can't get the lock");
        self.socket.as_mut().unwrap().send(Message::Text(json::stringify(data))).expect("Can't send the data");
        thread::sleep(duration);
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

    fn subscribe(&mut self) {
        for (i, items) in self.channels.clone()
            .chunks(self.channels_per_request)
            .into_iter()
            .enumerate()
        {
            log::info!("Subscribe the batch: {i}");
            self.send_message_and_handle(
                object! {method: "SUBSCRIPTION", params: items},
                |_, data: JsonValue| assert!(data["code"].as_usize().unwrap() == 0),
            );
            log::info!("Subscribed the batch: {i}");
        }

        // self.send_message_and_handle(
        //     object! {method: "LIST_SUBSCRIPTIONS"},
        //     |this, data|
        //         assert_eq!(
        //             HashSet::<String>::from_iter(
        //                 data["result"]
        //                     .members()
        //                     .map(|item| item.to_string())  // Ensure item is a string and convert
        //             ),
        //             HashSet::from_iter(this.channels.clone()),
        //         ),
        // );
        // log::info!("Subs checked")
    }

    fn handle_raw_price_ticker(&mut self, ts: u128, raw: String) {
        let data = &json::parse(&raw).expect("Can't parse json");
        let symbol = data["s"].as_str().expect(&format!("No symbol: {raw}"));
        let instrument_arc = self.instruments_map.get(&Exchange::Mexc, symbol).expect(&format!("No instrument: {symbol}"));
        let ticker_data = &data["d"];
        let price_ticker = DTO::PriceTicker(PriceTicker {
            timestamp: ts,
            instrument: Arc::clone(instrument_arc),
            bid: parse_f64_field(ticker_data, "b"),
            bid_amount: parse_f64_field(ticker_data, "B"),
            ask: parse_f64_field(ticker_data, "a"),
            ask_amount: parse_f64_field(ticker_data, "A"),
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
                        if raw == "{\"id\":0,\"code\":0,\"msg\":\"PONG\"}" {
                            let price_ticker_lag = Duration::from_nanos((ts - self.latest_ticker_ts) as u64).as_millis();
                            log::info!("Pong received. Latest price ticker: {price_ticker_lag}ms");
                        } else {
                            self.latest_ticker_ts = ts;
                            self.handle_raw_price_ticker(ts, raw);
                        }
                    }
                    msg => log::warn!("Unexpected msg: {msg}")
                }
                Err(err) => {
                    log::error!("Can't read socket: {}", err);
                    return;
                }
            };

            if self.next_ping_ts < ts {
                log::info!("Send ping");
                self.send_json_with_sleep(
                    object! { method: "PING" },
                    Duration::from_nanos(0),
                );
                self.next_ping_ts = ts + Duration::from_secs(29).as_nanos();
            }
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
        let id = self.request_id;
        self.request_id += 1;
        id
    }

    fn send_message_and_handle(&mut self, data: JsonValue, result_handler: fn(&PriceTickerStream, JsonValue)) {
        self.send_json(data);
        loop {
            let msg = self.socket.as_mut().unwrap().read();
            match msg {
                Ok(Message::Text(a)) => {
                    log::info!("Message during sub: {a}");
                    if a.starts_with("{\"id") {
                        let data = json::parse(&a).expect("Can't parse json");
                        result_handler(self, data);
                        return;
                    }
                },
                Err(err) => {
                    match err {
                        // Silently drop the error: Processing error: IO error: Resource temporarily unavailable (os error 11)
                        // That occurs when no messages are to be read
                        Error::Io(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(100));
                        },
                        _ => panic!("{}", err),
                    }
                }

                t => log::warn!("Unexpected msg type: {t:?}")
            }
        }
    }
}
