//! Cap'n Proto code generation and conversion to/from Rust bytes.
//! `structs` defined 

pub struct VmecRequestFields {
    pub timestamp_ms: u64,
    pub device_hash: String,
    pub request_hash: String,
    pub image_front: Vec<u8>,
    pub image_rear: Vec<u8>,
}
pub struct VmecResponseFields {
    pub timestamp_ms: u64,
    pub server_hash: String,
    pub response_hash: String,
    pub neural_output: String,
}

pub mod vmec_request_capnp {
    // code generated by capnpc
    // configured via build.rs
    include!(concat!(env!("OUT_DIR"), "/schemas/vmec_request_capnp.rs"));
}

pub mod vmec_response_capnp {
    include!(concat!(env!("OUT_DIR"), "/schemas/vmec_response_capnp.rs"));
}

pub mod capnp_bytes_io {
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


pub mod vmec_response_transport {
    // TODO: Implement vmec_response, see vmec_request_capnp. Create a new .capnp schema file and the rest of it.

    use crate::vmec_response_capnp::vmec_response_struct;
    use crate::VmecResponseFields;

    pub fn encode_response(fields: VmecResponseFields) -> Result<Vec<u8>, std::io::Error> {
        use crate::capnp_bytes_io::CapnpEncoding;
        let mut capnp_enc = CapnpEncoding {
            encoded_bytes: Vec::new(),
        };
        let mut message = ::capnp::message::Builder::new_default();
        {
            let vmec_response_struct = message.init_root::<vmec_response_struct::Builder>();

            let mut frame = vmec_response_struct.init_frame(1);
            
            {
                let mut res_frame = frame.reborrow().get(0);
                res_frame.set_timestamp_ms(fields.timestamp_ms);
                res_frame.set_server_hash(&fields.server_hash);
                res_frame.set_response_hash(&fields.response_hash);
                res_frame.set_neural_output(&fields.neural_output);
            }
        }

        capnp::serialize_packed::write_message(&mut capnp_enc, &message).unwrap();

        Ok((capnp_enc.encoded_bytes).to_vec())
    }

    pub fn decode_response(bytes_to_decode: &[u8]) -> capnp::Result<VmecResponseFields> {
        use crate::capnp_bytes_io::CapnpDecoding;
        let mut capnp_dec = CapnpDecoding {
            bytes_to_decode: bytes_to_decode.to_vec(),
        };

        let message_reader = capnp::serialize_packed::read_message(
            &mut capnp_dec,
            ::capnp::message::ReaderOptions::new(),
        )?;

        let vmec_response_struct= message_reader.get_root::<vmec_response_struct::Reader>()?;

        let mut res_fields = VmecResponseFields {
            timestamp_ms: 0,
            server_hash: String::new(),
            response_hash: String::new(),
            neural_output: String::new(),
        };

        for frame in vmec_response_struct.get_frame()? {
            res_fields.timestamp_ms = frame.get_timestamp_ms();
            res_fields.server_hash = frame.get_server_hash()?.to_string();
            res_fields.response_hash = frame.get_response_hash()?.to_string();
            res_fields.neural_output = frame.get_neural_output()?.to_string();
        }
        Ok(res_fields)
    }
}

pub mod vmec_request_transport {
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
                req_frame.set_device_hash(&String::from(fields.device_hash));
                req_frame.set_request_hash(&String::from(fields.request_hash));
                {
                    let mut req_frame_images = req_frame.reborrow().init_images(2);
                    req_frame_images
                        .reborrow()
                        .get(0)
                        .set_jpegbytes(&fields.image_front);
                    req_frame_images
                        .reborrow()
                        .get(0)
                        .set_type(req_frame::camera_image::CameraDirection::Frontcam);
                    req_frame_images
                        .reborrow()
                        .get(1)
                        .set_jpegbytes(&fields.image_rear);
                    req_frame_images
                        .reborrow()
                        .get(1)
                        .set_type(req_frame::camera_image::CameraDirection::Rearcam);
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
            req_fields.request_hash = frame.get_request_hash()?.to_string();

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert!(true);
    }
}