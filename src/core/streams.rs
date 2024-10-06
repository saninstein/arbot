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
use tungstenite::stream::{MaybeTlsStream, NoDelay};
use tungstenite::{connect, Error, Message, WebSocket};
use crate::core::{
    dto::PriceTicker,
    map::InstrumentsMap,
    utils::{parse_f64_field, time},
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

    pub fn ticker_to_channel(ticker: &String) -> String {
        let mut s = ticker.to_lowercase().replace("/", "");
        s.push_str("@bookTicker");
        s
    }

    pub fn listen_from_tickers_split(
        queue: Arc<ArrayQueue<PriceTicker>>,
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
        queue: Arc<ArrayQueue<PriceTicker>>,
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

    fn spawn_stream(socket_id: usize, channels: Vec<String>, queue: &Arc<ArrayQueue<PriceTicker>>, instruments_map: &Arc<InstrumentsMap>, channels_per_request: usize, request_latest_ts: &Arc<RwLock<u128>>) {
        let queue_ref = Arc::clone(queue);
        let instruments_map_ref = Arc::clone(instruments_map);
        let request_latest_ts_ref = Arc::clone(request_latest_ts);

        thread::Builder::new().name(format!("stream_pt_{socket_id}")).spawn(move || {
            Self::new(
                queue_ref,
                channels,
                instruments_map_ref,
                channels_per_request,
                250,
                request_latest_ts_ref,
            ).run()
        }).expect("Failed to spawn price ticker thread");
    }

    pub fn send_json(&mut self, data: JsonValue) {
        let mut ts = self.request_latest_ts.write().expect("Can't get the lock");
        self.socket.send(Message::Text(json::stringify(data))).expect("Can't send the data");
        thread::sleep(Duration::from_millis(250));
        *ts = time();
    }

    pub fn run(&mut self) {
        self.subscribe();
        log::info!("Subscription done");
        self.handle();
    }

    pub fn subscribe(&mut self) {
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
        let object = json::parse(&raw).expect("Can't parse json");
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

    fn handle(&mut self) {
        loop {
            let msg = self.socket.read().expect("Error reading message");
            let ts = time();
            match msg {
                Message::Text(raw) => {
                    self.handle_raw_price_ticker(ts, raw);
                }
                Message::Ping(_) => {
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

    fn send_message_and_handle(&mut self, mut data: JsonValue, result_handler: fn(&PriceTickerStream, JsonValue)) {
        let id = self.next_id();
        data.insert("id", id).expect("Can't insert data");
        self.send_json(data);

        loop {
            let msg = self.socket.read().expect("Error reading message");
            match msg {
                Message::Text(a) => {
                    // log::info!("Message during sub: {a}");

                    if a.contains("\"data\"") {
                        continue;
                    }
                    // log::info!("Message during sub: {a}");
                    if a.contains("\"result\"") {
                        // log::info!("Got the result: {}", a);
                        let data = json::parse(&a).expect("Can't parse json");
                        let got_id = data["id"].as_usize().unwrap();
                        assert!(id == got_id);
                        result_handler(self, data);
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

pub fn dump_freq(
    path: String,
    tickers: Vec<String>,
    instruments_map: Arc<InstrumentsMap>,
) {
    let request_latest_ts = Arc::new(RwLock::new(0));
    let queue = Arc::new(ArrayQueue::new(100_000));
    for (i, channels) in tickers.iter()
        .map(PriceTickerStream::ticker_to_channel)
        .chunks(10)
        .into_iter()
        .map(|chunk| chunk.collect_vec())
        .enumerate() {
        if i < 9 {
            continue;
        }
        log::info!("Process: {channels:?}");
        // if i == 13 {
        //     println!("{channels:?}")
        // }
        // continue;

        let queue_ref = Arc::clone(&queue);
        let instruments_map_ref = Arc::clone(&instruments_map);
        let request_latest_ts_ref = Arc::clone(&request_latest_ts);

        let t = thread::Builder::new().spawn(move || {
            let mut stream = PriceTickerStream::new(
                queue_ref,
                channels,
                instruments_map_ref,
                25,
                250,
                request_latest_ts_ref,
            );
            stream.subscribe();
            let ts_start = time();
            let ts_end = ts_start + Duration::from_secs(10).as_nanos();

            match stream.socket.get_mut() {
                MaybeTlsStream::NativeTls(t) => {
                    // -- use either one or another
                    //t.get_mut().set_nonblocking(true);
                    t.get_mut().set_read_timeout(Some(Duration::from_millis(100))).expect("Error: cannot set read-timeout to underlying stream");
                },
                // handle more cases as necessary, this one only focuses on native-tls
                _ => unimplemented!()
            }


            while time() < ts_end {
                match stream.socket.read() {
                    Ok(Message::Text(raw)) => {
                        stream.handle_raw_price_ticker(time() - ts_start, raw)
                    },
                    Err(err) => {
                        match err {
                            // Silently drop the error: Processing error: IO error: Resource temporarily unavailable (os error 11)
                            // That occurs when no messages are to be read
                            Error::Io(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                // log::info!("Would block");
                            },
                            _ => panic!("{}", err),
                        }
                    },
                    _ => {}
                }

            }
        }).expect("Failed to spawn price ticker thread");

        let mut v = Vec::new();
        while !t.is_finished() || !queue.is_empty() {
            match queue.pop() {
                Some(price_ticker) => {
                    let base = price_ticker.instrument.base.clone();
                    let quote = price_ticker.instrument.quote.clone();
                    v.push(object! {ts: price_ticker.timestamp.to_string(), symbol: format!("{base}/{quote}")});
                }
                None => {
                    // log::info!("Empty queue");
                    thread::sleep(Duration::from_nanos(10));
                    // log::info!("Queue size {}", queue.len());
                }
            }
        }

        fs::write(Path::new(&path).join(format!("{i}.json")), json::stringify(v)).expect("Can't write json");
        log::info!("Done {i}")
    }
}
