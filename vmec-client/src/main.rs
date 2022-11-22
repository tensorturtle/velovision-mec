use std::time::Duration;
use std::{io, env, thread};
use std::io::Write;
use std::fs;

use log::{debug, info, warn, error};
use log::LevelFilter;

use clap::Parser;
use rscam::{Camera, Config};
use turbojpeg;
use show_image::{ImageView, ImageInfo, create_window, WindowProxy, WindowOptions};
use ctrlc;
use image;
use uuid;
use blake3;

use cornflakes::{
    VmecRequestFields, 
    VmecResponseFields,
    vmec_request_transport,
    vmec_response_transport,
};

fn print_type<T>(_: &T) {
    println!("{}", std::any::type_name::<T>())
}

fn precise_duration_ms(duration: Duration) -> f64 {
    duration.subsec_micros() as f64 / 1000.0
}

fn launch_camera(path: &str, fps: u32, width: u32, height: u32, format: &[u8]) -> io::Result<rscam::Camera> {
    let camera_handle = Camera::new(path);
    match camera_handle {
        Ok(mut cam) => {
            cam.start(&Config {
                interval: (1, fps),
                resolution: (width, height),
                format: format,
                ..Default::default()
            }).unwrap();
            Ok(cam)
        },
        Err(e) => Err(e)
    }
}

fn decode_jpeg_to_imagebuffer(frame: &rscam::Frame) -> image::ImageBuffer<image::Rgb<u8>, Vec<u8>> {
    let image: image::RgbImage = turbojpeg::decompress_image(&frame[..]).unwrap();
    image
}

fn encode_imagebuffer_to_jpeg(image: &image::ImageBuffer<image::Rgb<u8>, Vec<u8>>) -> turbojpeg::OwnedBuf {
    turbojpeg::compress_image(image, 100, turbojpeg::Subsamp::Sub2x2).unwrap()
}

fn decode_jpeg_to_raw(frame: &rscam::Frame) -> Vec<u8> {
    // convert JPEG from camera to raw image with turbojpeg
    // 1.4ms for 640x480
    let image: image::RgbImage = turbojpeg::decompress_image(&frame[..]).unwrap();
    image.into_raw()
}

fn raw_pixels_to_tensor(pixels: &[u8], width: u32, height: u32) -> tch::Tensor {
    let pixel_tensor = tch::Tensor::of_slice(pixels);

    // pixel_tensor is a 1D tensor organized as [R, G, B, R, G, B, ...]
    // each ['R', 'G', 'B'] is a pixel, and grouped along width, then height
    // Torch tensor requires [C, H, W] ordering, so we need to reshape and permute
    pixel_tensor.reshape(&[height as i64, width as i64, 3]).permute(&[2, 0, 1])
}

fn save_tensor_as_image(tensor: tch::Tensor, path: &str) {
    tch::vision::image::save(&tensor, path).unwrap();
}

fn crush(frame: &rscam::Frame, width: u32, height: u32, quality: u16) -> Vec<u8> {
    // Too slow; 8ms for 320x240
    assert!(quality <= 100 && quality > 0);
    let imgbuf = decode_jpeg_to_imagebuffer(frame);

    let resized_imgbuf = image::imageops::resize(&imgbuf, width, height, image::imageops::FilterType::Triangle); // choose CatmullRom (cubic) for better quality and Lanczos3 for best quality
    
    let encoded_jpeg = turbojpeg::compress_image(&resized_imgbuf, quality as i32, turbojpeg::Subsamp::Sub2x2).unwrap();

    encoded_jpeg[..].to_vec()
}

fn write_jpeg_to_file(path: &str, frame: &rscam::Frame) {
    let mut file = std::fs::OpenOptions::new()
    .write(true)
    .create(true)
    .open(path).unwrap();
    file.write_all(&frame[..]).unwrap();
}

fn get_machine_hash() -> String {
    // per https://man7.org/linux/man-pages/man5/machine-id.5.html
    // machine-id should not be used directly (especially over network)
    // instead, hash it with a cryptographically secure hash function
    fs::read_to_string("/etc/machine-id").unwrap()
}

fn hash_strings(strings: Vec<String>) -> String {
    let mut hasher = blake3::Hasher::new();
    for string in strings {
        hasher.update(string.as_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None)]
struct Args {
    #[arg(long, default_value="/dev/video0")]
    /// Path to the camera device
    camera_path: String,

    #[arg(long)]
    video_path: String,

    #[arg(long, default_value="localhost")]
    server_ip: String,

    #[arg(long, default_value="5555")]
    server_port: u16,

    #[arg(long)]
    show_image_window: bool,

    #[arg(long)]
    save_cam_image: bool,

    #[arg(long)]
    save_tensor_image: bool,

    #[arg(long, default_value="300")]
    // milliseconds to wait for a response from the server
    receive_timeout: i32,
}

#[show_image::main]
fn main() {
    simple_logging::log_to_stderr(LevelFilter::Debug);

    let args = Args::parse();

    let arg_cam_path = args.camera_path;
    let arg_save_image = args.save_cam_image;
    let arg_show_image = args.show_image_window;

    let window_options = WindowOptions {
        start_hidden: true,
        ..Default::default()
    };
    let window = show_image::create_window(
        "camera display",
        window_options,
    ).unwrap();
    let shown_image_info = ImageInfo::new(
        show_image::PixelFormat::Rgb8,
        320,
        240,
    );

    // clean up window - if not done, gnome-shell leaks and eats CPU
    ctrlc::set_handler(move || {
        println!("Exiting...");
        show_image::exit(0);
    }).expect("Error setting Ctrl-C handler");


    let camera: Result<rscam::Camera, io::Error> = launch_camera(&arg_cam_path, 30, 320, 240, b"MJPG");

    // for file naming
    if arg_save_image {
        std::fs::create_dir_all("images").unwrap();
    }

    // basic ZMQ request client
    let context = zmq::Context::new();

    let mut i = 0;
    loop {
        i += 1;
        debug!("\n=== Next Frame ===");

        let start = std::time::Instant::now();

        let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
        let request_hash = hash_strings(vec![get_machine_hash(), i.to_string(), timestamp.to_string()]);

        let frame = camera.as_ref().unwrap().capture().unwrap();
        let (width, height) = frame.resolution;

        // decode, resize, and re-encode with lower quality
        // let jpeg_bytes = crush(&frame, 320, 240, 50);

        if args.save_cam_image {
            write_jpeg_to_file(&format!("{}/{}_{}_{}", "images", "image", &i.to_string(), "rawcam"), &frame);
        }

        debug!("Size of frame in bytes: {}", frame.len());

        let vmec_request_vals = VmecRequestFields {
            timestamp_ms: timestamp,
            device_hash: get_machine_hash(),
            request_hash: request_hash,
            image_front: Vec::from(&(*frame)),
            // image_front: Vec::from([1,2,3]),
            image_rear: Vec::from(&(*frame)),
            // image_rear: Vec::from([1,2,3]),
        };

        let request_to_send = vmec_request_transport::encode_request(vmec_request_vals).unwrap();

        let requester = context.socket(zmq::REQ).unwrap();
        requester.set_rcvtimeo(args.receive_timeout).unwrap(); // Set timeout because otherwise infinite connections will be made since ZMQ socket is run in a thread
        //let address = "tcp://localhost:5555";
        let address = format!("tcp://{}:{}",
            args.server_ip,
            args.server_port,
        );
        assert!(requester.connect(&address).is_ok());

        let request_handle = thread::spawn(move || {
            //requester.send("Hello", 0).unwrap();
            // time round trip
            let sent_time = std::time::Instant::now();
            debug!("Inside ZMQ Thread: Sending request");
            debug!("Length of request: {}", request_to_send.len());
            requester.send(request_to_send, 0).unwrap();
            debug!("Inside ZMQ Thread: Sent request");

            //let received_bytes = requester.recv_bytes(0).unwrap();

            match requester.recv_bytes(0) {
                Ok(received_bytes) => {
                    let duration = sent_time.elapsed();
                    debug!("Length of received bytes: {}", received_bytes.len());
                    Ok((duration, received_bytes))
                },
                Err(_) => {
                    debug!("Inside ZMQ Thread: Socket timed out waiting for reply");
                    Err(std::io::Error::new(std::io::ErrorKind::Other, "ZMQ Timeout"))
                }
            }

            // calculate duration
            //let duration = sent_time.elapsed();
            //(duration, received_bytes)
        });

        let t1 = std::time::Instant::now();
        let raw_pixels = decode_jpeg_to_raw(&frame);
        debug!("Time to decode JPEG: {} microseconds", t1.elapsed().as_micros());
        debug!("Size of raw_pixels in bytes: {}", raw_pixels.len());
        let image_tensor = raw_pixels_to_tensor(&raw_pixels, width, height);

        debug!("resized pixel_tensor: {:?}", image_tensor.size());

        // save tensor as image
        if args.save_tensor_image{
            // create directory
            std::fs::create_dir_all("images").unwrap();
            let filename = format!("images/image_{}_tensor.jpg", i);
            save_tensor_as_image(image_tensor, &filename);
        }

        if arg_show_image {
            let shown_image = show_image::ImageView::new(
                shown_image_info,
                &raw_pixels,
            );
            window.run_function(|mut window| {
                window.set_visible(true);
            });
            window.set_image("image", shown_image).unwrap();
        }

        thread::sleep(Duration::from_millis(500)); // mock work on client

        if request_handle.is_finished() {
            debug!("Request thread finished");
            let (roundtrip_time, reply_bytes) = match request_handle.join().unwrap() {
                Ok(thread_output) => thread_output,
                Err(e) => {
                    debug!("Error in request thread: {:?}", e);
                    continue;
                }
            };
            let reply = vmec_response_transport::decode_response(&reply_bytes).unwrap();
            info!("Roundtrip time: {} μs", roundtrip_time.as_micros());
            info!("Roundtrip time: {} ms",
        roundtrip_time.as_millis());
            info!("Reply data timestamp: {} ms", reply.timestamp_ms);
        } else {
            warn!("Timeout: Reply from server did not arrive by the time local work was done. Continuing...");
        }
    }
}

#[cfg(test)]
mod tests {
    // Test Conventions
    // + "common" tests are meant to be run on any platform (including CI)
    // + "bare" tests are for systems with cameras & hardware connected. They are expected to fail on CI
    use tch::Kind;
    use std::env;
    use super::{
        raw_pixels_to_tensor,
        launch_camera,
    };

    fn running_in_ci_server() -> bool {
        // check CI environment variable
        let ci = env::var("CI").unwrap_or("false".to_string());
        ci == "true"
    }

    #[test]
    fn common_it_works() {
        assert_eq!(2 + 2, 4);
    }
    #[test]
    fn bare_preproc_launch_camera() {
        if running_in_ci_server() {
            assert!(false, "This is a bare test, and should not be run on CI");
        }
        println!("Is bare metal - running test");
        let width = 640;
        let height = 480;

        let camera = launch_camera("/dev/video0", 30, width, height, b"MJPG").unwrap();
        let frame: rscam::Frame = camera.capture().unwrap();
        assert_eq!(frame.resolution, (width, height));
    }
    #[test]
    fn common_preproc_raw_pixels_to_tensor() {
        let subpixel_value: u8 = 255; // 0-255 for 8-bit pixel values
        let sum_subpixel_value: u32 = subpixel_value as u32 * 640 * 480 * 3; // we will use this to check that the tensor sum is correct
        // Create a 640x480 image where all pixels are the same value
        let mut pixels = vec![0; 640 * 480 * 3];
        for i in 0..pixels.len() {
            pixels[i] = subpixel_value;
        }

        let image_tensor: tch::Tensor = raw_pixels_to_tensor(&pixels, 640, 480);
        println!("image_tensor shape: {:?}", image_tensor.size());
        assert_eq!(image_tensor.size(), &[3, 480, 640]); // C, H, W ordering for tensor

        let tensor_val_sum = image_tensor.sum(Kind::Float).double_value(&[]) as u32;
        println!("Sum of tensor values: {}", tensor_val_sum);
        println!("Sum of subpixel values: {}", sum_subpixel_value);
        assert_eq!(tensor_val_sum, sum_subpixel_value);
    } 
}