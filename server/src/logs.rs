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

#[derive(Error, Debug)]
pub enum DbErr {
    #[error("Tcp Error: {0}")]
    TcpErr(#[from] TcpErr),
    #[error("Store Error: {0}")]
    StoreErr(#[from] StoreErr),
    #[error("User Error: {0}")]
    UserErr(#[from] UserErr),
}

#[derive(Error, Debug)]
pub enum TcpErr {
    #[error("Error encountered when decoding incoming message")]
    TryFromIntError(#[from] std::num::TryFromIntError),
    #[error("Error when matching startup code")]
    StartupMessageError,
    #[error("Incoming message has an unrecognized type")]
    BadMessageType,
    #[error("Error processing query bytes")]
    FromUtf8Error(#[from] std::string::FromUtf8Error),
    #[error("I/O error occurred: {0}")]
    IOErr(#[from] io::Error),
    #[error("Client disconnected")]
    ClientDisconnected
}

#[derive(Error, Debug)]
pub enum StoreErr {
    #[error("Filetype does not match kawikadb filetype")]
    BadFile,
    #[error("Bincode encoding error occured: {0}")]
    EncodeErr(#[from] bincode_next::error::EncodeError),
    #[error("Bincode decoding error occured: {0}")]
    DecodeErr(#[from] bincode_next::error::DecodeError),
    #[error("Page read overflow")]
    ReadOverflow,
    #[error("I/O error occurred: {0}")]
    IOErr(#[from] io::Error),
    #[error("The Store RwLock was poisoned")]
    PoisonError,
}

#[derive(Error, Debug)]
pub enum UserErr {
    #[error("No value found at requested key")]
    NoValue,
    #[error("Value input is invalid")]
    BadVal,
    #[error("API call is malformed")]
    BadQuery,
    #[error("Put call was unsuccessful")]
    BadPut,
    #[error("No value to delete at requested key")]
    BadDel,
    #[error("There is no content in the database")]
    NoRoot,
    #[error("Key exceeds maximum length of 8 characters (sorry)")]
    LongKey,
    #[error("Value exceeds maximum length of ")]
    LongVal,
}

impl From<DbErr> for io::Error {
    fn from(err: DbErr) -> Self {
        io::Error::new(io::ErrorKind::Other, err)
    }
}
