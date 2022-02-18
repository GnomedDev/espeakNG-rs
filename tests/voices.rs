fn init<'a>() -> parking_lot::MutexGuard<'a, espeakng::Speaker> {
    espeakng::initialise(None).unwrap().lock()
}

#[test]
fn get_voice() -> espeakng::Result<()> {
    assert_eq!(
        init().get_current_voice().filename,
        espeakng::Speaker::DEFAULT_VOICE
    );

    Ok(())
}
