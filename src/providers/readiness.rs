//! Setup has exactly two requirements: a channel key and the required system permissions.
#[derive(Clone, Debug, Default)]
pub struct ChannelChecks {
    pub transcription: Option<(bool, String)>,
    pub polishing: Option<(bool, String)>,
}
impl ChannelChecks {
    pub fn invalidate(&mut self) {
        *self = Self::default();
    }
}
#[derive(Clone, Copy, Debug)]
pub struct SetupReadiness {
    pub key_configured: bool,
    pub microphone_granted: bool,
    pub accessibility_granted: bool,
}
impl SetupReadiness {
    pub fn permissions_ready(self) -> bool {
        self.microphone_granted && self.accessibility_granted
    }
    pub fn ready(self) -> bool {
        self.key_configured && self.permissions_ready()
    }
    pub fn missing_count(self) -> usize {
        usize::from(!self.key_configured) + usize::from(!self.permissions_ready())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn setup_is_ready_only_after_key_and_both_permissions() {
        for key in [false, true] {
            for microphone in [false, true] {
                for accessibility in [false, true] {
                    let setup = SetupReadiness {
                        key_configured: key,
                        microphone_granted: microphone,
                        accessibility_granted: accessibility,
                    };
                    assert_eq!(setup.ready(), key && microphone && accessibility);
                    assert!(setup.missing_count() <= 2);
                }
            }
        }
    }
    #[test]
    fn key_without_permissions_still_needs_configuration() {
        let setup = SetupReadiness {
            key_configured: true,
            microphone_granted: false,
            accessibility_granted: true,
        };
        assert_eq!(setup.missing_count(), 1);
        assert!(!setup.ready());
    }
    #[test]
    fn changing_config_invalidates_both_capability_checks() {
        let mut checks = ChannelChecks {
            transcription: Some((true, "OK".into())),
            polishing: Some((true, "OK".into())),
        };
        checks.invalidate();
        assert!(checks.transcription.is_none());
        assert!(checks.polishing.is_none());
    }
}
