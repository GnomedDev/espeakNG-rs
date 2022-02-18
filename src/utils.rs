use std::ffi::CStr;

pub(crate) fn null_term(s: &str) -> Vec<i8> {
    let mut nul_term_s: Vec<i8> = Vec::with_capacity(s.len());
    nul_term_s.extend(s.as_bytes().iter().map(|i| *i as i8));
    nul_term_s.push(0);
    nul_term_s 
}

pub(crate) unsafe fn parse_lang_array(ptr: *const i8) -> Vec<crate::Language> {
    let mut languages = Vec::new();
    let mut ptr = ptr;

    loop {
        // SAFETY: It probably isn't
        let (name, priority) = unsafe {
            if *ptr == 0 {break}

            // First byte is priority
            let priority = ptr.read();
            ptr = ptr.add(1);
            
            // Then we have language, as a null term string
            let namelen = libc::strlen(ptr);
            let name = std::slice::from_raw_parts(ptr.cast::<u8>(), namelen);

            // Move the pointer past the name, plus 1 for the next iter
            ptr = ptr.add(namelen + 1);
            (name, priority)
        };

        languages.push(crate::Language {
            name: String::from_utf8(Vec::from(name)).unwrap(),
            priority
        });
    }
    languages
}

pub(crate) trait StringFromCPtr {
    unsafe fn from_cptr(ptr: *const i8) -> Self;
}

impl StringFromCPtr for String {
    unsafe fn from_cptr(ptr: *const i8) -> Self {
        unsafe {CStr::from_ptr(ptr)}.to_string_lossy().into_owned()
    }
}
