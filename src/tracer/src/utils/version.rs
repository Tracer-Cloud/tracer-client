use anyhow::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use std::cmp::Ordering;
use std::fmt;

include!(concat!(env!("OUT_DIR"), "/built.rs"));

/// Represents a semantic version with optional build metadata, commit hash, dirty state, and build date.
/// - The version reflects the date of its build.
/// - Release builds: `1.2.3`
/// - Non-release builds: `1.2.3+123`
/// - Dirty state is indicated by the `dirty` flag.
/// - The `commit` field is set for non-release builds if available.
#[derive(Debug)]
pub struct Version {
    major: u32,
    minor: u32,
    patch: u32,
    build: Option<u32>,
    commit: Option<String>,
    dirty: bool,
}

impl Version {
    pub fn current_str() -> &'static str {
        PKG_VERSION
    }

    pub fn current() -> &'static Self {
        static VERSION: Lazy<Version> =
            Lazy::new(|| Version::from_str(Version::current_str()).unwrap());
        &VERSION
    }

    /// Parses a version string from:
    /// - the main branch / release: "1.2.3" or "1.2.3+123"
    /// - custom branch: "1.2.3+213"
    ///
    /// syntax: major.minor.patch[+build][.commit]
    /// Returns an error if the string is not in the correct format
    /// or any part is not a valid number.
    pub(super) fn from_str(s: &str) -> Result<Self, String> {
        static RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"^(\d+)\.(\d+)\.(\d+)(?:\+(\d+))?$").unwrap());

        let err_msg = format!("Failed to parse version string: {}", s);
        let caps = RE.captures(s).ok_or(err_msg.clone())?;

        let major = caps
            .get(1)
            .unwrap()
            .as_str()
            .parse::<u32>()
            .map_err(|_| err_msg.clone())?;
        let minor = caps
            .get(2)
            .unwrap()
            .as_str()
            .parse::<u32>()
            .map_err(|_| err_msg.clone())?;
        let patch = caps
            .get(3)
            .unwrap()
            .as_str()
            .parse::<u32>()
            .map_err(|_| err_msg.clone())?;
        let build = caps.get(4).and_then(|m| m.as_str().parse::<u32>().ok());
        let commit = if PROFILE == "release" && build.is_some() {
            GIT_COMMIT_HASH_SHORT.map(|s| s.to_string())
        } else {
            None
        };
        let dirty = GIT_DIRTY.unwrap_or(false);

        Ok(Self {
            major,
            minor,
            patch,
            build,
            commit,
            dirty,
        })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(build) = self.build {
            write!(f, "+{}", build)?;
        }
        if let Some(commit) = &self.commit {
            write!(f, ".{}", commit)?;
        }
        if self.dirty {
            write!(f, "-dirty")?;
        }
        Ok(())
    }
}

// Implement PartialEq for == and !=
impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.major == other.major
            && self.minor == other.minor
            && self.patch == other.patch
            && self.build == other.build
    }
}

// Implement PartialOrd for <, <=, >, >=
impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some((self.major, self.minor, self.patch, self.build).cmp(&(
            other.major,
            other.minor,
            other.patch,
            other.build,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_versions() {
        let v1 = Version::from_str("1.2.3").unwrap();
        assert_eq!(v1.major, 1);
        assert_eq!(v1.minor, 2);
        assert_eq!(v1.patch, 3);
        assert_eq!(v1.build, None);

        let v2 = Version::from_str("1.2.3+123").unwrap();
        assert_eq!(v2.major, 1);
        assert_eq!(v2.minor, 2);
        assert_eq!(v2.patch, 3);
        assert_eq!(v2.build, Some(123));

        let v4 = Version::from_str("1.2.3+0").unwrap();
        assert_eq!(v4.major, 1);
        assert_eq!(v4.minor, 2);
        assert_eq!(v4.patch, 3);
        assert_eq!(v4.build, Some(0));
    }

    #[test]
    fn test_parse_invalid_versions() {
        assert!(Version::from_str("").is_err());
        assert!(Version::from_str("v1.2").is_err());
        assert!(Version::from_str("1.2.3.4").is_err());
        assert!(Version::from_str("1.a.3").is_err());
        assert!(Version::from_str("version1.2.3").is_err());
        assert!(Version::from_str("+1.2.3").is_err());
        assert!(Version::from_str("1.2.3+abc").is_err()); // build is non-numeric
    }

    #[test]
    fn test_format_version() {
        let version_without_build = Version {
            major: 2,
            minor: 5,
            patch: 9,
            build: None,
            commit: None,
            dirty: false,
        };
        assert_eq!(version_without_build.to_string(), "2.5.9");

        let version_with_build = Version {
            major: 2,
            minor: 5,
            patch: 9,
            build: Some(42),
            commit: None,
            dirty: false,
        };
        assert_eq!(version_with_build.to_string(), "2.5.9+42");
    }

    #[test]
    fn test_version_comparison() {
        let v1 = Version {
            major: 1,
            minor: 0,
            patch: 0,
            build: None,
            commit: None,
            dirty: false,
        };
        let v2 = Version {
            major: 1,
            minor: 0,
            patch: 1,
            build: None,
            commit: None,
            dirty: false,
        };
        let v3 = Version {
            major: 1,
            minor: 1,
            patch: 0,
            build: None,
            commit: None,
            dirty: false,
        };
        let v4 = Version {
            major: 2,
            minor: 0,
            patch: 0,
            build: None,
            commit: None,
            dirty: false,
        };
        let v5 = Version {
            major: 1,
            minor: 0,
            patch: 0,
            build: None,
            commit: None,
            dirty: false,
        };
        let v6 = Version {
            major: 1,
            minor: 0,
            patch: 0,
            build: Some(1),
            commit: None,
            dirty: false,
        };
        let v7 = Version {
            major: 1,
            minor: 0,
            patch: 0,
            build: Some(2),
            commit: None,
            dirty: false,
        };

        // Equality without build
        assert_eq!(v1, v5);

        // Equality with build differs
        assert_ne!(v1, v6);
        assert_ne!(v6, v7);

        // Ordering with builds
        assert!(v1 < v6);
        assert!(v6 < v7);

        // Less than
        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v3 < v4);

        // Less than or equal
        assert!(v1 <= v5);
        assert!(v1 <= v2);

        // Greater than
        assert!(v4 > v3);
        assert!(v3 > v2);

        // Greater than or equal
        assert!(v5 >= v1);
        assert!(v4 >= v3);
    }
}
