//! Version parsing and comparison.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: Option<String>,
}

impl Version {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('v');
        let (version_part, pre_part) = match s.split_once('-') {
            Some((v, p)) => (v, Some(p.to_string())),
            None => (s, None),
        };
        let mut parts = version_part.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        Some(Self {
            major,
            minor,
            patch,
            pre: pre_part,
        })
    }

    pub fn is_newer_than(&self, current: &Self) -> bool {
        if (self.major, self.minor, self.patch) > (current.major, current.minor, current.patch) {
            return true;
        }
        if (self.major, self.minor, self.patch) == (current.major, current.minor, current.patch) {
            // A final release is newer than a prerelease with the same version numbers
            if self.pre.is_none() && current.pre.is_some() {
                return true;
            }
        }
        false
    }
}

pub fn is_newer(remote: &str, current: &str) -> bool {
    match (Version::parse(remote), Version::parse(current)) {
        (Some(rem), Some(cur)) => rem.is_newer_than(&cur),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_compares_versions() {
        assert!(is_newer("v0.4.0", "0.3.0"));
        assert!(is_newer("0.3.1", "0.3.0"));
        assert!(is_newer("v1.0.0", "0.3.0"));
        assert!(!is_newer("0.3.0", "0.3.0"));
        assert!(!is_newer("v0.3.0", "0.3.0"));
        assert!(!is_newer("v0.2.7", "0.3.0"));
        assert!(is_newer("0.3.0", "0.3.0-beta"));
    }
}
