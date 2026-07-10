fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::PathBuf::from(
        std::env::var("OUT_DIR").expect("OUT_DIR not set"),
    );

    let proto_dir = std::path::PathBuf::from("proto");
    let proto_file = proto_dir.join("greplog/v1/events.proto");

    println!("cargo:rerun-if-changed={}", proto_file.display());
    println!("cargo:rerun-if-changed=src/redact.rs");
    println!("cargo:rerun-if-changed=src/schema.rs");

    prost_build::Config::new()
        .out_dir(&out)
        .compile_protos(&[&proto_file], &["proto"])
        .expect("protobuf compilation failed");

    Ok(())
}
