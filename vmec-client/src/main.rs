use std::time::Duration;
use std::{io, env, thread};
use std::io::Write;
use std::fs;

use log::{debug, info, warn, error};
use log::LevelFilter;

use rscam::{Camera, Config};
use turbojpeg;
use show_image::{ImageView, ImageInfo, create_window, WindowProxy, WindowOptions};
use ctrlc;
use image;

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
    let prec_ms = duration.subsec_micros() as f64 / 1000.0;
    return prec_ms;
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
    let pixels = image.into_raw();
    return pixels;
}

fn raw_pixels_to_tensor(pixels: &Vec<u8>) -> tch::Tensor {
    let pixel_tensor = tch::Tensor::of_slice(&pixels);

    // pixel_tensor is a 1D tensor organized as [R, G, B, R, G, B, ...]
    // each ['R', 'G', 'B'] is a pixel, and grouped along width, then height
    // Torch tensor requires [C, H, W] ordering, so we need to reshape and permute
    let resized_pixel_tensor = pixel_tensor.reshape(&[480, 640, 3]).permute(&[2, 0, 1]);

    return resized_pixel_tensor;
}

fn save_tensor_as_image(tensor: tch::Tensor, path: &str) {
    tch::vision::image::save(&tensor, path).unwrap();
}

fn crush(frame: &rscam::Frame, width: u32, height: u32) -> Vec<u8> {
    let imgbuf = decode_jpeg_to_imagebuffer(frame);

    let resized_imgbuf = image::imageops::resize(&imgbuf, width, height, image::imageops::FilterType::Triangle); // choose CatmullRom (cubic) for better quality and Lanczos3 for best quality
    
    let encoded_jpeg = turbojpeg::compress_image(&resized_imgbuf, 50, turbojpeg::Subsamp::Sub2x2).unwrap();

    encoded_jpeg[..].to_vec()
}

#[show_image::main]
fn main() {
    simple_logging::log_to_stderr(LevelFilter::Debug);

    let args: Vec<String> = env::args().collect();

    // parse args for 
    // 1. camera path
    // 2. save image?
    // 3. show image?
    // confirm that we have at least 3 args
    if args.len() < 4 {
        println!("Usage: cargo run <camera_path> <save_image> <show_image>");
        println!("Example (both save and show): cargo run /dev/video0 save show");
        println!("Example (only show): cargo run /dev/video0 -- show");
        return;
    }

    let arg_cam_path = &args[1];
    let arg_save_image = &args[2] == "save";
    let arg_show_image = &args[3] == "show";

    let window: WindowProxy;

    let window_options = WindowOptions {
        start_hidden: true,
        ..Default::default()
    };
    window = show_image::create_window(
        "camera display",
        window_options,
    ).unwrap();
    let shown_image_info = ImageInfo::new(
        show_image::PixelFormat::Rgb8,
        640,
        480,
    );

    // clean up window - if not done, gnome-shell leaks and eats CPU
    ctrlc::set_handler(move || {
        println!("Exiting...");
        show_image::exit(0);
    }).expect("Error setting Ctrl-C handler");


    let camera: Result<rscam::Camera, io::Error> = launch_camera(arg_cam_path, 30, 320, 240, b"MJPG");

    // for file naming
    let mut i = 0;
    if arg_save_image {
        std::fs::create_dir_all("images").unwrap();
    }

    // basic ZMQ request client
    let context = zmq::Context::new();

    loop {
        info!("=== Next Frame ===\n");

        let start = std::time::Instant::now();

        //let raw_pixels: Vec<u8>;

        let frame = camera.as_ref().unwrap().capture().unwrap();

        // decode, resize, and re-encode with lower quality
        // let t1 = precise_duration_ms(start.elapsed());
        // let img = crush(&frame, 320, 240);
        // let t2 = precise_duration_ms(start.elapsed());

        // println!("Time to decode and re-encode: {}ms", t2 - t1);

        // write encoded_jpeg to file as .jpg
        // let mut file = std::fs::OpenOptions::new()
        // .write(true)
        // .create(true)
        // .open(format!("images/{}_fromcam.jpg", i)).unwrap();
        // i += 1;

        // file.write_all(&frame[..]).unwrap();



        // let mut file = std::fs::OpenOptions::new()
        // .write(true)
        // .create(true)
        // .open(format!("images/{}_reencoded.jpg", i)).unwrap();
        // i += 1;
        // file.write_all(&img[..]).unwrap();


    





        // get size of encoded jpeg
        println!("Size of frame from camera: {}", frame.len());
        //println!("Size of re-encoded image: {}", img.len());

        //print_type(&(frame.to_vec()));


        // send jpeg image through zmq
        //println!("Sending image...");

        let vmec_request_vals = VmecRequestFields {
            timestamp_ms: 101010101111,
            device_hash: String::from("ee70384492767846"),
            request_hash: String::from("hamilton"),
            image_front: Vec::from(&(*frame)),
            // image_front: Vec::from([1,2,3]),
            image_rear: Vec::from(&(*frame)),
            // image_rear: Vec::from([1,2,3]),
        };

        // vmec_request_vals.image_front = Vec::from(&(*frame));

        let request_to_send = vmec_request_transport::encode_request(vmec_request_vals).unwrap();
        // send a request, wait for reply
        // run in a thread so we can do other stuff while waiting
        //println!("Spawning thread to receive reply");

        // Normally, ZMQ sockets are created once outside of loop
        // However, since the socket is moved into the thread, we need to recreate it
        let requester = context.socket(zmq::REQ).unwrap();
        requester.set_rcvtimeo(300).unwrap(); // 2 second timeout. Important to set it because otherwise infinite connections will be made since this ZMQ socket is run in a thread
        //let address = "tcp://localhost:5555";
        let address = "tcp://18.188.177.59:5555";
        assert!(requester.connect(address).is_ok());

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
                    return Ok((duration, received_bytes));
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


        // // if camera is not found, generate random pixels
        // raw_pixels = vec![0; 640 * 480 * 3];

        // start timer
        let postprocess_start = std::time::Instant::now();

        //let image_tensor = raw_pixels_to_tensor(&raw_pixels);

        //println!("resized pixel_tensor: {:?}", image_tensor.size());

        // save tensor as image
        if arg_save_image {
            // create directory
            std::fs::create_dir_all("images").unwrap();
            let filename = format!("images/image_{}.jpg", i);
            //save_tensor_as_image(image_tensor, &filename);
            i += 1;
        }

        // measure postprocess time in ms
        let postprocess_duration = postprocess_start.elapsed();

        // if arg_show_image {
        //     let shown_image = show_image::ImageView::new(
        //         shown_image_info,
        //         &raw_pixels,
        //     );
        //     window.run_function(|mut window| {
        //         window.set_visible(true);
        //     });
        //     window.set_image("image", shown_image).unwrap();
        // }

        //println!("Postprocessing: {:.3} ms", precise_duration_ms(postprocess_duration));

        // measure time
        let duration = start.elapsed();
        //println!("frame: {} ms", duration.subsec_millis());

        // do some local work while waiting for remote reply
        thread::sleep(Duration::from_millis(300));

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
        return ci == "true";
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

        let image_tensor: tch::Tensor = raw_pixels_to_tensor(&pixels);
        println!("image_tensor shape: {:?}", image_tensor.size());
        assert_eq!(image_tensor.size(), &[3, 480, 640]); // C, H, W ordering for tensor

        let tensor_val_sum = image_tensor.sum(Kind::Float).double_value(&[]) as u32;
        println!("Sum of tensor values: {}", tensor_val_sum);
        println!("Sum of subpixel values: {}", sum_subpixel_value);
        assert_eq!(tensor_val_sum, sum_subpixel_value);
    } 
}