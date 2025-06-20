use anyhow::Result;
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

#[derive(Debug)]
pub struct Version {
    major: u32,
    minor: u32,
    patch: u32,
    build: Option<u32>,
}

impl Version {
    pub fn current_str() -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
    pub fn current() -> Self {
        Self::from_str(Self::current_str()).unwrap()
    }
}

impl FromStr for Version {
    type Err = String;

    /// Parses a version string like "1.2.3" or "v1.2.3+4" into a `Version`.
    ///
    /// Returns an error if the string is not in the correct format
    /// or any part is not a valid number.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err_msg = format!("Failed to parse version string: {}", s);

        let s = s.trim_start_matches('v');

        // Split on '+' to get version and optional build
        let mut parts_iter = s.splitn(2, '+');
        let version = parts_iter.next().ok_or_else(|| err_msg.clone())?;
        let build = parts_iter.next().map(String::from);

        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() != 3 {
            return Err(err_msg.clone());
        }

        let major = parts[0].parse::<u32>().ok();
        let minor = parts[1].parse::<u32>().ok();
        let patch = parts[2].parse::<u32>().ok();
        let build = match build {
            Some(b) if !b.is_empty() => Some(b.parse::<u32>().map_err(|_| err_msg.clone())?),
            _ => None,
        };
        match (major, minor, patch) {
            (Some(major), Some(minor), Some(patch)) => Ok(Self {
                major,
                minor,
                patch,
                build,
            }),
            _ => Err(err_msg),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}.{}{}",
            self.major,
            self.minor,
            self.patch,
            self.build.map_or(String::new(), |b| format!("+{}", b))
        )
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
    use std::str::FromStr;

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

        let v4 = Version::from_str("v1.2.3+0").unwrap();
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
        };
        assert_eq!(version_without_build.to_string(), "2.5.9");

        let version_with_build = Version {
            major: 2,
            minor: 5,
            patch: 9,
            build: Some(42),
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
        };
        let v2 = Version {
            major: 1,
            minor: 0,
            patch: 1,
            build: None,
        };
        let v3 = Version {
            major: 1,
            minor: 1,
            patch: 0,
            build: None,
        };
        let v4 = Version {
            major: 2,
            minor: 0,
            patch: 0,
            build: None,
        };
        let v5 = Version {
            major: 1,
            minor: 0,
            patch: 0,
            build: None,
        };
        let v6 = Version {
            major: 1,
            minor: 0,
            patch: 0,
            build: Some(1),
        };
        let v7 = Version {
            major: 1,
            minor: 0,
            patch: 0,
            build: Some(2),
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
