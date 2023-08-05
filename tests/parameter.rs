mod base;
use base::init;

#[test]
fn set() {
    let mut speaker = init();
    speaker
        .set_parameter(espeakng::Parameter::Volume, 1, true)
        .unwrap();

    assert_eq!(
        speaker.get_parameter(espeakng::Parameter::Volume, true) + 1,
        speaker.get_parameter(espeakng::Parameter::Volume, false)
    );
}
