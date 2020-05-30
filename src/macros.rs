#[macro_export]
macro_rules! wide_format {
    ($format:literal, $($arg:tt)*) => {{
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        let mut widened: Vec<u16> = OsStr::new(&format!($format, $($arg)*))
            .encode_wide()
            .map(|byte| if byte == 0 {
                const REPLACEMENT_CHARACTER: u16 = 0xFFFD;
                REPLACEMENT_CHARACTER
            } else {
                byte
            })
            .collect();

        widened.push(0);

        widened
    }}
}
