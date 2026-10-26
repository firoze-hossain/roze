// tools/roze-pkg/src/semver.rs
//
// Full Semantic Versioning (SemVer 2.0.0) parser, comparator, and requirement matcher.
// Supports standard caret (^), tilde (~), comparison (>=, <=, >, <, =), and wildcard (*) ranges.

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: Option<String>,
    pub build: Option<String>,
}

impl Version {
    #[allow(dead_code)]
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
            pre: None,
            build: None,
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim().trim_start_matches('v').trim_start_matches('=');
        if s.is_empty() {
            return Err(anyhow!("Empty version string"));
        }

        let (ver_pre, build) = match s.split_once('+') {
            Some((vp, b)) => (vp, Some(b.to_string())),
            None => (s, None),
        };

        let (ver_str, pre) = match ver_pre.split_once('-') {
            Some((v, p)) => (v, Some(p.to_string())),
            None => (ver_pre, None),
        };

        let parts: Vec<&str> = ver_str.split('.').collect();
        let major = parts.get(0).ok_or_else(|| anyhow!("Missing major version"))?
            .parse::<u64>().map_err(|_| anyhow!("Invalid major version '{}'", parts[0]))?;

        let minor = match parts.get(1) {
            Some(m) => m.parse::<u64>().map_err(|_| anyhow!("Invalid minor version '{}'", m))?,
            None => 0,
        };

        let patch = match parts.get(2) {
            Some(p) => p.parse::<u64>().map_err(|_| anyhow!("Invalid patch version '{}'", p))?,
            None => 0,
        };

        Ok(Self {
            major,
            minor,
            patch,
            pre,
            build,
        })
    }
}

impl FromStr for Version {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(ref pre) = self.pre {
            write!(f, "-{}", pre)?;
        }
        if let Some(ref build) = self.build {
            write!(f, "+{}", build)?;
        }
        Ok(())
    }
}

impl Serialize for Version {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Version {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Version::parse(&s).map_err(serde::de::Error::custom)
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.major != other.major {
            return self.major.cmp(&other.major);
        }
        if self.minor != other.minor {
            return self.minor.cmp(&other.minor);
        }
        if self.patch != other.patch {
            return self.patch.cmp(&other.patch);
        }

        // SemVer 2.0.0 Pre-release comparison:
        // A normal version has greater precedence than a pre-release version.
        match (&self.pre, &other.pre) {
            (None, None) => Ordering::Equal,
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (Some(pre_a), Some(pre_b)) => compare_pre_release(pre_a, pre_b),
        }
        // Build metadata is ignored in precedence.
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn compare_pre_release(a: &str, b: &str) -> Ordering {
    let parts_a: Vec<&str> = a.split('.').collect();
    let parts_b: Vec<&str> = b.split('.').collect();
    let min_len = parts_a.len().min(parts_b.len());

    for i in 0..min_len {
        let p_a = parts_a[i];
        let p_b = parts_b[i];

        let num_a = p_a.parse::<u64>();
        let num_b = p_b.parse::<u64>();

        match (num_a, num_b) {
            (Ok(na), Ok(nb)) => {
                if na != nb {
                    return na.cmp(&nb);
                }
            }
            (Ok(_), Err(_)) => return Ordering::Less, // Numeric < Alphanumeric
            (Err(_), Ok(_)) => return Ordering::Greater,
            (Err(_), Err(_)) => {
                if p_a != p_b {
                    return p_a.cmp(p_b);
                }
            }
        }
    }

    parts_a.len().cmp(&parts_b.len())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Exact,      // =
    Greater,    // >
    GreaterEq,  // >=
    Less,       // <
    LessEq,     // <=
    Tilde,      // ~
    Caret,      // ^
    Wildcard,   // *
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparator {
    pub op: Op,
    pub major: u64,
    pub minor: Option<u64>,
    pub patch: Option<u64>,
    pub pre: Option<String>,
}

impl Comparator {
    pub fn parse(mut s: &str) -> Result<Self> {
        s = s.trim();
        if s.is_empty() || s == "*" || s == "x" || s == "X" {
            return Ok(Comparator {
                op: Op::Wildcard,
                major: 0,
                minor: None,
                patch: None,
                pre: None,
            });
        }

        let op = if s.starts_with(">=") {
            s = &s[2..].trim_start();
            Op::GreaterEq
        } else if s.starts_with("<=") {
            s = &s[2..].trim_start();
            Op::LessEq
        } else if s.starts_with('>') {
            s = &s[1..].trim_start();
            Op::Greater
        } else if s.starts_with('<') {
            s = &s[1..].trim_start();
            Op::Less
        } else if s.starts_with('=') {
            s = &s[1..].trim_start();
            Op::Exact
        } else if s.starts_with('~') {
            s = &s[1..].trim_start();
            Op::Tilde
        } else if s.starts_with('^') {
            s = &s[1..].trim_start();
            Op::Caret
        } else {
            Op::Caret // Default operator in Roze / Cargo package manager
        };

        let (ver_part, pre_part) = match s.split_once('-') {
            Some((v, p)) => (v, Some(p.to_string())),
            None => {
                // Check if build metadata exists without pre
                let v = match s.split_once('+') {
                    Some((vp, _)) => vp,
                    None => s,
                };
                (v, None)
            }
        };

        let parts: Vec<&str> = ver_part.split('.').collect();
        let major = parts[0].parse::<u64>().map_err(|_| anyhow!("Invalid comparator major: '{}'", parts[0]))?;

        let minor = match parts.get(1) {
            Some(&"*") | Some(&"x") | Some(&"X") | None => None,
            Some(m) => Some(m.parse::<u64>().map_err(|_| anyhow!("Invalid comparator minor: '{}'", m))?),
        };

        let patch = match parts.get(2) {
            Some(&"*") | Some(&"x") | Some(&"X") | None => None,
            Some(p) => Some(p.parse::<u64>().map_err(|_| anyhow!("Invalid comparator patch: '{}'", p))?),
        };

        Ok(Comparator {
            op,
            major,
            minor,
            patch,
            pre: pre_part,
        })
    }

    pub fn matches(&self, ver: &Version) -> bool {
        match self.op {
            Op::Wildcard => true,
            Op::Exact => {
                let m = ver.major == self.major;
                let mi = self.minor.map_or(true, |mi| ver.minor == mi);
                let p = self.patch.map_or(true, |p| ver.patch == p);
                m && mi && p && (self.pre.is_none() || ver.pre == self.pre)
            }
            Op::Greater => {
                let cmp_ver = Version {
                    major: self.major,
                    minor: self.minor.unwrap_or(0),
                    patch: self.patch.unwrap_or(0),
                    pre: self.pre.clone(),
                    build: None,
                };
                ver > &cmp_ver
            }
            Op::GreaterEq => {
                let cmp_ver = Version {
                    major: self.major,
                    minor: self.minor.unwrap_or(0),
                    patch: self.patch.unwrap_or(0),
                    pre: self.pre.clone(),
                    build: None,
                };
                ver >= &cmp_ver
            }
            Op::Less => {
                let cmp_ver = Version {
                    major: self.major,
                    minor: self.minor.unwrap_or(0),
                    patch: self.patch.unwrap_or(0),
                    pre: self.pre.clone(),
                    build: None,
                };
                ver < &cmp_ver
            }
            Op::LessEq => {
                let cmp_ver = Version {
                    major: self.major,
                    minor: self.minor.unwrap_or(0),
                    patch: self.patch.unwrap_or(0),
                    pre: self.pre.clone(),
                    build: None,
                };
                ver <= &cmp_ver
            }
            Op::Caret => {
                // Caret requirements:
                // ^1.2.3 := >= 1.2.3, < 2.0.0
                // ^0.2.3 := >= 0.2.3, < 0.3.0
                // ^0.0.3 := >= 0.0.3, < 0.0.4
                let min_ver = Version {
                    major: self.major,
                    minor: self.minor.unwrap_or(0),
                    patch: self.patch.unwrap_or(0),
                    pre: self.pre.clone(),
                    build: None,
                };
                if ver < &min_ver {
                    return false;
                }

                if self.major > 0 {
                    ver.major == self.major
                } else if let Some(min) = self.minor {
                    if min > 0 {
                        ver.major == 0 && ver.minor == min
                    } else if let Some(pat) = self.patch {
                        ver.major == 0 && ver.minor == 0 && ver.patch == pat
                    } else {
                        ver.major == 0 && ver.minor == 0
                    }
                } else {
                    ver.major == 0
                }
            }
            Op::Tilde => {
                // Tilde requirements:
                // ~1.2.3 := >= 1.2.3, < 1.3.0
                // ~1.2   := >= 1.2.0, < 1.3.0
                // ~1     := >= 1.0.0, < 2.0.0
                let min_ver = Version {
                    major: self.major,
                    minor: self.minor.unwrap_or(0),
                    patch: self.patch.unwrap_or(0),
                    pre: self.pre.clone(),
                    build: None,
                };
                if ver < &min_ver {
                    return false;
                }

                if let Some(min) = self.minor {
                    ver.major == self.major && ver.minor == min
                } else {
                    ver.major == self.major
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionReq {
    pub comparators: Vec<Comparator>,
    raw: String,
}

impl VersionReq {
    #[allow(dead_code)]
    pub const STAR: &'static str = "*";

    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim();
        if s.is_empty() || s == "*" {
            return Ok(Self {
                comparators: vec![Comparator {
                    op: Op::Wildcard,
                    major: 0,
                    minor: None,
                    patch: None,
                    pre: None,
                }],
                raw: "*".to_string(),
            });
        }

        let mut comparators = Vec::new();
        // Split by comma: e.g. ">= 1.2.0, < 2.0.0"
        for part in s.split(',') {
            let part = part.trim();
            if !part.is_empty() {
                comparators.push(Comparator::parse(part)?);
            }
        }

        if comparators.is_empty() {
            comparators.push(Comparator {
                op: Op::Wildcard,
                major: 0,
                minor: None,
                patch: None,
                pre: None,
            });
        }

        Ok(Self {
            comparators,
            raw: s.to_string(),
        })
    }

    pub fn matches(&self, ver: &Version) -> bool {
        self.comparators.iter().all(|cmp| cmp.matches(ver))
    }

    #[allow(dead_code)]
    pub fn as_str(&self) -> &str {
        &self.raw
    }
}

impl FromStr for VersionReq {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s)
    }
}

impl fmt::Display for VersionReq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl Serialize for VersionReq {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.raw)
    }
}

impl<'de> Deserialize<'de> for VersionReq {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        VersionReq::parse(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parse_and_display() {
        let v = Version::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.pre, None);
        assert_eq!(v.to_string(), "1.2.3");

        let v2 = Version::parse("2.0.0-rc.1+build.123").unwrap();
        assert_eq!(v2.major, 2);
        assert_eq!(v2.minor, 0);
        assert_eq!(v2.patch, 0);
        assert_eq!(v2.pre.as_deref(), Some("rc.1"));
        assert_eq!(v2.build.as_deref(), Some("build.123"));
        assert_eq!(v2.to_string(), "2.0.0-rc.1+build.123");
    }

    #[test]
    fn test_version_ordering() {
        let v1 = Version::parse("1.0.0-alpha").unwrap();
        let v2 = Version::parse("1.0.0-alpha.1").unwrap();
        let v3 = Version::parse("1.0.0-beta").unwrap();
        let v4 = Version::parse("1.0.0-beta.2").unwrap();
        let v5 = Version::parse("1.0.0-beta.11").unwrap();
        let v6 = Version::parse("1.0.0-rc.1").unwrap();
        let v7 = Version::parse("1.0.0").unwrap();
        let v8 = Version::parse("1.0.1").unwrap();
        let v9 = Version::parse("1.1.0").unwrap();
        let v10 = Version::parse("2.0.0").unwrap();

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v3 < v4);
        assert!(v4 < v5);
        assert!(v5 < v6);
        assert!(v6 < v7);
        assert!(v7 < v8);
        assert!(v8 < v9);
        assert!(v9 < v10);
    }

    #[test]
    fn test_caret_requirements() {
        let req = VersionReq::parse("^1.2.3").unwrap();
        assert!(req.matches(&Version::parse("1.2.3").unwrap()));
        assert!(req.matches(&Version::parse("1.2.4").unwrap()));
        assert!(req.matches(&Version::parse("1.9.0").unwrap()));
        assert!(!req.matches(&Version::parse("1.2.2").unwrap()));
        assert!(!req.matches(&Version::parse("2.0.0").unwrap()));

        let req_zero_minor = VersionReq::parse("^0.2.3").unwrap();
        assert!(req_zero_minor.matches(&Version::parse("0.2.3").unwrap()));
        assert!(req_zero_minor.matches(&Version::parse("0.2.5").unwrap()));
        assert!(!req_zero_minor.matches(&Version::parse("0.3.0").unwrap()));
        assert!(!req_zero_minor.matches(&Version::parse("0.2.2").unwrap()));

        let req_zero_patch = VersionReq::parse("^0.0.3").unwrap();
        assert!(req_zero_patch.matches(&Version::parse("0.0.3").unwrap()));
        assert!(!req_zero_patch.matches(&Version::parse("0.0.4").unwrap()));
    }

    #[test]
    fn test_tilde_requirements() {
        let req = VersionReq::parse("~1.2.3").unwrap();
        assert!(req.matches(&Version::parse("1.2.3").unwrap()));
        assert!(req.matches(&Version::parse("1.2.9").unwrap()));
        assert!(!req.matches(&Version::parse("1.3.0").unwrap()));
        assert!(!req.matches(&Version::parse("1.2.2").unwrap()));

        let req_major = VersionReq::parse("~1").unwrap();
        assert!(req_major.matches(&Version::parse("1.0.0").unwrap()));
        assert!(req_major.matches(&Version::parse("1.9.0").unwrap()));
        assert!(!req_major.matches(&Version::parse("2.0.0").unwrap()));
    }

    #[test]
    fn test_range_and_wildcard() {
        let req = VersionReq::parse(">= 1.0.0, < 2.0.0").unwrap();
        assert!(req.matches(&Version::parse("1.0.0").unwrap()));
        assert!(req.matches(&Version::parse("1.5.0").unwrap()));
        assert!(!req.matches(&Version::parse("0.9.9").unwrap()));
        assert!(!req.matches(&Version::parse("2.0.0").unwrap()));

        let wildcard = VersionReq::parse("*").unwrap();
        assert!(wildcard.matches(&Version::parse("0.0.1").unwrap()));
        assert!(wildcard.matches(&Version::parse("99.0.0").unwrap()));
    }
}
