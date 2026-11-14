//! Notes locked with a password. A locked note is still a `.md` file in the folder, but its
//! text is sealed with AES-256-GCM, with a key from the password by PBKDF2-HMAC-SHA256. The
//! file holds a header line, a sentence for other apps, and the sealed text in base64:
//!
//! ```text
//! %QUIRE-LOCKED-NOTE v1 pbkdf2-sha256 600000 <salt, base64>
//!
//! This note is locked with a password. Open it in Typo to read it.
//!
//! <the sealed text, base64, 76 characters a line>
//! ```
//!
//! Sync, search and the link index see only this text. The plain text lives in memory while
//! the note is unlocked, and each save seals it again with a new nonce.

use base64::Engine;

use crate::sync::crypto::{self, Keys};

pub const MAGIC: &str = "%QUIRE-LOCKED-NOTE";
const NOTICE: &str = "This note is locked with a password. Open it in Typo to read it.";
/// A password shorter than this is refused.
pub const MIN_PASSWORD: usize = 8;

/// What it takes to seal a note again without the password: the key and its salt.
#[derive(Clone)]
pub struct Key {
    keys: Keys,
    salt: Vec<u8>,
    iterations: u32,
}

/// Prints no key material.
impl std::fmt::Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Key { .. }")
    }
}

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

/// Whether a note's text is a locked note.
pub fn is_locked(text: &str) -> bool {
    text.starts_with(MAGIC)
}

/// A new key for `password`, with a new salt.
pub fn new_key(password: &str) -> Key {
    let salt = crypto::random(16);
    Key { keys: crypto::derive(password, &salt, crypto::ITERATIONS), salt, iterations: crypto::ITERATIONS }
}

/// The text of the locked note that holds `plain`.
pub fn seal(key: &Key, plain: &str) -> String {
    let body = b64().encode(crypto::seal(&key.keys, plain.as_bytes()));
    let mut out = format!("{MAGIC} v1 pbkdf2-sha256 {} {}\n\n{NOTICE}\n\n", key.iterations, b64().encode(&key.salt));
    for (i, chunk) in body.as_bytes().chunks(76).enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(std::str::from_utf8(chunk).unwrap_or(""));
    }
    out.push('\n');
    out
}

/// The salt, the iteration count and the sealed bytes of a locked note.
fn parts(text: &str) -> Result<(Vec<u8>, u32, Vec<u8>), String> {
    let bad = || "This note is not in the locked note format.".to_string();
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().ok_or_else(bad)?.split_whitespace().collect();
    match head.as_slice() {
        [m, "v1", "pbkdf2-sha256", iterations, salt] if *m == MAGIC => {
            let iterations: u32 = iterations.parse().map_err(|_| bad())?;
            let salt = b64().decode(salt).map_err(|_| bad())?;
            // The body is every base64 line after the notice, joined. The notice is a sentence,
            // and base64 has no spaces, so Quire's notice from before the rename drops out too.
            let body: String = lines.map(str::trim).filter(|l| !l.is_empty() && !l.contains(' ')).collect();
            let sealed = b64().decode(body).map_err(|_| bad())?;
            Ok((salt, iterations, sealed))
        }
        [m, version, ..] if *m == MAGIC => Err(format!("This note is locked with a newer format ({version}). Update Typo to open it.")),
        _ => Err(bad()),
    }
}

/// The plain text and the key of a locked note, when the password is right.
pub fn open(text: &str, password: &str) -> Result<(String, Key), String> {
    let (salt, iterations, sealed) = parts(text)?;
    let keys = crypto::derive(password, &salt, iterations);
    let plain = crypto::open(&keys, &sealed).map_err(|_| "The password does not open this note.".to_string())?;
    let plain = String::from_utf8(plain).map_err(|_| "The note is not text.".to_string())?;
    Ok((plain, Key { keys, salt, iterations }))
}

/// The plain text of a locked note with a key from before, for a note that changed on disk
/// while it was unlocked. Fails when another device locked it with another password.
pub fn open_with(key: &Key, text: &str) -> Result<String, String> {
    let (salt, _, sealed) = parts(text)?;
    if salt != key.salt {
        return Err("The note has another password now.".into());
    }
    let plain = crypto::open(&key.keys, &sealed).map_err(|_| "The note has another password now.".to_string())?;
    String::from_utf8(plain).map_err(|_| "The note is not text.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_locked_note_opens_with_its_password_only() {
        let key = Key { keys: crypto::derive("tiger lily", b"0123456789abcdef", 1000), salt: b"0123456789abcdef".to_vec(), iterations: 1000 };
        let text = "# Diary\n\nBlåbær and [[links]] #private\n";
        let locked = seal(&key, text);
        assert!(is_locked(&locked));
        assert!(locked.contains(NOTICE));
        assert!(!locked.contains("Diary") && !locked.contains("#private") && !locked.contains("[["), "no plain text, tags or links show");
        assert!(locked.lines().all(|l| l.len() <= 76 || l.starts_with(MAGIC)));
        let (plain, k2) = open(&locked, "tiger lily").unwrap();
        assert_eq!(plain, text);
        assert_eq!(open_with(&k2, &seal(&k2, "edited")).unwrap(), "edited");
        assert_eq!(open(&locked, "wrong pass").unwrap_err(), "The password does not open this note.");
        let other = Key { keys: crypto::derive("other", b"fedcba9876543210", 1000), salt: b"fedcba9876543210".to_vec(), iterations: 1000 };
        assert!(open_with(&key, &seal(&other, "x")).is_err());
        assert!(open(&locked.replace(" v1 ", " v9 "), "tiger lily").unwrap_err().contains("newer format"));
        // A note Quire locked, before the rename to Typo, opens too.
        let quire = locked.replace(NOTICE, "This note is locked with a password. Open it in Quire to read it.");
        assert_ne!(quire, locked);
        assert_eq!(open(&quire, "tiger lily").unwrap().0, text);
        // Two seals of one text differ, because each has its own nonce.
        assert_ne!(seal(&key, text), seal(&key, text));
        assert!(!is_locked("# A note"));
    }
}

#[cfg(test)]
mod vector {
    use super::*;

    /// Prints a locked note for other implementations: `cargo test print_locked_note -- --nocapture`.
    #[test]
    fn print_locked_note() {
        let key = Key { keys: crypto::derive("tiger lily", b"0123456789abcdef", 1000), salt: b"0123456789abcdef".to_vec(), iterations: 1000 };
        print!("{}", seal(&key, "# Diary\n\nBlåbær\n"));
    }
}
