//! `arboard` under another name (see `pdfcraft-rfd`). arboard has no Android backend, so there
//! every clipboard call returns an error the app already shows as "couldn't copy".

#[cfg(not(target_os = "android"))]
pub use arboard::*;

#[cfg(target_os = "android")]
mod android {
    use std::borrow::Cow;
    use std::fmt;
    use std::path::Path;

    #[derive(Debug)]
    pub struct Error;

    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("the clipboard isn't available on Android yet")
        }
    }

    impl std::error::Error for Error {}

    pub struct ImageData<'a> {
        pub width: usize,
        pub height: usize,
        pub bytes: Cow<'a, [u8]>,
    }

    pub struct Clipboard;

    pub struct Set;

    impl Clipboard {
        pub fn new() -> Result<Clipboard, Error> {
            Err(Error)
        }

        pub fn get_image(&mut self) -> Result<ImageData<'static>, Error> {
            Err(Error)
        }

        pub fn get_text(&mut self) -> Result<String, Error> {
            Err(Error)
        }

        pub fn set_image(&mut self, _image: ImageData<'_>) -> Result<(), Error> {
            Err(Error)
        }

        pub fn set(&mut self) -> Set {
            Set
        }
    }

    impl Set {
        pub fn file_list(self, _paths: &[impl AsRef<Path>]) -> Result<(), Error> {
            Err(Error)
        }
    }
}

#[cfg(target_os = "android")]
pub use android::*;
