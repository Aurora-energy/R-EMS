// ---------------------------------------------------------------------------
// Build script for the schemas crate.
//
// Running `cargo build` automatically invokes this script. We use it to compile
// the protobuf definitions located under `services/schemas/proto` into Rust
// modules using `tonic-build`. Keeping generation within the crate ensures all
// services get consistent types without requiring developers to run a separate
// command.
// ---------------------------------------------------------------------------

fn main() {
    // Instruct Cargo to re-run the build script whenever any `.proto` file
    // changes. This keeps generated code fresh during development.
    println!("cargo:rerun-if-changed=proto");

    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .compile(&["proto/ems/core/v1/common.proto"], &["proto"])
        .expect("failed to compile protobufs");
}
