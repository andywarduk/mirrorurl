macro_rules! output {
    ($($arg:tt)*) => {{
        println!("{}", format!($($arg)*));
    }};
}

macro_rules! error {
    ($($arg:tt)*) => {{
        eprintln!("ERROR: {}", format!($($arg)*));
    }};
}

macro_rules! debug {
    ($state:ident, $level:expr, $($arg:tt)*) => (
        {
            if $level <= $state.as_ref().args.debug {
                eprintln!("DEBUG: {}", format!($($arg)*));
            }
        }
    )
}

pub(crate) use {debug, error, output};