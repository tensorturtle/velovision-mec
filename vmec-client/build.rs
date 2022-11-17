extern crate capnpc;

fn main() {
    capnpc::CompilerCommand::new()
        .file("../cornflakes/vmec-request.capnp")
        .run()
        .expect("schema compiler command");
    
    capnpc::CompilerCommand::new()
        .file("../cornflakes/vmec-response.capnp")
        .run()
        .expect("schema compiler command");
}
