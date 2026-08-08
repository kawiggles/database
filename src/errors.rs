use thiserror::Error;
use std::{
    io,
    fs::File,
};
use bincode_next;
use log::LevelFilter;
use simplelog::{WriteLogger, Config};

use crate::tcp::response::Response;

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

pub type DbResult<T> = std::result::Result<T, DbErr>;

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

pub type TcpResult<T> = std::result::Result<T, TcpErr>;

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

pub type StoreResult<T> = std::result::Result<T, StoreErr>;

#[derive(Error, Debug)]
pub enum UserErr {
    #[error("No value found at requested key")]
    NoValue,
    #[error("Value input is invalid")]
    BadVal,
    #[error("API call is malformed")]
    BadQuery(#[from] QueryErr),
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

pub type UserResult<T> = std::result::Result<T, UserErr>;

#[derive(Error, Debug)]
pub enum QueryErr {
    #[error("Non-Ascii byte found at position {}: {}", pos, byte)]
    NonAsciiChar {
        pos: usize,
        byte: u8,
    },
    #[error("String literal at {0} has no close")]
    StrLiteralNoClose(usize),
    #[error("Non-utf-8 char found in literal")]
    LiteralNonUtf8(#[from] std::str::Utf8Error),
}

pub type QueryResult<T> = std::result::Result<T, QueryErr>;

impl From<DbErr> for io::Error {
    fn from(err: DbErr) -> Self {
        io::Error::new(io::ErrorKind::Other, err)
    }
}

pub trait Err {
    fn gen_error_response(&self) -> Response;
}

impl Err for DbErr {
    fn gen_error_response(&self) -> Response {
        match self {
            DbErr::TcpErr(err) => err.gen_error_response(),
            DbErr::UserErr(err) => err.gen_error_response(),
            DbErr::StoreErr(err) => err.gen_error_response(),
        }
    }
}

impl Err for TcpErr {
    fn gen_error_response(&self) -> Response {
        todo!();
    }
}

impl Err for UserErr {
    fn gen_error_response(&self) -> Response {
        todo!();        
    }
}

impl Err for StoreErr {
    fn gen_error_response(&self) -> Response {
        todo!();        
    }
}
