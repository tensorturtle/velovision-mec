@0xfd248f7aaf6c160e;

struct ReqFrame {
  timestampMs @0 :UInt64;
  monotonicId @1 :UInt64;
  deviceHash @2 :Text;
  sessionHash @3 :Text;
  images @4 :List(CameraImage);

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
