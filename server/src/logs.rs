use thiserror::Error;
use std::io;
use std::fs::File;
use bincode_next;
use log::LevelFilter;
use simplelog::{WriteLogger, Config};

pub fn init_logs() {
    WriteLogger::init(
        LevelFilter::Info,
        Config::default(),
        File::create("db.log").unwrap()
    ).unwrap();
}

// TODO: add values to some of these errors for better info
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
    IOErr(#[from] io::Error),
    #[error("Bincode encoding error occured: {0}")]
    EncodeErr(#[from] bincode_next::error::EncodeError),
    #[error("Bincode decoding error occured: {0}")]
    DecodeErr(#[from] bincode_next::error::DecodeError),
    #[error("Filetype does not match kawikadb filetype")]
    BadFile,
    #[error("Key exceeds maximum length of 8 characters (sorry)")]
    LongKey,
    // need to find maximum length
    #[error("Value exceeds maximum length of ")]
    LongVal,
    #[error("Page read overflow")]
    ReadOverflow,
    #[error("The database b+ tree has no root")]
    NoRoot,
    #[error("Error encountered when validating b+ tree, fix that shit")]
    TreeErr,
}

impl From<DbError> for io::Error {
    fn from(err: DbError) -> Self {
        io::Error::new(io::ErrorKind::Other, err)
    }
}

pub type Result<T> = std::result::Result<T, DbError>;
