//! macOS permission queries. Reading status never requests access or starts audio capture.
use objc2::{class, msg_send, runtime::Bool};
use objc2_foundation::NSString;
#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {
    static AVMediaTypeAudio: *const NSString;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MicrophoneAccess {
    NotDetermined,
    Restricted,
    Denied,
    Authorized,
    Unknown,
}
impl MicrophoneAccess {
    fn from_status(status: isize) -> Self {
        match status {
            0 => Self::NotDetermined,
            1 => Self::Restricted,
            2 => Self::Denied,
            3 => Self::Authorized,
            _ => Self::Unknown,
        }
    }
    pub fn granted(self) -> bool {
        self == Self::Authorized
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Authorized => "已授权",
            Self::NotDetermined => "待授权",
            Self::Denied => "未授权",
            Self::Restricted => "受系统限制",
            Self::Unknown => "待检查",
        }
    }
}
pub fn microphone_status() -> MicrophoneAccess {
    // AVMediaTypeAudio is a process-lifetime Foundation string. Apple's API returns NSInteger.
    let status: isize = unsafe {
        msg_send![class!(AVCaptureDevice), authorizationStatusForMediaType: AVMediaTypeAudio]
    };
    MicrophoneAccess::from_status(status)
}
pub fn request_microphone() {
    if microphone_status() == MicrophoneAccess::NotDetermined {
        // AVFoundation copies the completion block before returning. The callback never touches GPUI.
        let completion = block2::RcBlock::new(|_granted: Bool| {});
        unsafe {
            let _: () = msg_send![class!(AVCaptureDevice), requestAccessForMediaType: AVMediaTypeAudio, completionHandler: &*completion];
        }
    } else {
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
            .spawn();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_authorized_microphone_status_is_ready() {
        for status in [0, 1, 2, 4, -1] {
            assert!(!MicrophoneAccess::from_status(status).granted());
        }
        assert!(MicrophoneAccess::from_status(3).granted());
    }
}
