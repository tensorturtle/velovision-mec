use std::time::Duration;
use std::{io, env, thread};

use rscam::{Camera, Config};
use turbojpeg;
use show_image::{ImageView, ImageInfo, create_window, WindowProxy, WindowOptions};
use ctrlc;
use thread_tryjoin::TryJoinHandle;



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

fn decode_jpeg(frame: &rscam::Frame) -> Vec<u8> {
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



#[show_image::main]
fn main() {
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

    // clean up window - if not done, gnome-shell eats CPU
    ctrlc::set_handler(move || {
        println!("Exiting...");
        show_image::exit(0);
    }).expect("Error setting Ctrl-C handler");


    let camera: Result<rscam::Camera, io::Error> = launch_camera(arg_cam_path, 30, 640, 480, b"MJPG");

    // for file naming
    let mut i = 0;
    if arg_save_image {
        std::fs::create_dir_all("images").unwrap();
    }

    // basic ZMQ request client
    let context = zmq::Context::new();

    loop {
        println!("");


        let start = std::time::Instant::now();

        let raw_pixels: Vec<u8>;

        let frame = camera.as_ref().unwrap().capture().unwrap();

        // send jpeg image through zmq
        //println!("Sending image...");
        let requester = context.socket(zmq::REQ).unwrap();
        assert!(requester.connect("tcp://localhost:5555").is_ok());
        // send a request, wait for reply
        // run in a thread so we can do other stuff while waiting
        //println!("Spawning thread to receive reply");
        let request_handle = thread::spawn(move || {
            requester.send("Hello", 0).unwrap();
            requester.recv_string(0).unwrap().unwrap()
        });

        raw_pixels = decode_jpeg(&frame);

        // // if camera is not found, generate random pixels
        // raw_pixels = vec![0; 640 * 480 * 3];

        // start timer
        let postprocess_start = std::time::Instant::now();

        let image_tensor = raw_pixels_to_tensor(&raw_pixels);

        //println!("resized pixel_tensor: {:?}", image_tensor.size());

        // save tensor as image
        if arg_save_image {
            // create directory
            std::fs::create_dir_all("images").unwrap();
            let filename = format!("images/image_{}.jpg", i);
            save_tensor_as_image(image_tensor, &filename);
            i += 1;
        }

        // measure postprocess time in ms
        let postprocess_duration = postprocess_start.elapsed();

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

        //println!("Postprocessing: {:.3} ms", precise_duration_ms(postprocess_duration));

        // measure time
        let duration = start.elapsed();
        //println!("frame: {} ms", duration.subsec_millis());

        // do some local work while waiting for remote reply
        thread::sleep(Duration::from_millis(30));

        if request_handle.is_finished() {
            let reply = request_handle.join().unwrap();
            println!("Received reply in time: {}", reply);
        } else {
            println!("Timeout: Reply from server did not arrive by the time local work was done. Continuing...");
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