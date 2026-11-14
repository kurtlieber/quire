//! This fork's own releases (FORK.md, "Releases"). A fix made here ships as upstream's version
//! plus `-kl.N`, such as `0.5.6-kl.1`, before upstream's next release. Upstream's updater reads
//! only three numbers, so `0.5.6-kl.1` would never be offered over `0.5.6`. Here the fork's N is
//! a fourth number: `0.5.6` < `0.5.6-kl.1` < `0.5.6-kl.2` < `0.5.7`.
//!
//! The fork's changes to `update.rs` are two small hunks that call this file, so an upstream
//! change there rarely conflicts with them.

/// What this build calls itself. release.yml builds with `TAG` set to the release's tag. That
/// tag, without its `v`, is the version when it is Cargo's version, alone or with `-kl.N`.
/// Any other tag, and any build outside release.yml, gives Cargo's version. `desktop/Cargo.toml`
/// stays upstream's, so an upstream release never conflicts with an own one there.
pub const VERSION: &str = match option_env!("TAG") {
    Some(tag) => stamp(tag, env!("CARGO_PKG_VERSION")),
    None => env!("CARGO_PKG_VERSION"),
};

const SUFFIX: &[u8] = b"-kl.";

/// `tag` without its `v` when it is `v` + `cargo`, alone or with `-kl.N`; otherwise `cargo`.
const fn stamp(tag: &'static str, cargo: &'static str) -> &'static str {
    let t = tag.as_bytes();
    let c = cargo.as_bytes();
    if t.len() < 1 + c.len() || t[0] != b'v' {
        return cargo;
    }
    let mut i = 0;
    while i < c.len() {
        if t[1 + i] != c[i] {
            return cargo;
        }
        i += 1;
    }
    let mut i = 1 + c.len();
    if i < t.len() {
        // The rest must be "-kl." and at least one digit.
        if t.len() < i + SUFFIX.len() + 1 {
            return cargo;
        }
        let mut j = 0;
        while j < SUFFIX.len() {
            if t[i + j] != SUFFIX[j] {
                return cargo;
            }
            j += 1;
        }
        i += SUFFIX.len();
        while i < t.len() {
            if !t[i].is_ascii_digit() {
                return cargo;
            }
            i += 1;
        }
    }
    match std::str::from_utf8(t.split_at(1).1) {
        Ok(v) => v,
        Err(_) => cargo,
    }
}

/// A version as four numbers: upstream's three, then N of `-kl.N`, 0 without it. Another
/// suffix counts as 0, as upstream's `parse_version` drops it.
pub fn parse(s: &str) -> Option<(u64, u64, u64, u64)> {
    let (x, y, z) = super::parse_version(s)?;
    let core = s.trim().split('+').next()?;
    let n = match core.split_once("-kl.") {
        Some((_, n)) => n.parse().ok()?,
        None => 0,
    };
    Some((x, y, z, n))
}

/// Whether `latest` is newer than `current`, by four numbers.
pub fn newer(latest: &str, current: &str) -> bool {
    matches!((parse(latest), parse(current)), (Some(a), Some(b)) if a > b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_numbers() {
        assert_eq!(parse("v0.5.6"), Some((0, 5, 6, 0)));
        assert_eq!(parse("0.5.6-kl.2"), Some((0, 5, 6, 2)));
        assert_eq!(parse("0.5.6-kl.10+build"), Some((0, 5, 6, 10)));
        assert_eq!(parse("0.5.6-beta"), Some((0, 5, 6, 0)));
        assert_eq!(parse("0.5.6-kl.x"), None);
        assert_eq!(parse("0.5"), None);
        assert!(newer("0.5.6-kl.1", "0.5.6"));
        assert!(newer("0.5.6-kl.10", "0.5.6-kl.9"));
        assert!(newer("0.5.7", "0.5.6-kl.3"));
        assert!(!newer("0.5.6", "0.5.6-kl.1"));
        assert!(!newer("0.5.6-kl.1", "0.5.6-kl.1"));
        assert!(!newer("0.5.6-kl.x", "0.5.6"));
    }

    #[test]
    fn stamps() {
        assert_eq!(stamp("v0.5.6", "0.5.6"), "0.5.6");
        assert_eq!(stamp("v0.5.6-kl.1", "0.5.6"), "0.5.6-kl.1");
        assert_eq!(stamp("v0.5.6-kl.12", "0.5.6"), "0.5.6-kl.12");
        // Anything else is Cargo's version.
        for tag in [
            "v0.5.7",
            "v0.5.7-kl.1",
            "0.5.6",
            "v0.5.6-kl.",
            "v0.5.6-kl.1a",
            "v0.5.6-rc.1",
            "v0.5",
            "main",
            "",
        ] {
            assert_eq!(stamp(tag, "0.5.6"), "0.5.6", "{tag}");
        }
    }

    #[test]
    fn this_build() {
        assert!(parse(VERSION).is_some());
        assert_eq!(super::super::CURRENT, VERSION);
    }
}
