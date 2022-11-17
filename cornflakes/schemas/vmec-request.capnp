@0xfd248f7aaf6c160e;

struct ReqFrame {
  timestampMs @0 :UInt64;
  deviceHash @1 :Text;
  requestHash @2 :Text;
  images @3 :List(CameraImage);

  struct CameraImage{
    jpegbytes@0 :Data;
    type @1 :CameraDirection;

    enum CameraDirection {
      frontcam @0;
      rearcam @1;
    }
  }
}

struct VmecRequestStruct{
  frame @0 :List(ReqFrame);
}
