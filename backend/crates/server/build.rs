fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(false)
        .compile_protos(
            &["../../../proto/greetings/v1/greetings_service.proto"],
            &["../../../proto"],
        )?;
    println!("cargo:rerun-if-changed=../../../proto/greetings/v1/greetings_service.proto");
    Ok(())
}
