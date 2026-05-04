fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(false)
        .compile_protos(
            &["../../../proto/users/v1/users_service.proto"],
            &["../../../proto"],
        )?;
    println!("cargo:rerun-if-changed=../../../proto/users/v1/users_service.proto");
    Ok(())
}
