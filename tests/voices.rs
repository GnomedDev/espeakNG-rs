mod base;
use base::init;

#[test]
fn get_voice() -> espeakng::Result<()> {
    assert_eq!(
        init().get_current_voice().filename,
        espeakng::Speaker::DEFAULT_VOICE
    );

    Ok(())
}
