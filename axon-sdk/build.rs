fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_client(true)
        .compile(
            &["../proto/axon_manifest.proto", "../proto/axon_handshake.proto"],
            &["../proto"],
        )?;
    Ok(())
}
