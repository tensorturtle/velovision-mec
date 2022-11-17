use std::thread;
use std::time::Duration;

use zmq;

use cornflakes::{
    capnp_bytes_io,
    VmecRequestFields,
    VmecResponseFields,
    vmec_request_capnp,
    vmec_response_capnp,
    vmec_request_transport,
    vmec_response_transport,
};

fn ms_now() -> u64 {
    let now = std::time::SystemTime::now();
    let since_the_epoch = now
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards");
    let in_ms = since_the_epoch.as_millis();
    in_ms as u64
}

fn process_request(request: &VmecRequestFields) -> VmecResponseFields {
    // mock function for server-side work
    VmecResponseFields {
        timestamp_ms: ms_now(),
        server_hash: String::from("server_hash blah blash"),
        response_hash: String::from("respond_hash"),
        neural_output: String::from("neural_output"),
    }
}

fn main() {
    let context = zmq::Context::new();
    let responder = context.socket(zmq::REP).unwrap();
    assert!(responder.bind("tcp://*:5555").is_ok());

    loop {
        let byte_msg = responder.recv_bytes(0).unwrap();

        //println!("Received request: [{:?}]", byte_msg);
        println!("Received");
        // server
        let request_received: &[u8] = &byte_msg;
        let parsed_request: VmecRequestFields = vmec_request_transport::decode_request(request_received).unwrap();
        //println!("Parsed request image data: {:?}", parsed_request.image_front);
        let response_vals: VmecResponseFields = process_request(&parsed_request);
        let response_to_send = vmec_response_transport::encode_response(response_vals).unwrap();

        // do some 'work'
        thread::sleep(Duration::from_millis(1));

        // send reply back to client
        responder.send(response_to_send, 0).unwrap();
    }

}
