use crate::error::CliResult;
use keyring::Entry;

const SERVICE: &str = "orbitflare-cli";

fn entry(profile: &str, key: &str) -> Result<Entry, keyring::Error> {
    Entry::new(&format!("{}.{}", SERVICE, key), profile)
}

pub fn store_secret(profile: &str, key: &str, value: &str) -> CliResult<()> {
    if value.is_empty() {
        let _ = delete_secret(profile, key);
        return Ok(());
    }
    entry(profile, key)?
        .set_password(value)
        .map_err(|e| format!("Failed to store {} in keychain: {}", key, e))?;
    Ok(())
}

pub fn get_secret(profile: &str, key: &str) -> Option<String> {
    entry(profile, key)
        .ok()?
        .get_password()
        .ok()
        .filter(|s| !s.is_empty())
}

pub fn delete_secret(profile: &str, key: &str) -> CliResult<()> {
    match entry(profile, key)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("Failed to remove {} from keychain: {}", key, e).into()),
    }
}

pub fn delete_profile_secrets(profile: &str) -> CliResult<()> {
    let _ = delete_secret(profile, "api_key");
    let _ = delete_secret(profile, "token");
    let _ = delete_secret(profile, "license_key");
    let _ = delete_secret(profile, "apex_key");
    Ok(())
}

pub fn store_profile_secrets(profile: &str, api_key: Option<&str>, token: &str) -> CliResult<()> {
    store_secret(profile, "token", token)?;
    store_secret(profile, "api_key", api_key.unwrap_or(""))?;
    Ok(())
}
