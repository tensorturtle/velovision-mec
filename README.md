WORK IN PROGRESS

# velovision-mec

**velovision-mec** is a [Multi-Access Edge Computing](https://portal.etsi.org/Portals/0/TBpages/MEC/Docs/Mobile-edge_Computing_-_Introductory_Technical_White_Paper_V1%2018-09-14.pdf) implementation of [velovision](https://github.com/hydoai/velovision). It is split into two rust binaries: **vmec-server** runs on [Amazon Wavelength](https://aws.amazon.com/wavelength/) servers which are accessed with extremely low latency from 5G devices running **vmec-client**.


[![Rust](https://github.com/tensorturtle/waverust-client/actions/workflows/rust.yml/badge.svg?branch=main)](https://github.com/tensorturtle/waverust-client/actions/workflows/rust.yml)

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

ZMQ
```
libzmq3-dev
```

Capn Proto
```
capnproto
libcapnp-dev
```


# Image Pre-processing Notes

Optimal strategy so far (1-2ms):

1. Turbojpeg to decompress MJPG to RGBImage
2. Grab raw representation (Vec<u8>) from RGBImage
3. Convert to Tensor
4. Resize Tensor

Slower alternatives

+ Use torchvision to convert from JPEG `tch::vision::image::load_from_memory()`
