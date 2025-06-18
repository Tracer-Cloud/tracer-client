use anyhow::{anyhow, Result};
use std::cmp::Ordering;
use std::fmt;

#[derive(Debug)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    /// Parses a version string like "1.2.3" or "v1.2.3" into a `Version`.
    ///
    /// Ignores any build metadata after a '+' sign.
    ///
    /// Returns an error if the string is not in the correct format
    /// or any part is not a valid number
    pub fn from_str(s: &str) -> Result<Self> {
        let message = format!("Failed to parse version string: {}", s);
        let s = s.trim_start_matches('v');
        let version = s
            .split('+')
            .next()
            .ok_or_else(|| anyhow!(message.clone()))?;
        let parts: Vec<&str> = version.split('.').collect();

        if parts.len() != 3 {
            return Err(anyhow!(message.clone()));
        }

        let major = parts[0].parse::<u32>().ok();
        let minor = parts[1].parse::<u32>().ok();
        let patch = parts[2].parse::<u32>().ok();

        match (major, minor, patch) {
            (Some(major), Some(minor), Some(patch)) => Ok(Self {
                major,
                minor,
                patch,
            }),
            _ => Err(anyhow!(message)),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

// Implement PartialEq for == and !=
impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.major == other.major && self.minor == other.minor && self.patch == other.patch
    }
}

// Implement PartialOrd for <, <=, >, >=
impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some((self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch)))
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

        let v2 = Version::from_str("v10.20.30").unwrap();
        assert_eq!(v2.major, 10);
        assert_eq!(v2.minor, 20);
        assert_eq!(v2.patch, 30);

        let v3 = Version::from_str("1.2.3+build123").unwrap();
        assert_eq!(v3.major, 1);
        assert_eq!(v3.minor, 2);
        assert_eq!(v3.patch, 3);
    }

    #[test]
    fn test_parse_invalid_versions() {
        assert!(Version::from_str("").is_err());
        assert!(Version::from_str("v1.2").is_err());
        assert!(Version::from_str("1.2.3.4").is_err());
        assert!(Version::from_str("1.a.3").is_err());
        assert!(Version::from_str("version1.2.3").is_err());
        assert!(Version::from_str("+1.2.3").is_err());
    }

    #[test]
    fn test_format_version() {
        let version = Version {
            major: 2,
            minor: 5,
            patch: 9,
        };
        let s = version.to_string();
        assert_eq!(s, "2.5.9");
    }

    #[test]
    fn test_version_comparison() {
        let v1 = Version {
            major: 1,
            minor: 0,
            patch: 0,
        };
        let v2 = Version {
            major: 1,
            minor: 0,
            patch: 1,
        };
        let v3 = Version {
            major: 1,
            minor: 1,
            patch: 0,
        };
        let v4 = Version {
            major: 2,
            minor: 0,
            patch: 0,
        };
        let v5 = Version {
            major: 1,
            minor: 0,
            patch: 0,
        };

        // Equality
        assert_eq!(v1, v5);

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
