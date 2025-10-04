fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Compile the plugin protocol
    tonic_prost_build::compile_protos("proto/plugin.proto")?;

    Ok(())
}
