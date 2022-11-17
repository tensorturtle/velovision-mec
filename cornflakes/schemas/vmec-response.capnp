@0x85775cfa24ed96df;

struct ResFrame {
  timestampMs @0 :UInt64;
  serverHash @1 :Text;
  responseHash @2 :Text;
  neuralOutput @3 :Text;
}

struct VmecResponseStruct{
  frame @0 :List(ResFrame);
}
