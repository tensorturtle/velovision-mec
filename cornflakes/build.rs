extern crate capnpc;

fn main() {
    capnpc::CompilerCommand::new()
        .file("schemas/vmec-request.capnp")
        .run()
        .expect("schema compiler command");
    
    capnpc::CompilerCommand::new()
        .file("schemas/vmec-response.capnp")
        .run()
        .expect("schema compiler command");
}
