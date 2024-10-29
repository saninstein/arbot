use std::{fs, io, thread};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::str::Utf8Error;
use std::sync::Arc;
use std::time::Duration;
use chrono::NaiveDateTime;
use ed25519_dalek::pkcs8::DecodePrivateKey;
use ed25519_dalek::{Signer, SigningKey};
use fefix::definitions::{fix44, HardCodedFixFieldDefinition};
use fefix::dict::{FieldLocation, FixDatatype};
use fefix::{Dictionary, FieldMap, FieldType, FieldValueError, RepeatingGroup, SetField, StreamingDecoder};
use fefix::field_types::Timestamp;
use fefix::tagvalue::{Decoder, DecoderStreaming, Encoder, EncoderHandle, Message};
use json::object;
use rustls::{ClientConnection, RootCertStore};
use rustls::Stream;
use uuid::Uuid;
use crate::core::api::OrderListener;
use crate::core::dto::{Order, OrderSide, OrderStatus, OrderType};
use crate::core::map::InstrumentsMap;
// struct OMS {
//     out_queue: Arc<ArrayQueue<DTO>>,
// }
//
//
// impl OMS {
//     fn on_order_execution(&mut self, order: Order) {
//         self.out_queue.push(DTO::Order(order)).unwrap()
//     }
//
//     fn tick() {
//         // do something when no new messages
//     }
// }
//
// impl OrderListener for OMS {
//     fn on_order(&mut self, order: &Order) {
//
//     }
// }

struct StreamTLS {
    stream: rustls::StreamOwned<ClientConnection, TcpStream>,
}

impl StreamTLS {
    pub fn new() -> Self {
        let hostname = "fix-oe.testnet.binance.vision";
        let port = 9000;
        let uri = format!("{hostname}:{port}");
        let root_store = RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.into(),
        };
        let mut config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        // Allow using SSLKEYLOGFILE.
        config.key_log = Arc::new(rustls::KeyLogFile::new());

        let server_name = hostname.try_into().unwrap();

        let mut conn = rustls::ClientConnection::new(Arc::new(config), server_name).unwrap();
        let mut sock = TcpStream::connect(uri).unwrap();

        let mut tls = rustls::StreamOwned::new(conn, sock);
        tls.sock.set_nonblocking(true).expect("set_nonblocking call failed");
        log::info!("Connected to {hostname}");
        Self { stream: tls }
    }

    fn send_message(&mut self, msg: &[u8]) {
        loop {
            let result = self.stream.write_all(msg);
            match result {
                Ok(()) => {
                    log::info!("Successfully wrote all data");
                    return;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // The socket is not ready to write yet
                    log::warn!("Socket is not ready to write yet, would block");
                    // You can retry in a loop or handle this with an event-driven approach
                    thread::sleep(Duration::from_secs(1));
                }
                Err(e) => {
                    // Handle other potential errors
                    log::error!("Failed to write to socket: {}", e);
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }
    }
}

pub enum ConnectionState { NotReady, Ready, ShouldReconnect }


struct FixMessageEncoderHandler {
    api_key: String,
    encoder: Encoder,
    signing_key: SigningKey,

    buffer: Vec<u8>,
    msg_seq_num: usize,
}

impl FixMessageEncoderHandler {
    fn start_message(&mut self, msg_type: &[u8]) -> EncoderHandle<Vec<u8>> {
        let sender_comp_id = "EXAMPLE2";
        let target_comp_id = "SPOT";

        let sending_time = Timestamp::utc_now();

        self.buffer.clear();
        let mut msg = self.encoder.start_message(b"FIX.4.4", &mut self.buffer, msg_type);
        msg.set(fix44::MSG_SEQ_NUM, self.msg_seq_num.clone());
        msg.set(fix44::SENDER_COMP_ID, sender_comp_id.clone());
        msg.set(fix44::SENDING_TIME, sending_time.clone());
        msg.set(fix44::TARGET_COMP_ID, target_comp_id.clone());

        self.msg_seq_num += 1;

        msg
    }

    fn create_logon_message(&mut self) -> &[u8] {
        let sender_comp_id = "EXAMPLE2";
        let target_comp_id = "SPOT";

        let sending_time = Timestamp::utc_now();

        self.buffer.clear();
        let mut msg = self.encoder.start_message(b"FIX.4.4", &mut self.buffer, b"A");
        msg.set(fix44::MSG_SEQ_NUM, self.msg_seq_num.clone());
        msg.set(fix44::SENDER_COMP_ID, sender_comp_id.clone());
        msg.set(fix44::SENDING_TIME, sending_time.clone());
        msg.set(fix44::TARGET_COMP_ID, target_comp_id.clone());

        // raw data

        let msg_to_sign = format!(
            "A\x01{}\x01{}\x01{}\x01{}",
            sender_comp_id.clone(), target_comp_id.clone(), self.msg_seq_num.clone(), sending_time.to_string()
        );
        let signature = self.signing_key.sign(msg_to_sign.as_bytes());
        let raw_data = base64::encode(&signature.to_bytes());


        msg.set(fix44::RAW_DATA_LENGTH, raw_data.len() as i64);
        msg.set(fix44::RAW_DATA, raw_data.as_bytes());
        msg.set(fix44::ENCRYPT_METHOD, 0);
        msg.set(fix44::HEART_BT_INT, 10);
        msg.set(fix44::RESET_SEQ_NUM_FLAG, true);
        msg.set(fix44::USERNAME, self.api_key.as_str());

        let MessageHandling: &HardCodedFixFieldDefinition = &HardCodedFixFieldDefinition {
            name: "MessageHandling",
            tag: 25035,
            data_type: FixDatatype::Int,
            location: FieldLocation::Body,
        };
        msg.set(MessageHandling, 2);

        self.msg_seq_num += 1;

        msg.done().0
    }

    fn create_heartbeat_message(&mut self, request_id: &str) -> &[u8] {
        let mut msg = self.start_message(b"0");
        msg.set(fix44::TEST_REQ_ID, request_id);

        msg.done().0
    }

    fn create_limit_message(&mut self) -> &[u8] {
        let mut msg = self.start_message(b"XLQ");
        pub const ReqID: &HardCodedFixFieldDefinition = &HardCodedFixFieldDefinition {
            name: "ReqID",
            tag: 6136,
            data_type: FixDatatype::String,
            location: FieldLocation::Body,
        };

        msg.set(ReqID, Uuid::new_v4().to_string().as_str());

        msg.done().0
    }

    fn create_order_message(&mut self, order: &Order) -> &[u8] {
        let mut msg = self.start_message(b"D");

        msg.set(fix44::CL_ORD_ID, order.client_order_id.as_str());

        if order.amount > 0. {
            msg.set(fix44::ORDER_QTY, order.amount);
        }

        let order_type = match order.order_type {
            OrderType::Market => fix44::OrdType::Market,
            OrderType::Limit => fix44::OrdType::Limit,
            OrderType::LimitMaker => fix44::OrdType::Market
        };

        msg.set(fix44::ORD_TYPE, order_type);


        if order.order_type == OrderType::Limit {
            msg.set(fix44::PRICE, order.price);
        }

        let side = match order.side {
            OrderSide::Buy => fix44::Side::Buy,
            OrderSide::Sell => fix44::Side::Sell
        };

        msg.set(fix44::SIDE, side);
        msg.set(fix44::SYMBOL, order.instrument.symbol.as_str());

        if order.amount_quote > 0. {
            msg.set(fix44::CASH_ORDER_QTY, order.amount_quote);
        }
        msg.done().0
    }
}

pub struct BinanceFixConnection {
    stream: StreamTLS,
    encoder: FixMessageEncoderHandler,
    pub state: ConnectionState,
    decoder: DecoderStreaming<Vec<u8>>,
    instruments_map: Arc<InstrumentsMap>,
}


impl BinanceFixConnection {
    pub fn new(
        spec_path: &str,
        signing_key_path: &str,
        api_key: &str,
        instruments_map: Arc<InstrumentsMap>,
    ) -> Self {
        let spec = fs::read_to_string(spec_path.to_string()).unwrap();

        let dictionary = Dictionary::from_quickfix_spec(&spec).unwrap();
        let mut decoder = Decoder::new(dictionary).streaming(vec![]);
        Self {
            stream: StreamTLS::new(),
            decoder: decoder,
            encoder: FixMessageEncoderHandler {
                encoder: Encoder::default(),
                signing_key: SigningKey::read_pkcs8_pem_file(Path::new(signing_key_path)).unwrap(),
                buffer: Default::default(),
                msg_seq_num: 1,
                api_key: api_key.to_string(),
            },
            state: ConnectionState::NotReady,
            instruments_map,
        }
    }

    pub fn logon(&mut self) {
        let msg = self.encoder.create_logon_message();
        self.stream.send_message(msg);
    }


    pub fn tick(&mut self) {
        Self::handle_incoming_message(
            &mut self.stream,
            &mut self.decoder,
            &mut self.encoder,
            &self.instruments_map,
        )
    }

    pub fn execution_report_to_order(msg: Message<&[u8]>, instruments_map: &Arc<InstrumentsMap>) -> Order {
        let mut order = Order::new();

        match msg.get(fix44::ORIG_CL_ORD_ID) {
            Ok(value) => {
                order.client_order_id = String::from_utf8_lossy(value).to_string();
            }
            Err(FieldValueError::Missing) => {
                log::warn!("fix44::ORIG_CL_ORD_ID missed");
            }
            _ => panic!("fix44::ORIG_CL_ORD_ID err")
        }

        match msg.get(fix44::ORDER_ID) {
            Ok(value) => {
                order.exchange_order_id = String::from_utf8_lossy(value).to_string();
            }
            Err(FieldValueError::Missing) => {
                log::warn!("fix44::ORDER_ID missed");
            }
            _ => panic!("fix44::ORDER_ID")
        }

        match msg.get(fix44::ORDER_QTY) {
            Ok(value) => {
                order.amount = value;
            }
            Err(FieldValueError::Missing) => {
                log::warn!("fix44::ORDER_QTY missed");
            }
            _ => panic!("fix44::ORDER_QTY")
        }

        match msg.get(fix44::CASH_ORDER_QTY) {
            Ok(value) => {
                order.amount_quote = value;
            }
            Err(FieldValueError::Missing) => {
                log::warn!("fix44::CASH_ORDER_QTY missed");
            }
            _ => panic!("fix44::CASH_ORDER_QTY")
        }


        order.order_type = match msg.get(fix44::ORD_TYPE).unwrap() {
            fix44::OrdType::Market => OrderType::Market,
            fix44::OrdType::Limit => OrderType::Limit,
            _ => panic!("Unexpected order type"),
        };
        order.side = match msg.get(fix44::SIDE).unwrap() {
            fix44::Side::Buy => OrderSide::Buy,
            fix44::Side::Sell => OrderSide::Sell,
            _ => panic!("Unexpected order side"),
        };


        let symbol = String::from_utf8_lossy(msg.get(fix44::SYMBOL).unwrap()).to_string();
        order.instrument = Arc::clone(
            instruments_map.map.get(&symbol).unwrap()
        );

        match msg.get(fix44::PRICE) {
            Ok(value) => {
                order.price = value;
            }
            Err(FieldValueError::Missing) => {
                log::warn!("fix44::PRICE missed");
            }
            _ => panic!("fix44::PRICE")
        }

        let ts = String::from_utf8_lossy(msg.get_raw(fix44::TRANSACT_TIME).unwrap());
        let naive_datetime = NaiveDateTime::parse_from_str(&ts, "%Y%m%d-%H:%M:%S%.f").unwrap();

        order.timestamp = naive_datetime.and_utc().timestamp_nanos_opt().unwrap() as u128;

        order.status = match msg.get(fix44::ORD_STATUS).unwrap() {
            fix44::OrdStatus::New => OrderStatus::New,
            fix44::OrdStatus::PartiallyFilled => OrderStatus::PartiallyFilled,
            fix44::OrdStatus::Filled => OrderStatus::Filled,
            fix44::OrdStatus::Canceled => OrderStatus::Canceled,
            fix44::OrdStatus::PendingCancel => OrderStatus::Canceling,
            fix44::OrdStatus::Rejected => OrderStatus::Error,
            fix44::OrdStatus::PendingNew => OrderStatus::ScheduledSent,
            fix44::OrdStatus::Expired => OrderStatus::Error,
            _ => panic!("Unexpected order status"),
        };
        order.amount_filled = msg.get(fix44::CUM_QTY).unwrap();

        if order.status == OrderStatus::Error {
            order.error = String::from_utf8_lossy(msg.get(fix44::TEXT).unwrap()).parse().unwrap();
        }

        match msg.group(fix44::NO_MISC_FEES) {
            Ok(fees_group) => {
                for i in 0..fees_group.len() {
                    let fee_data = fees_group.get(i).unwrap();
                    order.fees.push(
                        (
                            fee_data.get::<&str>(fix44::MISC_FEE_CURR).unwrap().parse().unwrap(),
                            fee_data.get(fix44::MISC_FEE_AMT).unwrap()
                        )
                    )
                }
            }
            Err(FieldValueError::Missing) => {
                log::warn!("fix44::NO_MISC_FEES missed");
            }
            _ => panic!("fix44::NO_MISC_FEES")
        }

        order
    }

    fn handle_incoming_message(stream: &mut StreamTLS, decoder: &mut DecoderStreaming<Vec<u8>>, encoder: &mut FixMessageEncoderHandler, instruments_map: &Arc<InstrumentsMap>) {
        match stream.stream.read_exact(decoder.fillable()) {
            Ok(_) => {
                // Successfully filled the buffer.
                match decoder.try_parse() {
                    Ok(Some(_)) => {
                        // Successfully parsed a message.
                        let msg = decoder.message();
                        log::info!("{}", String::from_utf8_lossy(msg.as_bytes()));

                        match msg.get(fix44::MSG_TYPE) {
                            Ok(fix44::MsgType::ExecutionReport) => {
                                log::info!("Handle:Execution report");
                                // [2024-10-23T20:52:04Z INFO  untitled::core::oms] 8=FIX.4.49=000031335=849=SPOT56=EXAMPLE234=352=20241023-20:52:04.02209317=2461233511=dummy37=1105758938=0.0001000040=154=155=BTCUSDT59=160=20241023-20:52:04.02100025018=20241023-20:52:04.02100025001=3150=014=0.00000000151=0.0001000025017=0.000000001057=Y32=0.0000000039=0636=Y25023=20241023-20:52:04.02100010=218

                                let order = Self::execution_report_to_order(msg, instruments_map);
                                log::info!("Got order {order:?}");
                            }
                            Ok(fix44::MsgType::Reject) => {
                                panic!("Handle:Reject");
                            }
                            Ok(fix44::MsgType::Logon) => {
                                log::info!("Handle:Logon");
                                let msg = encoder.create_limit_message();
                                stream.send_message(msg);
                                log::info!("Limit check sent");
                            }
                            Ok(fix44::MsgType::Logout) => {
                                log::info!("Handle:Logout");
                                panic!("Logout");
                            }
                            Ok(fix44::MsgType::Heartbeat) => {
                                log::info!("Handle:HEARTBEAT");
                            }
                            Ok(fix44::MsgType::TestRequest) => {
                                log::info!("Handle:TEST REQUEST");
                                let request_id = String::from_utf8_lossy(msg.get(fix44::TEST_REQ_ID).unwrap());
                                let msg = encoder.create_heartbeat_message(&request_id);
                                stream.send_message(msg);
                                log::info!("Heartbeat sent");
                            }
                            Err(FieldValueError::Invalid(_)) if matches!(msg.get_raw(35), Some(b"XLR")) => {
                                log::info!("Handle:XLR");
                                let limits_group = msg.group(25003).unwrap();
                                for i in 0..limits_group.len() {
                                    let limit_data = limits_group.get(i).unwrap();
                                    let limit_type = match limit_data.get::<&str>(25004) {
                                        Ok("1") => "ORDER",
                                        Ok("2") => "MSG",
                                        Err(_) => {
                                            panic!("Invalid type");
                                        }
                                        t => {
                                            panic!("Unknown type: {t:?}");
                                        }
                                    };
                                    let current_count = limit_data.get::<usize>(25005).unwrap();
                                    let max = limit_data.get::<usize>(25006).unwrap();
                                    let reset_interval = limit_data.get::<usize>(25007).unwrap();
                                    let reset_interval_resolution = limit_data.get::<&str>(25008).unwrap();

                                    log::info!("XLR:{limit_type} {current_count}/{max} reset: {reset_interval}{reset_interval_resolution}");

                                }

                                // Create order

                                log::info!("Create order^^");
                                let mut order = Order::new();
                                order.instrument = instruments_map.map.get("BTCUSDT").unwrap().clone();
                                order.amount = 0.0001;
                                order.side = OrderSide::Buy;
                                order.client_order_id = "dummy".to_string();


                                let msg = encoder.create_order_message(&order);
                                stream.send_message(msg);
                            }
                            t => {
                                log::warn!("Unknown message type {t:?}");
                            }
                        }
                        decoder.clear(); // Clear the decoder for the next message.
                    }
                    Ok(None) => {
                        log::info!("Still parsing message");
                    }
                    Err(e) => {
                        log::error!("Decode error: {}", e);
                        panic!("Decode error: {}", e);
                    }
                }
            }
            Err(ref e)  if e.kind() == std::io::ErrorKind::WouldBlock => {
                // We hit the end of the current stream buffer, but the connection is still open.
                log::info!("WouldBlock reached, waiting for more data...");
                decoder.clear();
                thread::sleep(Duration::from_secs(1));
            }
            Err(e) => {
                // Handle other types of errors.
                log::error!("TLS read error: {}", e);
                thread::sleep(Duration::from_secs(1));
            }
        }
    }
}


impl OrderListener for BinanceFixConnection {
    fn on_order(&mut self, order: &Order) {
        //
    }
}
