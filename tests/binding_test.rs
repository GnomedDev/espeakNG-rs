//! Very basic test for the bindings, should not be used as an example!

use std::ffi::{c_void, CStr};

use zstr::zstr;

use espeakng::bindings;

#[test]
fn binding_check() -> Result<(), Box<dyn std::error::Error>> {
    let mode = zstr!("r+");
    let en = zstr!("mb-en1");
    let hello_world = zstr!("Hello world");

    unsafe {
        bindings::espeak_Initialize(
            bindings::espeak_AUDIO_OUTPUT_AUDIO_OUTPUT_RETRIEVAL,
            0,
            std::ptr::null(),
            0,
        );
        loop {
            let r = bindings::espeak_SetVoiceByName(en.as_ptr());
            if r == 0 {
                break;
            }
        }

        let mut buf = vec![0; 205];
        let fake_file = libc::fmemopen(buf.as_mut_ptr() as *mut libc::c_void, 200, mode.as_ptr());

        bindings::espeak_SetPhonemeTrace(
            bindings::espeakPHONEMES_MBROLA as i32,
            std::mem::transmute(fake_file),
        );

        bindings::espeak_ng_Synthesize(
            hello_world.as_ptr() as *const c_void,
            hello_world.to_bytes_with_nul().len(),
            0,
            bindings::espeak_POSITION_TYPE_POS_CHARACTER,
            0,
            bindings::espeakCHARS_AUTO,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );

        bindings::espeak_Synchronize();
        bindings::espeak_Terminate();

        libc::fseek(fake_file, 0, libc::SEEK_END);
        buf.set_len(libc::ftell(fake_file) as usize);
        libc::fseek(fake_file, 0, libc::SEEK_SET);

        assert_eq!(
            CStr::from_ptr(buf.as_ptr()).to_str()?,
            include_str!("../test_data/hello_world_mbrola.pho")
        );
    };

    Ok(())
}
