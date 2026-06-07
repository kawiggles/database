use std::fmt;
use std::fs::File;
use log::LevelFilter;
use simplelog::{WriteLogger, Config};

pub fn init_logs() {
    WriteLogger::init(
        LevelFilter::Info,
        Config::default(),
        File::create("db.log").unwrap()
    ).unwrap();
}

#[derive(Debug)]
pub enum DbError {
    NoValue,
    BadVal,
    BadCall,
    BadPut,
    BadDel,
}

impl From<DbError> for fmt::Error {
    fn from(_error: DbError) -> Self {
        fmt::Error
    }
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", String::from(match self {
            DbError::NoValue => "No value found at requested key",
            DbError::BadVal => "Value input is invalid",
            DbError::BadCall => "API call is malformed",
            DbError::BadPut => "Put call was unsuccessful",
            DbError::BadDel => "No value to delete at requested key",
        }))
    }
}
