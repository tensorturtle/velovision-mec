use std::thread;
use std::time::Duration;

use zmq;

mod capnp_bytes_io {
    pub struct CapnpEncoding {
        // Copies the bytes output of capn proto serialization to `bytesbuffer`
        // Instead of printing it to stdout as the example does
        // Example: https://github.com/capnproto/capnproto-rust/blob/master/example/addressbook/addressbook.rs
        pub encoded_bytes: Vec<u8>,
    }

    impl std::io::Write for CapnpEncoding {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            //dbg!(buf);
            self.encoded_bytes.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            println!("flush");
            Ok(())
        }
    }

    pub struct CapnpDecoding {
        // Takes bytes from `bytesbuffer` to deserialize them through capnproto
        // Instead of reading from stdin as the example does
        // Example: https://github.com/capnproto/capnproto-rust/blob/master/example/addressbook/addressbook.rs
        pub bytes_to_decode: Vec<u8>,
    }
    impl std::io::Read for CapnpDecoding {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let mut i = 0;
            for b in self.bytes_to_decode.iter() {
                buf[i] = *b;
                i += 1;
            }
            Ok(i)
        }
    }

    impl std::io::BufRead for CapnpDecoding {
        fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
            Ok(&self.bytes_to_decode)
        }

        fn consume(&mut self, amt: usize) {
            self.bytes_to_decode.drain(..amt);
        }
    }
}

pub struct VmecRequestFields {
    pub timestamp_ms: u64,
    pub device_hash: String,
    pub request_hash: String,
    pub image_front: Vec<u8>,
    pub image_rear: Vec<u8>,
}

pub struct VmecRespondFields<'a> {
    pub timestamp_ms: u64,
    pub server_hash: &'a str,
    pub respond_hash: &'a str,
    pub neural_output: &'a str,
}

pub mod vmec_request_capnp {
    include!(concat!(env!("OUT_DIR"), "/vmec_request_capnp.rs"));
}

pub mod vmec_response {
    // TODO: Implement vmec_response, see vmec_request_capnp. Create a new .capnp schema file and the rest of it.
}

pub mod vmec_request {
    use crate::vmec_request_capnp::{req_frame, vmec_request_struct};
    use crate::VmecRequestFields;

    pub fn encode_request(fields: VmecRequestFields) -> Result<Vec<u8>, std::io::Error> {
        use crate::capnp_bytes_io::CapnpEncoding;
        let mut capnp_enc = CapnpEncoding {
            encoded_bytes: Vec::new(),
        };

        let mut message = ::capnp::message::Builder::new_default();
        {
            let vmec_request_struct= message.init_root::<vmec_request_struct::Builder>();

            let mut frame = vmec_request_struct.init_frame(1);

            {
                let mut req_frame = frame.reborrow().get(0);
                req_frame.set_timestamp_ms(fields.timestamp_ms);
                req_frame.set_monotonic_id(1234);
                req_frame.set_device_hash("ee70384492767846");
                req_frame.set_session_hash("hamilton");
                {
                    let mut req_frame_images = req_frame.reborrow().init_images(1);
                    req_frame_images
                        .reborrow()
                        .get(0)
                        .set_jpegbytes(b"555-1212");
                    req_frame_images
                        .reborrow()
                        .get(0)
                        .set_type(req_frame::camera_image::CameraDirection::Frontcam);
                }
            }
        }
        capnp::serialize_packed::write_message(&mut capnp_enc, &message).unwrap();
        Ok((capnp_enc.encoded_bytes).to_vec())
    }

    pub fn decode_request(bytes_to_decode: &[u8]) -> capnp::Result<VmecRequestFields> {
        use crate::capnp_bytes_io::CapnpDecoding;
        let mut capnp_dec = CapnpDecoding {
            bytes_to_decode: bytes_to_decode.to_vec(),
        };

        let message_reader = capnp::serialize_packed::read_message(
            &mut capnp_dec,
            ::capnp::message::ReaderOptions::new(),
        )?;

        let vmec_request_struct= message_reader.get_root::<vmec_request_struct::Reader>()?;

        let mut req_fields = VmecRequestFields {
            timestamp_ms: 0,
            device_hash: "".to_string(),
            request_hash: "".to_string(),
            image_front: Vec::new(),
            image_rear: Vec::new(),
        };

        for frame in vmec_request_struct.get_frame()? {
            req_fields.timestamp_ms = frame.get_timestamp_ms();
            req_fields.device_hash = frame.get_device_hash()?.to_string();
            req_fields.request_hash = frame.get_session_hash()?.to_string();
            for image in frame.get_images()? {
                if image.get_type()? == req_frame::camera_image::CameraDirection::Frontcam {
                    req_fields.image_front = image.get_jpegbytes()?.to_vec();
                } else {
                    req_fields.image_rear = image.get_jpegbytes()?.to_vec();
                }
            }
        }
        Ok(req_fields)
    }
}

fn ms_now() -> u64 {
    let now = std::time::SystemTime::now();
    let since_the_epoch = now
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards");
    let in_ms = since_the_epoch.as_millis();
    in_ms as u64
}

fn process_request(request: &VmecRequestFields) -> VmecRespondFields {
    // mock function for server-side work
    VmecRespondFields {
        timestamp_ms: ms_now(),
        server_hash: "server_hash",
        respond_hash: "respond_hash",
        neural_output: "neural_output",
    }
}

fn main() {
    // client
    let vmec_request_vals = VmecRequestFields {
        timestamp_ms: ms_now(),
        device_hash: "ee70384492767846".to_string(),
        request_hash: "hamilton".to_string(),
        image_front: b"555-1212".to_vec(),
        image_rear: b"555-1212".to_vec(),
    };

    let request_to_send = vmec_request::encode_request(vmec_request_vals).unwrap();

    println!("buffer_to_send: {:?}", request_to_send);

    // server
    let request_received = &request_to_send;
    let parsed_request = vmec_request::decode_request(request_received).unwrap();
    let response_to_send: VmecRespondFields = process_request(&parsed_request);

    // client
    let response_received = &response_to_send;

    // Basic ZMQ reply server
    let context = zmq::Context::new();
    let responder = context.socket(zmq::REP).unwrap();
    assert!(responder.bind("tcp://*:5555").is_ok());

    loop {
        let msg = responder.recv_string(0).unwrap().unwrap();
        println!("Received request: [{}]", msg);

        // do some 'work'
        thread::sleep(Duration::from_millis(10));

        // send reply back to client
        responder.send("World", 0).unwrap();
    }
}
