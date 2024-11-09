use std::net::TcpStream;
use std::sync::{Arc, RwLock};
use std::thread;
use std::collections::HashSet;
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

pub type Res = Result<(), Box<dyn std::error::Error>>;

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
        }
    }

    fn connect(&mut self) -> Res {
        let (mut socket, response) = connect("wss://stream.binance.com:9443/ws")?;
        // let (mut socket, response) = connect("wss://testnet.binance.vision/ws").expect("Can't connect");
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

        if !(100..400).contains(&response.status().as_u16()) {
            return Err(response.status().as_str().into());
        }

        self.request_id = 0;
        self.socket = Some(socket);
        Ok(())
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

        thread::Builder::new().name(format!("{:?}_pt_{}", Exchange::Binance, socket_id)).spawn(move || {
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

    fn send_json(&mut self, data: JsonValue) -> Res {
        let mut ts = self.request_latest_ts.write().unwrap();
        self.socket.as_mut().unwrap().send(Message::Text(json::stringify(data)))?;
        thread::sleep(Duration::from_millis(250));
        *ts = time();
        Ok(())
    }

    fn run(&mut self) {
        let mut reconnect_sleep = 5;

        let retry_shots = 5;
        loop {
            for i in 0..reconnect_sleep + 1 {
                match self.connect() {
                    Ok(_) => {
                        break;
                    }
                    Err(_) => {
                        if i == retry_shots {
                            panic!("Failed to reconnect stream");
                        }
                        log::warn!("Failed to reconnect stream. Retry {i}/{retry_shots}");
                        thread::sleep(Duration::from_millis(1000));
                    }
                }
            }
            match self.subscribe() {
                Ok(_) => {
                    log::info!("Subscription done");
                    match self.handle() {
                        Err(err) => {
                            log::error!("Error handling messages: {}", err);
                        },
                        _ => {
                            log::warn!("Handling messages stopped");
                        }
                    };
                }
                Err(err) => {
                    log::error!("Subscription failed: {}", err);
                }
            };
            self.close_socket();
            self.queue.push(
                DTO::MonitoringMessage(MonitoringMessage::new(
                    time(),
                    MonitoringStatus::Error, MonitoringEntity::PriceTicker,
                    self.entity_id,
                ))
            ).expect("Can't push error message");

            reconnect_sleep += 5;
            log::warn!("Reconnect stream in {reconnect_sleep}s...");
            thread::sleep(Duration::from_secs(reconnect_sleep.max(30)));
        }
    }

    fn handle_ping(&mut self, ts: u128, ping_payload: Vec<u8>) -> Res {
        let price_ticker_lag = Duration::from_nanos((ts - self.latest_ticker_ts) as u64).as_millis();
        log::info!("Ping received. Latest ticker {}ms", price_ticker_lag);
        self.socket.as_mut().unwrap().send(Message::Pong(ping_payload))?;
        Ok(())
    }

    fn subscribe(&mut self) -> Res {
        for (i, items) in self.channels.clone()
            .chunks(self.channels_per_request)
            .into_iter()
            .enumerate()
        {
            log::info!("Subscribe the batch: {i}");
            self.send_message_and_handle(
                object! {method: "SUBSCRIBE", params: items},
                |_, data: JsonValue| assert!(data["result"].is_null()),
            )?;
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
        )?;
        Ok(())
    }

    fn handle_raw_price_ticker(&mut self, ts: u128, raw: String) -> Res {
        let data = &json::parse(&raw)?;
        let symbol = data["s"].as_str().expect("No symbol");
        let instrument_arc = self.instruments_map.get(&Exchange::Binance, symbol).expect(&format!("No instrument: {symbol}"));
        let price_ticker = DTO::PriceTicker(PriceTicker {
            timestamp: ts,
            instrument: Arc::clone(instrument_arc),
            bid: parse_f64_field(data, "b"),
            bid_amount: parse_f64_field(data, "B"),
            ask: parse_f64_field(data, "a"),
            ask_amount: parse_f64_field(data, "A"),
        });

        self.queue.push(price_ticker).expect("Can't add price ticker to queue");
        Ok(())
    }

    fn handle(&mut self) -> Res {
        loop {
            let msg = self.socket.as_mut().unwrap().read()?;
            let ts = time();
            match msg {
                Message::Text(raw) => {
                    // log::info!("Received message: {raw}");
                    self.latest_ticker_ts = ts;
                    self.handle_raw_price_ticker(ts, raw)?;
                }
                Message::Ping(payload) => {
                    self.handle_ping(ts, payload)?;
                }
                Message::Close(reason) => {
                    log::warn!("Got the close frame: {reason:?}");
                    return Ok(());
                }
                msg => log::warn!("Unexpected msg: {msg}")
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

    fn send_message_and_handle(&mut self, mut data: JsonValue, result_handler: fn(&PriceTickerStream, JsonValue)) -> Res {
        let id = self.next_id();
        data.insert("id", id)?;
        self.send_json(data)?;
        loop {
            let msg = self.socket.as_mut().unwrap().read();
            match msg {
                Ok(Message::Text(a)) => {
                    // log::info!("Message during sub: {a}");
                    if a.starts_with("{\"r") {
                        // log::info!("Got the result: {}", a);
                        let data = json::parse(&a).expect("Can't parse json");
                        let got_id = data["id"].as_usize().unwrap();
                        assert_eq!(id, got_id);
                        result_handler(self, data);
                        return Ok(());
                    }
                }
                Ok(Message::Ping(payload)) => {
                    self.handle_ping(time(), payload)?;
                },
                Err(err) => {
                    match err {
                        // Silently drop the error: Processing error: IO error: Resource temporarily unavailable (os error 11)
                        // That occurs when no messages are to be read
                        Error::Io(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            // thread::sleep(Duration::from_millis(100));
                        },
                        _ => return Err(err.into()),
                    }
                }

                t => log::warn!("Unexpected msg type: {t:?}")
            }
        }
    }
}
