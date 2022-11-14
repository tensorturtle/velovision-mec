extern crate capnpc;

fn main() {
    capnpc::CompilerCommand::new()
        .file("vmec-request.capnp")
        .run()
        .expect("schema compiler command");
}
