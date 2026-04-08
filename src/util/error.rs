use std::error::Error;
use std::fmt::Display;

#[derive(Debug)]
pub struct CheeseError {
    msg: String,
}

impl CheeseError {
    pub fn new<S: Into<String>>(msg: S) -> Self {
        Self { msg: msg.into() }
    }
}

// std::io::Error will automatically convert into CheeseError with the ? operator
// if more details is desireable, convert into a CheeseError manually with cheese_error!

#[macro_export]
macro_rules! cheese_error {
    ($($arg:tt)*) => {{

        CheeseError::new(format!("[{} line {}] {}", file!(), line!(),
            format!($($arg)*)
        ))
    }};
}

impl From<std::io::Error> for CheeseError {
    fn from(err: std::io::Error) -> Self {
        CheeseError::new(format!("I/O error: {err}"))
    }
}

impl From<String> for CheeseError {
    fn from(value: String) -> Self {
        CheeseError::new(value)
    }
}

impl Display for CheeseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", self.msg)
    }
}

impl Error for CheeseError {}
