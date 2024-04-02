/// An error from this library.
#[derive(Debug)]
pub enum Error {
    /// Occured in an espeakng C function.
    ESpeakNg(ESpeakNgError),
    /// [crate::initialise] was called when already initialized.
    AlreadyInit,
    /// [crate::Speaker::text_to_phonemes] was called without an mbrola voice selected.
    MbrolaWithoutMbrolaVoice,
    /// Occured non-espeakng C function, errno is contained if populated.
    OtherC(Option<errno::Errno>),
    /// Occured in an unknown Rust location, usually a library bug.
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl std::error::Error for Error {}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&match self {
            Self::MbrolaWithoutMbrolaVoice => {
                String::from("eSpeak cannot generate mbrola phonemes without an mbrola voice set!")
            }
            Self::ESpeakNg(err) => {
                format!("Failed to execute an internal espeakNG function: {err:?}")
            }
            Self::AlreadyInit => {
                String::from("espeakng::initialise was called after already having been called!")
            }
            Self::OtherC(err) => format!("Failed to execute an internal C function: {err:?}"),
            Self::Other(err) => format!("An internal error occurred: {err:?}"),
        })
    }
}

impl From<ESpeakNgError> for Error {
    fn from(err: ESpeakNgError) -> Self {
        Self::ESpeakNg(err)
    }
}

macro_rules! generate_unknown_err {
    ($cause:ty) => {
        impl From<$cause> for Error {
            fn from(err: $cause) -> Self {
                Self::Other(Box::new(err))
            }
        }
    };
}

generate_unknown_err!(std::io::Error);
generate_unknown_err!(std::string::FromUtf8Error);

/// An error from the `espeakNG` C library.
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum_macros::FromRepr)]
#[allow(clippy::module_name_repetitions)]
#[repr(u32)]
#[rustfmt::skip]
pub enum ESpeakNgError {
    CompileError              = 0x1000_01FF,
    VersionMismatch           = 0x1000_02FF,
    FifoBufferFull            = 0x1000_03FF,
    NotInitialized            = 0x1000_04FF,
    AudioError                = 0x1000_05FF,
    VoiceNotFound             = 0x1000_06FF,
    MbrolaNotFound            = 0x1000_07FF,
    MbrolaVoiceNotFound       = 0x1000_08FF,
    EventBufferFull           = 0x1000_09FF,
    NotSupported              = 0x1000_0AFF,
    UnsupportedPhonemeFormat  = 0x1000_0BFF,
    NoSpectFrames             = 0x1000_0CFF,
    EmptyPhonemeManifest      = 0x1000_0DFF,
    SpeechStopped             = 0x1000_0EFF,
    UnknownPhonemeFeature     = 0x1000_0FFF,
    UnknownTextEncoding       = 0x1000_10FF,
}

impl std::error::Error for ESpeakNgError {}
impl std::fmt::Display for ESpeakNgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const BUFFER_LEN: usize = 512;
        let mut buffer: [libc::c_char; BUFFER_LEN] = [0; BUFFER_LEN];

        // SAFETY: The size of the buffer is from internal to espeakNG
        // if this isn't long enough, internal functions to espeakNG break.
        let status_code_message = unsafe {
            crate::bindings::espeak_ng_GetStatusCodeMessage(
                *self as u32,
                buffer.as_mut_ptr(),
                BUFFER_LEN,
            );

            std::ffi::CStr::from_ptr(buffer.as_ptr())
        };

        f.write_str(&status_code_message.to_string_lossy())
    }
}

pub(crate) fn handle_error(ret_code: u32) -> Result<(), Error> {
    if ret_code == 0 {
        Ok(())
    } else {
        Err(match ESpeakNgError::from_repr(ret_code) {
            Some(err) => Error::ESpeakNg(err),
            None => Error::OtherC(Some(errno::errno())),
        })
    }
}
