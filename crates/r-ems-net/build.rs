//! ---
//! ems_section: "05-networking-external-interfaces"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Network connectivity and edge adapters."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("failed to locate protoc");
    std::env::set_var("PROTOC", protoc);

    println!("cargo:rerun-if-changed=proto/ems.proto");
    println!("cargo:rerun-if-changed=proto");

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(&["proto/ems.proto"], &["proto"])
        .expect("failed to compile gRPC definitions");
}
