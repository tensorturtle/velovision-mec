use main::raw_pixels_to_tensor;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
    #[test]
    fn raw_zeros_to_tensor() {
        let pixels = vec![0; 640 * 480 * 3];
        // zero out pixels
        for i in 0..pixels.len() {
            pixels[i] = 0;
        }

        // convert to tensor
        let tensor = raw_pixels_to_tensor(pixels);

        // check that all values are zero
        let tensor_values = tensor.double_value(&[0, 0, 0]);
        println!("tensor_values: {}", tensor_values)
        assert_eq!(tensor_values, 0.0);
    } 
}