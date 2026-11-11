//! The secret seed of a node's keypair: random, and kept in a file when the node
//! must be the same node after a restart.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Context;

/// Length of an identity seed, bytes.
pub const SEED_LEN: usize = 32;

/// A fresh random identity seed from the operating system's generator.
pub fn random() -> [u8; SEED_LEN] {
    rand::random()
}

/// Read the identity seed in `path`, or create the file with a random seed.
///
/// A publisher that keeps its seed keeps its stream id across restarts.
///
/// Args:
///     path: Key file. Missing parent directories are created.
///
/// Returns:
///     The seed in the file.
///
/// Raises:
///     An error if the file cannot be read or created, or holds anything other
///     than `SEED_LEN` bytes as hex.
pub fn load_or_create(path: &Path) -> anyhow::Result<[u8; SEED_LEN]> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(text.trim()).with_context(|| format!("identity file {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let seed = random();
            write_new(path, &seed)
                .with_context(|| format!("creating identity file {}", path.display()))?;
            Ok(seed)
        }
        Err(e) => Err(e).with_context(|| format!("reading identity file {}", path.display())),
    }
}

/// The default key file of a role: `~/.bitstream/<role>.key`.
///
/// Returns `None` if the home directory is unknown.
pub fn default_path(role: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(
        PathBuf::from(home)
            .join(".bitstream")
            .join(format!("{role}.key")),
    )
}

fn parse(hex: &str) -> anyhow::Result<[u8; SEED_LEN]> {
    anyhow::ensure!(
        hex.len() == SEED_LEN * 2 && hex.is_ascii(),
        "expected {} hex characters",
        SEED_LEN * 2
    );
    let mut seed = [0u8; SEED_LEN];
    for (i, b) in seed.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).context("not hex")?;
    }
    Ok(seed)
}

fn write_new(path: &Path, seed: &[u8; SEED_LEN]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    // SECURITY: The file holds the secret key seed. Only the owner can read the file.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path)?;
    let hex: String = seed.iter().map(|b| format!("{b:02x}")).collect();
    writeln!(f, "{hex}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_created_seed_is_read_back() {
        let dir = std::env::temp_dir().join(format!("bs-identity-{}", std::process::id()));
        let path = dir.join("nested").join("publisher.key");
        let _ = std::fs::remove_dir_all(&dir);
        let first = load_or_create(&path).unwrap();
        let second = load_or_create(&path).unwrap();
        assert_eq!(first, second, "the seed persists");
        assert_ne!(first, random(), "seeds are random");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "owner-only key file");
        }
        std::fs::write(&path, "not a key").unwrap();
        assert!(load_or_create(&path).is_err(), "a damaged file is an error");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
