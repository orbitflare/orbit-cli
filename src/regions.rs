pub const RPC_REGIONS: &[&str] = &["va", "ny", "la", "slc", "ams", "fra", "lon", "jp", "sg"];
pub const JETSTREAM_REGIONS: &[&str] =
    &["ny", "slc", "ams", "fra", "lon", "dub", "sqq", "jp", "sgp"];
pub const DEFAULT_REGION: &str = "fra";
pub const DEVNET_RPC_URL: &str = "https://devnet.rpc.orbitflare.com";
pub const DEVNET_GRPC_URL: &str = "http://devnet.rpc.orbitflare.com:10000";

pub fn normalize(region: &str) -> String {
    match region.trim().to_lowercase().as_str() {
        "ash" => "va".into(),
        "nyc" => "ny".into(),
        "tok" | "tyo" => "jp".into(),
        other => other.to_string(),
    }
}

pub fn validate(region: &str) -> Result<String, String> {
    let r = normalize(region);
    if RPC_REGIONS.contains(&r.as_str()) || JETSTREAM_REGIONS.contains(&r.as_str()) {
        Ok(r)
    } else {
        Err(format!(
            "Unknown region '{}'. RPC/Yellowstone: {}. Jetstream: {}",
            region,
            RPC_REGIONS.join(", "),
            JETSTREAM_REGIONS.join(", ")
        ))
    }
}

fn rpc_region(region: &str) -> Result<&'static str, String> {
    let r = normalize(region);
    let mapped = match r.as_str() {
        "sgp" => "sg",
        other => other,
    };
    RPC_REGIONS
        .iter()
        .find(|c| **c == mapped)
        .copied()
        .ok_or_else(|| {
            format!(
                "RPC and Yellowstone gRPC have no '{}' endpoint. Use one of: {}",
                region,
                RPC_REGIONS.join(", ")
            )
        })
}

pub fn jetstream_region(region: &str) -> &'static str {
    let r = normalize(region);
    if let Some(c) = JETSTREAM_REGIONS.iter().find(|c| **c == r) {
        return c;
    }
    match r.as_str() {
        "va" => "ny",
        "la" => "slc",
        "sg" => "sgp",
        _ => DEFAULT_REGION,
    }
}

pub fn rpc_url(region: &str) -> Result<String, String> {
    Ok(format!(
        "https://{}.rpc.orbitflare.com",
        rpc_region(region)?
    ))
}

pub fn grpc_url(region: &str) -> Result<String, String> {
    Ok(format!(
        "http://{}.rpc.orbitflare.com:10000",
        rpc_region(region)?
    ))
}

pub fn jetstream_url(region: &str) -> String {
    format!(
        "http://{}.jetstream.orbitflare.com",
        jetstream_region(region)
    )
}

pub fn validate_network(network: &str) -> Result<(), String> {
    match network {
        "mainnet" | "devnet" => Ok(()),
        other => Err(format!(
            "Unknown network '{}'. OrbitFlare serves mainnet and devnet.",
            other
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_rpc_and_grpc_hosts() {
        for r in RPC_REGIONS {
            assert_eq!(
                rpc_url(r).unwrap(),
                format!("https://{r}.rpc.orbitflare.com")
            );
            assert_eq!(
                grpc_url(r).unwrap(),
                format!("http://{r}.rpc.orbitflare.com:10000")
            );
        }
        assert_eq!(rpc_url("ash").unwrap(), "https://va.rpc.orbitflare.com");
        assert_eq!(rpc_url("sgp").unwrap(), "https://sg.rpc.orbitflare.com");
        assert!(rpc_url("dub").is_err());
        assert!(grpc_url("sqq").is_err());
    }

    #[test]
    fn maps_rpc_regions_to_jetstream() {
        for r in JETSTREAM_REGIONS {
            assert_eq!(
                jetstream_url(r),
                format!("http://{r}.jetstream.orbitflare.com")
            );
        }
        assert_eq!(jetstream_region("va"), "ny");
        assert_eq!(jetstream_region("la"), "slc");
        assert_eq!(jetstream_region("sg"), "sgp");
        assert_eq!(jetstream_region("tok"), "jp");
    }

    #[test]
    fn validates_regions_and_networks() {
        assert_eq!(validate("FRA").unwrap(), "fra");
        assert_eq!(validate("dub").unwrap(), "dub");
        assert!(validate("mainnet").is_err());
        assert!(validate("global").is_err());
        assert!(validate_network("mainnet").is_ok());
        assert!(validate_network("devnet").is_ok());
        assert!(validate_network("testnet").is_err());
    }
}
