//! Tests for espeakng::Speaker::text_to_phonemes
mod base;
use base::init;
use espeakng::{PhonemeMode, TextMode};

#[test]
fn espeak() -> Result<(), espeakng::Error> {
    assert_eq!(
        init()
            .text_to_phonemes(
                "Hello world",
                espeakng::PhonemeGenOptions::Standard {
                    text_mode: TextMode::Utf8,
                    phoneme_mode: PhonemeMode::empty()
                }
            )?
            .unwrap(),
        include_str!("../test_data/hello_world.pho")
    );

    Ok(())
}

#[test]
fn mbrola() -> Result<(), espeakng::Error> {
    let mut speaker = init();
    speaker.set_voice_raw("mb/mb-en1")?;

    assert_eq!(
        speaker
            .text_to_phonemes("Hello world", espeakng::PhonemeGenOptions::Mbrola)?
            .unwrap(),
        include_str!("../test_data/hello_world_mbrola.pho")
    );

    Ok(())
}
