use thiserror::Error;
use std::io;
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

#[derive(Error, Debug)]
pub enum DbError {
    #[error("No value found at requested key")]
    NoValue,
    #[error("Value input is invalid")]
    BadVal,
    #[error("API call is malformed")]
    BadCall,
    #[error("Put call was unsuccessful")]
    BadPut,
    #[error("No value to delete at requested key")]
    BadDel,
    #[error("I/O error occurred: {0}")]
    FileErr(#[from] io::Error),
}

impl From<DbError> for io::Error {
    fn from(err: DbError) -> Self {
        io::Error::new(io::ErrorKind::Other, err)
    }
}

