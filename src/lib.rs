//! A safe Rust wrapper around [espeak NG](https://github.com/espeak-ng/espeak-ng)
//! via [espeakNG-sys](https://github.com/Better-Player/espeakng-sys).
//!
//! ## Safety
//! This library wraps the internal C calls in a singleton ([Speaker]) to keep the mutable global state safe.
//! In this future this may be changed to use the asynchronous features of `espeakNG` however I currently don't
//! trust it to be safe without a global lock.
//!
//! The raw bindings are re-exported via the [bindings] module however usage of this is `unsafe`
//! and all safety guarantees of the [Speaker] object are considered broken if used.
//!
//! ## Known Issues
//! - [`Speaker::synthesize`] seems to emit broken WAV audio data, no idea how to fix.
//!
//! ## Examples
//! Generating phonemes from text:
//! ```rust
//! fn main() -> Result<(), espeakng::Error> {
//!     // Get a reference to the global Speaker singleton, using default voice path and buffer length.
//!     let mut speaker = espeakng::initialise(None)?.lock();
//!
//!     // Generate the phonemes in standard mode.
//!     let phonemes = speaker.text_to_phonemes("Hello World", espeakng::PhonemeGenOptions::Standard {
//!         phoneme_mode: espeakng::PhonemeMode::default(),
//!         text_mode: espeakng::TextMode::default(),
//!     })?.unwrap();
//!     println!("Phonemes: {}", phonemes);
//!
//!     Ok(())
//! }
//! ```
#![warn(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_sign_loss, clippy::cast_possible_wrap, // Simple `as` conversions that will not fail.
    clippy::unused_self, // Speaker needs to take self to keep thread safe.
    unused_unsafe // Unsafe is unused in zstr
)]

use std::{
    ffi::CStr,
    io::{Read, Write},
    marker::PhantomData,
    os::unix::prelude::{AsRawFd, FromRawFd},
};

use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use zstr::zstr;

pub use espeakng_sys as bindings;

mod error;
mod structs;
mod utils;

pub use error::{ESpeakNgError, Error};
pub use structs::*;

use error::handle_error;

use crate::utils::StringFromCPtr;

pub type Result<T> = std::result::Result<T, Error>;
type AudioBuffer = Mutex<Vec<i16>>;

static SPEAKER: OnceCell<Mutex<Speaker>> = OnceCell::new();

/// Initialise the internal espeak-ng library. If already initialised, that [Speaker] is returned.
///
/// # Errors
/// If any initialisation steps fail, such as initialising `espeakNG` and setting the default voice.
pub fn initialise(voice_path: Option<&str>) -> Result<&'static Mutex<Speaker>> {
    SPEAKER.get_or_try_init(|| Speaker::initialise(voice_path).map(Mutex::new))
}

/// Gets the currently initialised [Speaker]. If not set, none is returned.
pub fn get() -> Option<&'static Mutex<Speaker>> {
    SPEAKER.get()
}

pub struct Speaker {
    _marker: PhantomData<std::cell::Cell<()>>,
}

impl Speaker {
    pub const DEFAULT_VOICE: &'static str = "gmw/en";

    fn initialise(voice_path: Option<&str>) -> Result<Self> {
        unsafe extern "C" fn synth_callback(
            wav: *mut i16,
            sample_count: i32,
            events: *mut bindings::espeak_EVENT,
        ) -> i32 {
            match std::panic::catch_unwind(|| {
                if wav.is_null() || sample_count == 0 {
                    return 0;
                }

                let mut new_ptr = events;

                // Loop through this C event until the terminate event, as this contains the pointer to the audio buffer
                let terminate_event = loop {
                    let event = unsafe { *new_ptr };
                    if event.type_ != bindings::espeak_EVENT_TYPE_espeakEVENT_LIST_TERMINATED {
                        break event;
                    }

                    new_ptr = unsafe { new_ptr.add(1) };
                };

                unsafe {
                    if let Some(audio_buffer) =
                        *(terminate_event.user_data as *const Option<&AudioBuffer>)
                    {
                        let wav_slice: &[i16] =
                            std::slice::from_raw_parts_mut(wav, sample_count as usize);
                        audio_buffer.lock().extend(wav_slice);
                    }
                }

                0
            }) {
                Ok(ret) => ret,
                Err(err) => {
                    eprintln!("Panic during Rust -> C -> Rust callback: {err:?}");
                    std::process::abort()
                }
            }
        }

        let voice_path = voice_path.map(utils::null_term);
        unsafe {
            bindings::espeak_SetSynthCallback(Some(synth_callback));
            bindings::espeak_ng_InitializePath(match voice_path {
                Some(path) => path.as_ptr(),
                None => std::ptr::null(),
            });

            handle_error(bindings::espeak_ng_Initialize(std::ptr::null_mut()))?;
            handle_error(bindings::espeak_ng_InitializeOutput(1, 0, std::ptr::null()))?;
        }

        let mut self_ = Self {
            _marker: PhantomData,
        };
        self_.set_voice_raw(Speaker::DEFAULT_VOICE)?;
        Ok(self_)
    }

    /// Fetch and clone the currently set voice.
    ///
    /// # Panics
    /// Panics if espeak-ng has somehow had the current voice reset, which should not happen.
    #[must_use]
    pub fn get_current_voice(&self) -> Voice {
        let voice_ptr = unsafe { bindings::espeak_GetCurrentVoice() };
        assert!(!voice_ptr.is_null(), "voice should not be null");

        Voice::from(unsafe { *voice_ptr })
    }

    /// Fetch the espeak voices currently installed.
    #[must_use]
    pub fn get_voices() -> Vec<Voice> {
        let mut array = unsafe { bindings::espeak_ListVoices(std::ptr::null_mut()) };
        let mut buf = Vec::new();

        unsafe {
            loop {
                let next = array.read();

                if next.is_null() {
                    break buf;
                }

                buf.push(Voice::from(*next));
                array = array.add(1);
            }
        }
    }

    /// Set the voice for future espeak calls.
    ///
    /// # Errors
    /// See [`Speaker::set_voice_raw`]
    pub fn set_voice(&mut self, voice: &Voice) -> Result<()> {
        self.set_voice_raw(&voice.filename)
    }

    /// Set the voice for future espeak calls based on the filename
    ///
    /// # Errors
    /// [`ESpeakNgError::VoiceNotFound`]
    pub fn set_voice_raw(&mut self, filename: &str) -> Result<()> {
        let mbrola_voice = filename.starts_with("mb/");

        // We have to do our own VoiceNotFound check as espeakNG seems to internally fail at that.
        if mbrola_voice {
            let mut voice_path = Self::info().1;
            voice_path.push(format!("voices/{filename}"));
            if !voice_path.exists() {
                return Err(Error::ESpeakNg(ESpeakNgError::VoiceNotFound));
            }
        }

        let name_null_term = utils::null_term(filename);
        if mbrola_voice {
            // Now we are sure the voice is set, we can loop until espeakNG shuts up.
            while let Err(err) =
                handle_error(unsafe { bindings::espeak_ng_SetVoiceByName(name_null_term.as_ptr()) })
            {
                if let Error::ESpeakNg(espeak_err) = err {
                    if espeak_err == ESpeakNgError::VoiceNotFound {
                        continue;
                    }
                }

                return Err(err);
            }
        } else {
            handle_error(unsafe { bindings::espeak_ng_SetVoiceByName(name_null_term.as_ptr()) })?;
        }

        Ok(())
    }

    /// Get the value of either the currently set or default value of a settings parameter.
    pub fn get_parameter(&mut self, param: Parameter, default: bool) -> i32 {
        unsafe { bindings::espeak_GetParameter(param as u32, i32::from(!default)) }
    }

    /// Set a settings parameter for future espeak calls.
    ///
    /// # Errors
    /// - If a value out of range of the parameter is passed.
    /// - If the internal C call fails.
    pub fn set_parameter(
        &mut self,
        param: Parameter,
        new_value: i32,
        relative: bool,
    ) -> Result<()> {
        handle_error(unsafe {
            bindings::espeak_ng_SetParameter(param as u32, new_value, i32::from(relative))
        })
    }

    /// Get the version string and voice path of the internal C library.
    #[must_use]
    pub fn info() -> (String, std::path::PathBuf) {
        let mut c_voice_path: *const libc::c_char = std::ptr::null();

        unsafe {
            let version_string = bindings::espeak_Info(std::ptr::addr_of_mut!(c_voice_path));

            (
                String::from_cptr(version_string),
                std::path::PathBuf::from(String::from_cptr(c_voice_path)),
            )
        }
    }

    fn _synthesize(&mut self, text: &str, user_data: Option<&AudioBuffer>) -> Result<()> {
        let text_nul_term = utils::null_term(text);

        handle_error(unsafe {
            bindings::espeak_ng_Synthesize(
                text_nul_term.as_ptr().cast::<std::ffi::c_void>(),
                text_nul_term.len(),
                0,
                bindings::espeak_POSITION_TYPE_POS_CHARACTER,
                0,
                bindings::espeakCHARS_UTF8,
                std::ptr::null_mut(),
                (&user_data.map(|ud| ud as *const _) as *const _) as *mut std::ffi::c_void,
            )
        })?;

        // Wait until TTS has finished being generated, could be made concurrent but global state....
        handle_error(unsafe { bindings::espeak_ng_Synchronize() })?;

        Ok(())
    }

    /// Processes the given text into WAV audio data.
    ///
    /// # Errors
    /// If the internal espeak synthesis fails, see [`ESpeakNgError`]
    pub fn synthesize(&mut self, text: &str) -> Result<Vec<i16>> {
        let audio_buffer: AudioBuffer = Mutex::new(Vec::<i16>::new());
        self._synthesize(text, Some(&audio_buffer))?;
        Ok(audio_buffer.into_inner())
    }

    /// Processes the given text into WAV audio data and writes it to a given file.
    ///
    /// This handles the `Vec<i16>` to `Vec<u8>` conversion internally.
    ///
    /// # Errors
    /// See [`Speaker::synthesize`] + the file writing failed.
    pub fn synthesize_to_file(&mut self, file: &mut std::fs::File, text: &str) -> Result<()> {
        let audio_data_i16 = self.synthesize(text)?;

        let audio_data: Vec<u8> = audio_data_i16
            .into_iter()
            .flat_map(i16::to_le_bytes)
            .collect();
        file.write_all(&audio_data)?;
        Ok(())
    }

    /// Processes the given text into phonemes, depending on which [`PhonemeGenOptions`] are passed.
    ///
    /// This will only return [None] if [`PhonemeGenOptions::MbrolaFile`] is passed.
    ///
    /// # Errors
    /// If [`PhonemeGenOptions::Mbrola`] or [`PhonemeGenOptions::MbrolaFile`] is passed, internal C calls may fail.
    pub fn text_to_phonemes(
        &mut self,
        text: &str,
        option: PhonemeGenOptions,
    ) -> Result<Option<String>> {
        let file = match option {
            PhonemeGenOptions::MbrolaFile(file) => Some(file),
            _ => None,
        };

        match option {
            PhonemeGenOptions::Standard {
                text_mode,
                phoneme_mode,
            } => Ok(Some(self.text_to_phonemes_standard(
                text,
                text_mode,
                phoneme_mode,
            ))),
            PhonemeGenOptions::Mbrola | PhonemeGenOptions::MbrolaFile(_) => {
                self.text_to_phonemes_mbrola(text, file)
            }
        }
    }

    fn text_to_phonemes_standard(
        &mut self,
        text: &str,
        text_mode: TextMode,
        phoneme_mode: PhonemeMode,
    ) -> String {
        let text_nul_term = utils::null_term(text);

        let output = unsafe {
            CStr::from_ptr(bindings::espeak_TextToPhonemes(
                &mut text_nul_term.as_ptr().cast() as *mut *const std::ffi::c_void,
                text_mode as i32,
                phoneme_mode.bits() as i32,
            ))
        };

        output.to_string_lossy().to_string()
    }

    fn text_to_phonemes_mbrola(
        &mut self,
        text: &str,
        file: Option<&dyn AsRawFd>,
    ) -> Result<Option<String>> {
        if !self.get_current_voice().filename.starts_with("mb/") {
            return Err(Error::MbrolaWithoutMbrolaVoice);
        };

        // If file is not passed, generate a fake FD to store the data in
        let raw_file_fd = match file {
            Some(file) => file.as_raw_fd(),
            None => unsafe { libc::memfd_create(zstr!("").as_ptr(), 0) },
        };

        // Generate fake C File from this FD
        let raw_file = unsafe {
            let raw_file_ptr = bindings::fdopen(raw_file_fd, zstr!("w+").as_ptr());
            std::ptr::NonNull::new(raw_file_ptr)
                .ok_or_else(|| Error::OtherC(Some(errno::errno())))?
        };

        // Set the phoneme output to the stream
        unsafe {
            bindings::espeak_SetPhonemeTrace(
                bindings::espeakPHONEMES_MBROLA as i32,
                raw_file.as_ptr(),
            );
        }

        // Generate TTS, this will populate the phoneme trace
        let result = self._synthesize(text, None);

        // Reset the phoneme trace back to stdout, to avoid side effects
        unsafe { bindings::espeak_SetPhonemeTrace(0, std::ptr::null_mut()) };

        if file.is_none() {
            let mut file = unsafe {
                // Seek to the start of the fake_file, now it has been written to
                bindings::fseek(raw_file.as_ptr(), 0, 0);

                // Transfer FD ownership from C to Rust
                let dup_fd = libc::dup(raw_file_fd);
                bindings::fclose(raw_file.as_ptr());

                // SAFETY: ^ must have just occured
                std::fs::File::from_raw_fd(dup_fd)
            };

            // Now handle possible errors, as we can return without leak.
            result?;

            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            Ok(Some(String::from_utf8(buf)?))
        } else {
            // Data has been written to the file passed in, close the C version of the file.
            unsafe { bindings::fclose(raw_file.as_ptr()) };
            // Now handle possible errors, and if successful get rid of any return value.
            result.map(|_| None)
        }
    }
}

impl Drop for Speaker {
    fn drop(&mut self) {
        unsafe { bindings::espeak_ng_Terminate() };
    }
}
