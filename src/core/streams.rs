use std::net::TcpStream;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
use crossbeam_queue::ArrayQueue;
use itertools::Itertools;
use json::{object, JsonValue};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, Message, WebSocket};
use crate::core::{
    dto::PriceTicker,
    map::InstrumentsMap,
    utils::{parse_f64_field, time}
};

#[allow(dead_code)]
pub struct PriceTickerStream {
    queue: Arc<ArrayQueue<PriceTicker>>,
    instruments_map: Arc<InstrumentsMap>,
    channels: Vec<String>,
    channels_per_request: usize,

    socket: WebSocket<MaybeTlsStream<TcpStream>>,

    request_latency: u64,
    request_latest_ts: Arc<RwLock<u128>>,
    request_id: usize,
}

#[allow(dead_code)]
impl PriceTickerStream {
    pub fn new(
        queue: Arc<ArrayQueue<PriceTicker>>,
        channels: Vec<String>,
        instruments_map: Arc<InstrumentsMap>,
        channels_per_request: usize,
        request_latency: u64,
        request_latest_ts: Arc<RwLock<u128>>,
    ) -> Self {
        let (socket, response) = connect("wss://data-stream.binance.vision/stream").expect("Can't connect");

        log::info!("Connected to the server. Response HTTP code: {}", response.status());

        Self {
            queue,
            instruments_map,
            channels_per_request,
            channels,
            request_latency,
            request_latest_ts,
            socket,
            request_id: 0,
        }
    }
    pub fn send_json(&mut self, data: JsonValue) {
        let mut ts = self.request_latest_ts.write().expect("Can't get the lock");
        self.socket.send(Message::Text(json::stringify(data))).expect("Can't send the data");
        thread::sleep(Duration::from_millis(250));
        *ts = time();
    }

    pub fn run(&mut self) {
        for (i, items) in self.channels.clone()
            .chunks(self.channels_per_request)
            .into_iter()
            .enumerate()
        {
            log::info!("Subscribe the batch: {i}");
            self.send_message_and_handle(
                object! {method: "SUBSCRIBE", params: items},
                |data: JsonValue| assert!(data["result"].is_null()),
            );
            log::info!("Subscribed the batch: {i}");
        }

        self.send_message_and_handle(
            object! {method: "LIST_SUBSCRIPTIONS"},
            |data| assert!(!data["result"].is_null()),
        );

        log::info!("Subscription done");

        self.handle();
    }

    fn handle(&mut self) {
        loop {
            let msg = self.socket.read().expect("Error reading message");
            let ts = time();
            match msg {
                Message::Text(a) => {
                    let object = json::parse(&a).expect("Can't parse json");
                    let data = &object["data"];
                    let instrument_arc = self.instruments_map.map.get(data["s"].as_str().expect("No symbol")).expect("No instrument");
                    let price_ticker = PriceTicker {
                        timestamp: ts,
                        instrument: Arc::clone(instrument_arc),
                        bid: parse_f64_field(data, "b"),
                        bid_amount: parse_f64_field(data, "B"),
                        ask: parse_f64_field(data, "a"),
                        ask_amount: parse_f64_field(data, "A"),
                    };

                    self.queue.push(price_ticker).expect("Can't add price ticker to queue");
                }
                Message::Ping(_) => {
                    // log::info!("Ping received");
                    self.flush_socket();
                }
                msg => log::warn!("Unexpected msg: {msg}")
            }
        }
    }

    pub fn next_id(&mut self) -> usize {
        let id = self.request_id.clone();
        self.request_id += 1;
        id
    }

    fn flush_socket(&mut self) {
        self.socket.flush().expect("Can't flush")
    }

    fn send_message_and_handle(&mut self, mut data: JsonValue, result_handler: fn(JsonValue)) {
        let id = self.next_id();
        data.insert("id", id).expect("Can't insert data");
        self.send_json(data);

        loop {
            let msg = self.socket.read().expect("Error reading message");
            match msg {
                Message::Text(a) => {
                    log::debug!("Message during sub: {a}");
                    if a.contains("data") {
                        continue;
                    }
                    // log::info!("Message during sub: {a}");
                    if a.contains("result") {
                        // log::info!("Got the result: {}", a);
                        let data = json::parse(&a).expect("Can't parse json");
                        let got_id = data["id"].as_usize().unwrap();
                        assert!(id == got_id);
                        result_handler(data);
                        return;
                    }
                }
                Message::Ping(_) => {
                    self.flush_socket()
                }
                _ => log::warn!("Unexpected msg type")
            }
        }
    }
}


pub fn listen_tickers(
    queue: Arc<ArrayQueue<PriceTicker>>,
    tickers: Vec<&str>,
    instruments_map: Arc<InstrumentsMap>,
    channels_per_stream: usize,
    channels_per_request: usize,
) {
    let request_latest_ts = Arc::new(RwLock::new(0));

    let mut sockets_count = 0;
    for (i, channels) in tickers.iter()
        .map(|x| {
            let mut s = x.to_lowercase().replace("/", "");
            s.push_str("@bookTicker");
            s
        })
        .chunks(channels_per_stream)
        .into_iter()
        .map(|chunk| chunk.collect_vec())
        .enumerate() {

        let queue_ref = Arc::clone(&queue);
        let instruments_map_ref = Arc::clone(&instruments_map);
        let request_latest_ts_ref = Arc::clone(&request_latest_ts);

        thread::Builder::new().name(format!("stream_pt_{i}")).spawn(move || {
            PriceTickerStream::new(
                queue_ref,
                channels,
                instruments_map_ref,
                channels_per_request,
                250,
                request_latest_ts_ref
            ).run()
        }).expect("Failed to spawn price ticker thread");
        sockets_count += 1;
    }
    log::info!("Inited tickers streams: {sockets_count}");
}
