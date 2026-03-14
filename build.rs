fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Jetstream (OrbitFlare custom proto)
    tonic_prost_build::configure()
        .build_server(false)
        .compile_protos(&["protos/jetstream.proto"], &["protos"])?;

    // Yellowstone/Geyser — solana-storage types first, then geyser with extern_path
    tonic_prost_build::configure()
        .build_server(false)
        .compile_protos(&["protos/solana-storage.proto"], &["protos"])?;

    tonic_prost_build::configure()
        .build_server(false)
        .extern_path(
            ".solana.storage.ConfirmedBlock",
            "crate::yellowstone_proto::solana_storage",
        )
        .compile_protos(&["protos/geyser.proto"], &["protos"])?;

    Ok(())
}
