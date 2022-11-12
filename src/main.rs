use std::env;
use std::time::Duration;

use rscam::{Camera, Config};
use turbojpeg;



fn print_type<T>(_: &T) {
    println!("{}", std::any::type_name::<T>())
}

fn precise_duration_ms(duration: Duration) -> f64 {
    let prec_ms = duration.subsec_micros() as f64 / 1000.0;
    return prec_ms;
}

fn launch_camera(path: &str, fps: u32, width: u32, height: u32, format: &[u8]) -> rscam::Camera {
    let mut camera = Camera::new(path).unwrap();

    camera.start(&Config {
        interval: (1, fps),
        resolution: (width, height),
        //format: b"MJPG",
        format: format,
        ..Default::default()
    }).unwrap();

    return camera;
}

fn decode_jpeg(frame: &rscam::Frame) -> Vec<u8> {
    // convert JPEG from camera to raw image with turbojpeg
    // 1.4ms for 640x480
    let image: image::RgbImage = turbojpeg::decompress_image(&frame[..]).unwrap();
    let pixels = image.into_raw();
    return pixels;
}

fn raw_pixels_to_tensor(pixels: Vec<u8>) -> tch::Tensor {
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

fn baremetal() -> bool {
    // Use virt-what to check if we are in a virtual machine

    let output = std::process::Command::new("virt-what")
        .output()
        .expect("Failed to execute virt-what");
    
    // if output is empty, we are not in a virtual machine
    return output.stdout.is_empty();
}


fn main() {
    let args: Vec<String> = env::args().collect();

    dbg!(args);

    let cam_path = "/dev/video0";


    let camera: rscam::Camera = launch_camera(cam_path, 30, 640, 480, b"MJPG");

    // for file naming
    // let mut i = 0;

    loop {
        let start = std::time::Instant::now();

        let raw_pixels: Vec<u8>;

        if baremetal() {
            // get frame from actual camera
            println!("Getting frame from camera");
            let frame: rscam::Frame = camera.capture().unwrap();
            raw_pixels = decode_jpeg(&frame);
        } else {
            // get frame from file
            //let frame = std::fs::read("test.jpg").unwrap();
            //raw_pixels = decode_jpeg(&frame);
            raw_pixels = vec![0u8; 640 * 480 * 3];
        }

        // // measure postprocess time (ms)
        let postprocess_start = std::time::Instant::now();

        // raw_pixels = decode_jpeg(&frame);

        //let raw_pixels = decode_jpeg(&frame);
        //let image_tensor = raw_pixels_to_tensor(raw_pixels);
        let image_tensor = raw_pixels_to_tensor(raw_pixels);

        println!("resized pixel_tensor: {:?}", image_tensor.size());

        // save tensor as image
        // let image_tensor_name = format!("images/image_tensor{}.jpg", i);

        // create directory
        //std::fs::create_dir_all("images").unwrap();

        // measure postprocess time in ms
        let postprocess_duration = postprocess_start.elapsed();

        println!("Postprocessing: {:.3} ms", precise_duration_ms(postprocess_duration));

        // measure time
        let duration = start.elapsed();
        println!("frame: {} ms", duration.subsec_millis());
    }
}

#[cfg(test)]
mod tests {
    use tch::Kind;
    use super::{
        raw_pixels_to_tensor,
        launch_camera,
        baremetal,
    };

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
    #[test]
    fn preproc_launch_camera() {
        if !baremetal() {
            // skip test if not baremetal
            assert!(true);
        }
        let width = 640;
        let height = 480;

        let camera: rscam::Camera = launch_camera("/dev/video0", 30, width, height, b"MJPG");
        let frame: rscam::Frame = camera.capture().unwrap();
        assert_eq!(frame.resolution, (width, height));
    }
    #[test]
    fn preproc_raw_pixels_to_tensor() {
        let subpixel_value: u8 = 255; // 0-255 for 8-bit pixel values
        let sum_subpixel_value: u32 = subpixel_value as u32 * 640 * 480 * 3; // we will use this to check that the tensor sum is correct

        // Create a 640x480 image where all pixels are the same value
        let mut pixels = vec![0; 640 * 480 * 3];
        for i in 0..pixels.len() {
            pixels[i] = subpixel_value;
        }

        let image_tensor: tch::Tensor = raw_pixels_to_tensor(pixels);
        println!("image_tensor shape: {:?}", image_tensor.size());
        assert_eq!(image_tensor.size(), &[3, 480, 640]); // C, H, W ordering for tensor

        let tensor_val_sum = image_tensor.sum(Kind::Float).double_value(&[]) as u32;
        println!("Sum of tensor values: {}", tensor_val_sum);
        println!("Sum of subpixel values: {}", sum_subpixel_value);
        assert_eq!(tensor_val_sum, sum_subpixel_value);
    } 
}