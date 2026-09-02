use age::x25519::{Identity, Recipient};
use anyhow::{anyhow, bail, Context, Result};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::model::Document;

pub fn key_path() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("TDEE_KEY_FILE") {
        return Ok(PathBuf::from(path));
    }
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow!("Cannot determine home directory; set TDEE_KEY_FILE"))?;
    Ok(home.join(".hermes/secrets/tdee-age-key.txt"))
}

fn read_identity(key_path: &Path) -> Result<Identity> {
    let contents = std::fs::read_to_string(key_path).with_context(|| {
        format!(
            "Cannot read key file: {}\n  \
            Hint: set TDEE_KEY_FILE or place key at ~/.hermes/secrets/tdee-age-key.txt",
            key_path.display()
        )
    })?;
    let key_str = contents
        .lines()
        .find(|line| line.starts_with("AGE-SECRET-KEY-"))
        .ok_or_else(|| {
            anyhow!(
                "No AGE-SECRET-KEY-1... line found in key file: {}",
                key_path.display()
            )
        })?;
    Identity::from_str(key_str)
        .map_err(|e| anyhow!("Failed to parse identity from {}: {:?}", key_path.display(), e))
}

fn read_recipient(key_path: &Path) -> Result<Recipient> {
    let contents = std::fs::read_to_string(key_path)
        .with_context(|| format!("Cannot read key file: {}", key_path.display()))?;
    let pubkey = contents
        .lines()
        .find(|line| line.starts_with("# public key: "))
        .and_then(|line| line.strip_prefix("# public key: "))
        .ok_or_else(|| {
            anyhow!(
                "No '# public key: age1...' comment found in key file: {}",
                key_path.display()
            )
        })?;
    pubkey
        .trim()
        .parse::<Recipient>()
        .map_err(|e| anyhow!("Failed to parse recipient from key file: {:?}", e))
}

pub fn load_encrypted(path: &Path, key_path: &Path) -> Result<Document> {
    let encrypted = std::fs::read(path)
        .with_context(|| format!("Failed to read encrypted data file: {}", path.display()))?;
    let identity = read_identity(key_path)?;
    let decryptor = match age::Decryptor::new(&encrypted[..])
        .map_err(|e| anyhow!("Failed to parse age header in {}: {}", path.display(), e))?
    {
        age::Decryptor::Recipients(d) => d,
        age::Decryptor::Passphrase(_) => bail!(
            "Data file {} was encrypted with a passphrase; expected recipient encryption",
            path.display()
        ),
    };
    let mut decrypted = Vec::new();
    let mut reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|e| {
            anyhow!(
                "Decryption failed for {}: {}\n  \
                Check that your key matches the file's recipient",
                path.display(),
                e
            )
        })?;
    reader
        .read_to_end(&mut decrypted)
        .with_context(|| format!("Failed to read decrypted data from {}", path.display()))?;
    serde_json::from_slice(&decrypted)
        .with_context(|| format!("Failed to parse JSON from decrypted {}", path.display()))
}

pub fn save_encrypted(data: &Document, path: &Path, key_path: &Path) -> Result<()> {
    let recipient = read_recipient(key_path)?;
    let json = serde_json::to_vec_pretty(data)?;
    let encryptor = age::Encryptor::with_recipients(vec![
        Box::new(recipient) as Box<dyn age::Recipient + Send>,
    ])
    .ok_or_else(|| anyhow!("Failed to create encryptor (empty recipient list)"))?;
    let mut encrypted = Vec::new();
    let mut writer = encryptor
        .wrap_output(&mut encrypted)
        .map_err(|e| anyhow!("Failed to initialise encryption: {}", e))?;
    writer
        .write_all(&json)
        .context("Failed to write plaintext to encryptor")?;
    writer.finish().context("Failed to finalise encryption")?;
    let tmp_path = PathBuf::from(format!("{}.tmp", path.display()));
    std::fs::write(&tmp_path, &encrypted)
        .with_context(|| format!("Failed to write temp file: {}", tmp_path.display()))?;
    std::fs::rename(&tmp_path, path).with_context(|| {
        format!(
            "Failed to rename {} to {}",
            tmp_path.display(),
            path.display()
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Config, Entry, Sex};
    use chrono::NaiveDate;
    use tempfile::NamedTempFile;

    fn test_doc() -> Document {
        Document {
            config: Config {
                start_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                start_weight_lb: 200.0,
                goal_weight_lb: 180.0,
                rate_lb_per_week: 1.0,
                height_cm: 180.0,
                age_years: 35,
                sex: Sex::Male,
                seed_tdee_kcal: 2328,
                ema_alpha: 0.1,
                window_days: 28,
            },
            entries: vec![Entry {
                date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                weight_lb: Some(199.6),
                kcal: Some(1900),
                note: None,
            }],
        }
    }

    fn write_temp_key() -> NamedTempFile {
        use secrecy::ExposeSecret;
        let identity = age::x25519::Identity::generate();
        let pubkey = identity.to_public().to_string();
        let secret = identity.to_string().expose_secret().clone();
        let content = format!(
            "# created: 2026-01-01T00:00:00Z\n# public key: {}\n{}\n",
            pubkey, secret
        );
        let f = NamedTempFile::new().unwrap();
        std::fs::write(f.path(), &content).unwrap();
        f
    }

    #[test]
    fn roundtrip_encrypt_decrypt() {
        let key_file = write_temp_key();
        let data_file = NamedTempFile::new().unwrap();
        let path = data_file.path().with_extension("age");
        let original = test_doc();
        save_encrypted(&original, &path, key_file.path()).unwrap();
        let recovered = load_encrypted(&path, key_file.path()).unwrap();
        assert_eq!(recovered.entries.len(), original.entries.len());
        assert_eq!(
            recovered.entries[0].weight_lb,
            original.entries[0].weight_lb
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_key_file_returns_clear_error() {
        let data_file = NamedTempFile::new().unwrap();
        let path = data_file.path().with_extension("age");
        let missing_key = PathBuf::from("/tmp/nonexistent-key-99999.txt");
        let data = test_doc();
        let err = save_encrypted(&data, &path, &missing_key).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("Cannot read key file") || msg.contains("No such file"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn corrupt_age_file_returns_clear_error() {
        let key_file = write_temp_key();
        let data_file = NamedTempFile::new().unwrap();
        let path = data_file.path().with_extension("age");
        std::fs::write(&path, b"this is not valid age ciphertext").unwrap();
        let err = load_encrypted(&path, key_file.path()).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("Failed to parse age header") || msg.contains("age header"),
            "unexpected error: {msg}"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn key_discovery_env_var() {
        let key_file = write_temp_key();
        std::env::set_var("TDEE_KEY_FILE", key_file.path().to_str().unwrap());
        let found = key_path().unwrap();
        assert_eq!(found, key_file.path());
        std::env::remove_var("TDEE_KEY_FILE");
    }
}
