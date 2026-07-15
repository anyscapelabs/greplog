use std::env;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    prost_build::Config::new()
        .out_dir(&out_dir)
        .compile_protos(
            &["proto/greplog/v1/events.proto"],
            &["proto"],
        )
        .expect("Failed to compile protobufs");

    println!("cargo:rerun-if-changed=proto/greplog/v1/events.proto");
}
