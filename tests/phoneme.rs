//! Tests for espeakng::Speaker::text_to_phonemes

fn init<'a>() -> parking_lot::MutexGuard<'a, espeakng::Speaker> {
    espeakng::initialise(None).unwrap().lock()
}

#[test]
fn espeak() -> Result<(), espeakng::Error> {
    assert_eq!(
        init().text_to_phonemes("Hello world", espeakng::PhonemeGenOptions::Standard)?.unwrap(),
        include_str!("../test_data/hello_world.pho")
    );

    Ok(())
}

#[test]
fn mbrola() -> Result<(), espeakng::Error> {
    let mut speaker = init();
    while let Err(err) = speaker.set_voice_raw("mb/mb-en1") {
        if let espeakng::Error::ESpeakNg(espeak_err) = err {
            if espeak_err == espeakng::ESpeakNgError::VoiceNotFound {
                continue
            } else {
                return Err(err)
            }
        }
    }

    assert_eq!(
        speaker.text_to_phonemes("Hello world", espeakng::PhonemeGenOptions::Mbrola)?.unwrap(),
        include_str!("../test_data/hello_world_mbrola.pho")
    );

    Ok(())
}
