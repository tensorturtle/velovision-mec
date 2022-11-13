use zmq;
use std::thread;
use std::time::Duration;

fn main() {
    // Basic ZMQ reply server
    let context = zmq::Context::new();
    let responder = context.socket(zmq::REP).unwrap();
    assert!(responder.bind("tcp://*:5555").is_ok());

    loop {
        let msg = responder.recv_string(0).unwrap().unwrap();
        println!("Received request: [{}]", msg);

        // do some 'work'
        thread::sleep(Duration::from_millis(20));

        // send reply back to client
        responder.send("World", 0).unwrap();
    }
}
