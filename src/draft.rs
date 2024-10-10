#[cfg(test)]
mod tests {



    // #[test]
    // fn it_works() {
    //     let id = Uuid::new_v4().to_string();
    //     println!("{id}");
    // }

    use std::fs;
    use std::fs::File;
    use std::io::{stdout, Cursor, Read, Write};
    use std::net::TcpStream;
    use std::path::Path;
    use std::sync::Arc;
    use ed25519_dalek::pkcs8::DecodePrivateKey;
    use ed25519_dalek::{Signer, SigningKey};
    use fefix::definitions::{fix44, HardCodedFixFieldDefinition};
    use fefix::dict::{FieldLocation, FixDatatype};
    use fefix::field_types::Timestamp;
    use fefix::{Dictionary, FieldMap, FieldType, GetConfig, SetField, StreamingDecoder};
    use fefix::tagvalue::{Decoder, Encoder};
    use rustls::RootCertStore;
    use crate::core::utils::time;

    #[test]
    fn test_decode() {
        let message = b"8=FIX.4.4|9=330|35=8|34=2|49=SPOT|52=20240611-09:01:46.228950|56=qNXO12fH|11=1718096506197867067|14=0.00000000|17=144|32=0.00000000|37=76|38=5.00000000|39=0|40=2|44=10.00000000|54=1|55=LTCBNB|59=4|60=20240611-09:01:46.228000|150=0|151=5.00000000|636=Y|1057=Y|25001=1|25017=0.00000000|25018=20240611-09:01:46.228000|25023=20240611-09:01:46.228000|10=095|";

        let spec = fs::read_to_string("binance-spot-fix-oe.xml".to_string()).unwrap();

        let dictionary = Dictionary::from_quickfix_spec(&spec).unwrap();
        let mut decoder = Decoder::new(dictionary);
        decoder.config_mut().separator = b'|';

        let ts = time();
        let res = decoder.decode(message).unwrap();
        let ts_end = time();

        let diff = ts_end - ts;
        println!("{diff}");

        //
        let symbol: &str = res.get(fix44::SYMBOL).unwrap();
        println!("{:#?}", symbol);

        let msg_type: &str = res.get(fix44::MSG_TYPE).unwrap();
        println!("{:#?}", msg_type);
    }

    fn get_msg() -> Vec<u8> {
        let mut encoder = Encoder::default();
        let mut buffer = Vec::new();
        let ts = time();
        let secs = ts / 1_000_000_000;
        let nsecs = (ts % 1_000_000_000) as u32;
        // let datetime = DateTime::from_timestamp(secs as i64, nsecs).unwrap();

        let sender_comp_id = "EXAMPLE2";
        let target_comp_id = "SPOT";
        let msg_seq_num = 1;

        let sending_time = Timestamp::utc_now();

        // println!("{:?}", sending_time.to_string());

        // Convert the buffer into a SecretKey
        let signing_key = SigningKey::read_pkcs8_pem_file(Path::new("/Users/alex/RustroverProjects/untitled/.creds/binance.pem")).unwrap();

        let msg = format!(
            "A\x01{}\x01{}\x01{}\x01{}",
            sender_comp_id.clone(), target_comp_id.clone(), msg_seq_num.clone(), sending_time.to_string()
        );

        // println!("{msg}");

        // assert_eq!(b"A\x011\x01SPOT\x011\x0120241010-10:47:10.550", msg.as_bytes());

        let signature = signing_key.sign(msg.as_bytes());
        // let raw_data:  [u8; SIGNATURE_LENGTH]  = signature.to_bytes();
        let raw_data = base64::encode(&signature.to_bytes());

        pub const MessageHandling: &HardCodedFixFieldDefinition = &HardCodedFixFieldDefinition {
            name: "MessageHandling",
            tag: 25035,
            data_type: FixDatatype::Int,
            location: FieldLocation::Body,
        };

        // pub const RecvWindow: &HardCodedFixFieldDefinition = &HardCodedFixFieldDefinition {
        //     name: "RecvWindow",
        //     tag: 25000,
        //     is_group_leader: false,
        //     data_type: FixDatatype::Int,
        //     location: FieldLocation::Body,
        // };

        let mut msg = encoder.start_message(b"FIX.4.4", &mut buffer, b"A");
        msg.set(fix44::MSG_SEQ_NUM, msg_seq_num.clone());
        msg.set(fix44::SENDER_COMP_ID, sender_comp_id.clone());
        msg.set(fix44::SENDING_TIME, sending_time.clone());
        msg.set(fix44::TARGET_COMP_ID, target_comp_id.clone());
        // msg.set(RecvWindow, dt.clone());


        msg.set(fix44::RAW_DATA_LENGTH, raw_data.len() as i64);
        msg.set(fix44::RAW_DATA, raw_data.as_bytes());
        msg.set(fix44::ENCRYPT_METHOD, 0);
        msg.set(fix44::HEART_BT_INT, 5);
        msg.set(fix44::RESET_SEQ_NUM_FLAG, true);
        msg.set(fix44::USERNAME, "7YPfVLXzckzQyMnWicLQiWEyhiOPJwGCLR27ErnbhsJUPKO3TnfT9N28YU9qePSX");


        msg.set(MessageHandling, 2);

        let (r, size) = msg.done();
        println!("{:?}", String::from_utf8_lossy(&r));

        // while r[12] == 48 {
        //     r.remove(12);
        // }
        //
        // r.truncate(r.len() - 7);
        //
        //
        //
        // let checksum = CheckSum::compute(&r);
        // println!("{checksum:?}");
        // 10.serialize(&mut r);
        // r.extend_from_slice(b"=" as &[u8]);
        // checksum.serialize(&mut r);
        // r.extend_from_slice(b"|" as &[u8]);

        // println!("\"{}\"", String::from_utf8_lossy(&r));
        // println!("{:?}", "8=FIX.4.4|9=247|35=A|34=1|49=EXAMPLE|52=20240627-11:17:25.223|56=SPOT|95=88|96=4MHXelVVcpkdwuLbl6n73HQUXUf1dse2PCgT1DYqW9w8AVZ1RACFGM+5UdlGPrQHrgtS3CvsRURC1oj73j8gCA==|98=0|108=30|141=Y|553=sBRXrJx2DsOraMXOaUovEhgVRcjOvCtQwnWj8VxkOh1xqboS02SPGfKi2h8spZJb|25035=2|10=227|");

        return r.to_vec();
        // return Vec::from(r);
    }

    #[test]
    fn test_get_msg() {
        get_msg();
    }

    fn get_msg_p() -> Vec<u8> {
        let mut file = File::open("/Users/alex/PycharmProjects/trading-anal/message.bin").unwrap();

        // Create a new Vec<u8> to hold the data
        let mut buffer = Vec::new();

        // Read the file's contents into the buffer
        file.read_to_end(&mut buffer).unwrap();
        return buffer;
    }

    #[test]
    fn limitedclient_rs() {
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
        let mut tls = rustls::Stream::new(&mut conn, &mut sock);
        tls.write_all(&get_msg()).unwrap();
        //     concat!(
        //     "GET / HTTP/1.1\r\n",
        //     "Host: www.rust-lang.org\r\n",
        //     "Connection: close\r\n",
        //     "Accept-Encoding: identity\r\n",
        //     "\r\n"
        //     )
        //         .as_bytes(),
        // )

        let ciphersuite = tls
            .conn
            .negotiated_cipher_suite()
            .unwrap();
        println!("ciphersuite {:?}", ciphersuite);


        let spec = fs::read_to_string("binance-spot-fix-oe.xml".to_string()).unwrap();

        let dictionary = Dictionary::from_quickfix_spec(&spec).unwrap();
        let mut decoder = Decoder::new(dictionary).streaming(vec![]);
        decoder.config_mut().separator = b'|';

        loop {
            // You *must* use `std::io::Read::read_exact`.
            tls.read_exact(decoder.fillable()).unwrap();
            match decoder.try_parse().unwrap() {
                Some(_) => {
                    // we have successfully parsed a message
                    let msg = decoder.message();
                    println!("{}", String::from_utf8_lossy(msg.as_bytes()));
                    // need to clear the decoder to to begin parsing next mesage
                    decoder.clear();
                    // break;
                }
                None => {
                    println!("still-parsing-message");
                }
            }
        }

        // let mut plaintext = Vec::new();
        // tls.read_to_end(&mut plaintext).unwrap();


        // println!("{}", String::from_utf8_lossy(&plaintext));
    }

    // #[test]
    // fn anoter_fix() {
    //     let hostname = "fix-oe.testnet.binance.vision";
    //     let port = 9000;
    //     let uri = format!("{hostname}:{port}");
    //
    //     let mut config = rustls::ClientConfig::new();
    //     let cert_file = &mut BufReader::new(File::open("/Users/alex/PycharmProjects/trading-anal/cacert.pem").unwrap());
    //     config
    //         .root_store
    //         .add_pem_file(cert_file);
    //
    //     let dns_name = webpki::DNSNameRef::try_from_ascii_str(hostname).unwrap();
    //     let mut sess = rustls::ClientSession::new(&Arc::new(config), dns_name);
    //     let mut sock = TcpStream::connect(uri).unwrap();
    //     let mut tls = rustls::Stream::new(&mut sess, &mut sock);
    //
    //     let mut file = File::open("/Users/alex/PycharmProjects/trading-anal/message.bin").unwrap();
    //
    //     // Create a new Vec<u8> to hold the data
    //     let mut buffer = Vec::new();
    //
    //     // Read the file's contents into the buffer
    //     file.read_to_end(&mut buffer).unwrap();
    //
    //     println!("{:?}", String::from_utf8_lossy(&buffer));
    //
    //     tls.write(&buffer).unwrap();
    //     tls.flush().unwrap();
    //     thread::sleep(Duration::from_secs(1));
    //     let ciphersuite = tls.sess.get_negotiated_ciphersuite().unwrap();
    //     writeln!(&mut std::io::stderr(), "Current ciphersuite: {:?}", ciphersuite.suite).unwrap();
    //     let mut plaintext = Vec::new();
    //     tls.read_to_end(&mut plaintext).unwrap();
    //     println!("{}", String::from_utf8_lossy(&plaintext));
    // }

    // #[test]
    // fn test_fix() {
    //     let mut stream = std::net::TcpStream::connect("localhost:3000").unwrap();
    //     // let mut config = rustls::ClientConfig::new();
    //     // let cert_file = &mut BufReader::new(File::open("/Users/alex/RustroverProjects/untitled/.creds/cacert.pem").unwrap());
    //     // config
    //     //     .root_store
    //     //     .add_pem_file(cert_file);
    //     // let arc = std::sync::Arc::new(config);
    //     // let dns_name = webpki::DNSNameRef::try_from_ascii_str("fix-oe.testnet.binance.vision").unwrap();
    //     // let mut client = rustls::ClientSession::new(&arc, dns_name);
    //     // let mut stream = rustls::Stream::new(&mut client, &mut socket); // Create stream
    //     // Instead of writing to the client, you write to the stream
    //     let mut file = File::open("/Users/alex/PycharmProjects/trading-anal/message.bin").unwrap();
    //
    //     // Create a new Vec<u8> to hold the data
    //     let mut buffer = Vec::new();
    //     stream
    //         .write(&buffer)
    //         // .write(b"8=FIX.4.4|9=247|35=A|34=1|49=EXAMPLE|52=20240627-11:17:25.223|56=SPOT|95=88|96=4MHXelVVcpkdwuLbl6n73HQUXUf1dse2PCgT1DYqW9w8AVZ1RACFGM+5UdlGPrQHrgtS3CvsRURC1oj73j8gCA==|98=0|108=30|141=Y|553=sBRXrJx2DsOraMXOaUovEhgVRcjOvCtQwnWj8VxkOh1xqboS02SPGfKi2h8spZJb|25035=2|10=227|")
    //         .unwrap();
    //     let mut plaintext = Vec::new();
    //     let r = stream.peer_addr().unwrap();
    //     println!("{:?}", r);
    //     stream.read_to_end(&mut plaintext).unwrap();
    //     println!("plaintext: {:?}", String::from_utf8_lossy(&plaintext));
    //     // stream.read_to_end(&mut plaintext).unwrap();
    //     // thread::sleep(Duration::from_secs(5));
    //     // println!("plaintext: {:?}", String::from_utf8_lossy(&plaintext));
    // }
    // #[test]
    // fn test_rustls() {
    //     let mut socket = std::net::TcpStream::connect("fix-oe.testnet.binance.vision:9000").unwrap();
    //     let mut config = rustls::ClientConfig::new();
    //     let cert_file = &mut BufReader::new(File::open("/Users/alex/RustroverProjects/untitled/.creds/cacert.pem").unwrap());
    //     config
    //         .root_store
    //         .add_pem_file(cert_file);
    //     let arc = std::sync::Arc::new(config);
    //     let dns_name = webpki::DNSNameRef::try_from_ascii_str("fix-oe.testnet.binance.vision").unwrap();
    //     let mut client = rustls::ClientSession::new(&arc, dns_name);
    //     let mut stream = rustls::Stream::new(&mut client, &mut socket); // Create stream
    //     // Instead of writing to the client, you write to the stream
    //     stream
    //         .write(b"8=FIX.4.4\x019=247\x0135=A\x0134=1\x0149=EXAMPLE2\x0152=20241010-15:49:38.085\x0156=SPOT\x0195=88\x0196=Oq4eS/gQdvXEGQa2GSycpw5FECHAI9AwyrFKtk4QxVDJrRz/sAHLXbSiA7fqhTwODKRcpxt+JoBObNqiYoyxDQ==\x0198=0\x01108=5\x01141=Y\x01553=7YPfVLXzckzQyMnWicLQiWEyhiOPJwGCLR27ErnbhsJUPKO3TnfT9N28YU9qePSX\x0125035=1\x0110=142\x01")
    //         .unwrap();
    //     let mut plaintext = Vec::new();
    //     stream.read_to_end(&mut plaintext).unwrap();
    //     println!("plaintext: {:?}", String::from_utf8_lossy(&plaintext));
    // }
    //
    // #[test]
    // fn test_rustls2() {
    //     let mut socket = std::net::TcpStream::connect("www.google.com:443").unwrap();
    //     let mut config = rustls::ClientConfig::new();
    //     let cert_file = &mut BufReader::new(File::open("/Users/alex/RustroverProjects/untitled/.creds/cacert.pem").unwrap());
    //     config
    //         .root_store
    //         .add_pem_file(cert_file).unwrap();
    //     let arc = std::sync::Arc::new(config);
    //     let dns_name = webpki::DNSNameRef::try_from_ascii_str("www.google.com").unwrap();
    //     let mut client = rustls::ClientSession::new(&arc, dns_name);
    //     let mut stream = rustls::Stream::new(&mut client, &mut socket); // Create stream
    //     // Instead of writing to the client, you write to the stream
    //     stream
    //         .write(b"GET / HTTP/1.1\r\nConnection: close\r\n\r\n")
    //         .unwrap();
    //     let mut plaintext = Vec::new();
    //     stream.read_to_end(&mut plaintext).unwrap();
    //     println!("plaintext: {:?}", String::from_utf8_lossy(&plaintext));
    // }
}
