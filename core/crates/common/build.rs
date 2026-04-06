fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?)
        .join("../../..")
        .canonicalize()?;
    let proto_dir = repo_root.join("proto");
    let proto_file = proto_dir.join("openpinch.proto");
    let protoc = protoc_bin_vendored::protoc_bin_path()?;

    println!("cargo:rerun-if-changed={}", proto_file.display());
    println!("cargo:rerun-if-changed={}", proto_dir.display());

    unsafe {
        std::env::set_var("PROTOC", protoc);
    }

    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .compile_protos(&[proto_file], &[proto_dir])?;

    Ok(())
}
