extern crate bytes;
extern crate env_logger;
#[macro_use]
extern crate log;
extern crate proto;
#[macro_use]
extern crate proto_derive;

pub mod conformance {
    include!(concat!(env!("OUT_DIR"), "/conformance.rs"));
}

pub mod protobuf_test_messages {
    pub mod proto3 {
        include!(concat!(env!("OUT_DIR"), "/proto3.rs"));
    }
}

pub mod google {
    pub mod protobuf {
        include!(concat!(env!("OUT_DIR"), "/protobuf.rs"));
    }
}

use std::io::{
    Cursor,
    Read,
    Write,
    self,
};

use bytes::{
    Buf,
    ByteOrder,
    LittleEndian,
};
use proto::Message;

use conformance::{
    conformance_request,
    conformance_response,
    ConformanceRequest,
    ConformanceResponse,
    WireFormat,
};
use protobuf_test_messages::proto3::{
    TestAllTypes,
};

fn main() {
    env_logger::init().unwrap();
    let mut bytes = Vec::new();

    loop {
        bytes.resize(4, 0);

        io::stdin().read_exact(&mut bytes[..]).expect("input closed");
        let len = LittleEndian::read_u32(&bytes[..]) as usize;

        bytes.resize(len, 0);
        io::stdin().read_exact(&mut bytes[..]).unwrap();

        let result = match ConformanceRequest::decode(&mut Buf::take(Cursor::new(&mut bytes), len)) {
            Ok(request) => handle_request(request),
            Err(error) => conformance_response::Result::ParseError(format!("{:?}", error)),
        };

        let mut response = ConformanceResponse::default();
        response.result = Some(result);

        let len = response.encoded_len();
        bytes.resize(4, 0);

        LittleEndian::write_u32(&mut bytes[..4], len as u32);
        response.encode(&mut bytes).unwrap();
        assert_eq!(len + 4, bytes.len());

        let mut stdout = io::stdout();
        stdout.lock().write_all(&bytes).unwrap();
        stdout.flush().unwrap();
    }
}

fn handle_request(request: ConformanceRequest) -> conformance_response::Result {
    match request.requested_output_format() {
        Some(WireFormat::Json) =>
            return conformance_response::Result::Skipped("JSON output is not supported".to_string()),
        None =>
            return conformance_response::Result::ParseError("unrecognized requested output format".to_string()),
            _ => (),
    };

    let mut buf = match request.payload {
        None => return conformance_response::Result::ParseError("no payload".to_string()),
        Some(conformance_request::Payload::JsonPayload(_)) =>
            return conformance_response::Result::Skipped("JSON input is not supported".to_string()),
        Some(conformance_request::Payload::ProtobufPayload(buf)) => buf,
    };

    let len = buf.len();
    let all_types = match TestAllTypes::decode(&mut Buf::take(Cursor::new(&mut buf), len)) {
        Ok(all_types) => all_types,
        Err(error) => return conformance_response::Result::ParseError(format!("failed to parse TestAllTypes: {:?}", error)),
    };

    debug!("request: {:?}", all_types);

    buf.clear();
    all_types.encode(&mut buf).unwrap();

    if all_types.encoded_len() != buf.len() {
        return conformance_response::Result::RuntimeError(
            format!("encoded length does not match actual; encoded_len: {}, buf len: {}",
                    all_types.encoded_len(), buf.len()));
    }

    let len = buf.len();
    let roundtrip = match TestAllTypes::decode(&mut Buf::take(Cursor::new(&mut buf), len)) {
        Ok(roundtrip) => roundtrip,
        Err(error) => return conformance_response::Result::ParseError(format!("failed to parse roundtrip TestAllTypes: {:?}", error)),
    };

    if all_types != roundtrip {
        return conformance_response::Result::RuntimeError(
            format!("roundtrip value does not match original;\n\t original: {:?}\n\troundtrip: {:?}",
                    all_types, roundtrip));
    }

    conformance_response::Result::ProtobufPayload(buf)
}
