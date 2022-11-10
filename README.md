# Dependencies

This project depends on the following libraries:

Install them with apt.

rscam requires

```
libv4l-dev
```

Turbojpeg requires
```
nasm
```

# Image Pre-processing Notes

Optimal strategy so far (1-2ms):

1. Turbojpeg to decompress MJPG to RGBImage
2. Grab raw representation (Vec<u8>) from RGBImage
3. Convert to Tensor
4. Resize Tensor

Slower alternatives

+ Use torchvision to convert from JPEG `tch::vision::image::load_from_memory()`