use std::os::unix::prelude::AsRawFd;

use crate::utils::StringFromCPtr;
use crate::{bindings, utils};

#[derive(Clone, Copy)]
pub enum PhonemeGenOptions<'a> {
    /// Generate phonemes using the standard espeak style
    Standard,
    /// Generate phonemes using the mbrola style
    Mbrola,
    /// Generate phonemes using the mbrola style and write them in a file
    MbrolaFile(&'a dyn AsRawFd)
}


#[derive(Debug, PartialEq, Eq, Copy, Clone, strum_macros::FromRepr)]
#[repr(u8)]
pub enum Gender {
    Male = 1,
    Female = 2
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Language {
    pub name: String,
    pub priority: i8,
}

#[derive(Debug, PartialEq, Eq, Clone)]
#[non_exhaustive] // Keep Voice private constructable to keep set_voice safe.
pub struct Voice {
    pub name: String,
    pub filename: String,
    pub languages: Vec<Language>,
    pub gender: Option<Gender>,
    pub age: u8,
}

impl From<bindings::espeak_VOICE> for Voice {
    fn from(voice: bindings::espeak_VOICE) -> Self {
        unsafe {
            Self {
                age: voice.age,
                name: String::from_cptr(voice.name),
                filename: String::from_cptr(voice.identifier),
                gender: Gender::from_repr(voice.gender),
                languages: utils::parse_lang_array(voice.languages)
            }
        }
    }
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Parameter {
    /// Words per minute. Values must be between 80-450 inclusive.
    Rate = 1,
    /// Volume of speech. Values should be 0-100 as greater values may produce amplitude compression or distortion.
    Volume = 2,
    /// Base pitch, default 50. Values must be between 0-100 inclusive.
    Pitch = 3,
    /// The pitch range, where 0 is monotone and 50 is normal. Values must be between 0-100 inclusive.
    Range = 4,
    /// The punctuation characters to speak. Value must be [PunctationType].
    Punctuation = 5,
    /// How to pronounce capital letters.
    /// - 0 = none
    /// - 1 = sound icon
    /// - 2 = spelling
    /// - 3 or higher, by raising pitch. The value is the amount of Hz by which the pitch of each capitalised word is raised.
    Capitals = 6,
    /// The units of how long to pause between words. At default speed, this is units of of 10mS.  
    Wordgap = 7,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum PunctationType {
    None = 0,
    All = 1,
    Some = 2,
}
