use crate::logs::{TcpErr, DbErr};
use crate::query::Query;
use log::{info, warn};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug)]
pub struct StartupMessage(usize); // where value is version

#[derive(Debug)]
pub enum Request {
    Query(Query),
    Termination
}

pub async fn decode_startup<T>(stream: &mut T) -> Result<StartupMessage, TcpErr> 
where T: AsyncReadExt + AsyncWriteExt + Unpin {

    info!(" - Decoding client startup message");
    let len = read_i32(stream).await?;
    let code = read_i32(stream).await?;
    info!("   - Code is {}", code);

    let code = if code == 80877103 {
        info!("   - Negotiating SSL with client");
        stream.write_all(&[b'N']).await?;
        let _ = read_i32(stream).await?;
        read_i32(stream).await?
    } else {
        code
    };

    info!("  - Message code is {}", code);

    match code {
        // SSL negotiation, WAYYYY down the line
        196608 => {
            info!("   - Postgresql client is using protocol version 3.0");
            let mut params = vec![0u8; (len - 8) as usize];
            stream.read_exact(&mut params).await?;

            Ok(StartupMessage(code as usize))
        },
        _ => {
            warn!("   - Error decoding startup message");
            Err(TcpErr::StartupMessageError)
        },
    }
}

pub async fn decode_request<T>(stream: &mut T) -> Result<Request, DbErr> 
where T: AsyncReadExt + Unpin{
    info!(" - Decoding client request");
    let message_type = read_char(stream).await?;
    info!("   - Message type is {}", message_type);

    let len = read_i32(stream).await?;
    let contents = read_contents(stream, len).await?;
    info!("   - Message contents are {}", contents);

    match message_type {
        // TODO: replace with updated querying system
        'Q' => Ok(Request::Query(Query::parse(&contents)?)),
        'X' => Ok(Request::Termination),
        _ => {
            warn!("   - Type not recognized {}", message_type);
            Err(TcpErr::BadMessageType)?
        }
    }
}

async fn _read_i16<T>(stream: &mut T) -> Result<i16, TcpErr>
where T: AsyncReadExt + Unpin {
    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).await?;
    Ok(i16::from_be_bytes(buf))
}

async fn read_i32<T>(stream: &mut T) -> Result<i32, TcpErr>
where T: AsyncReadExt + Unpin {
    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf).await?;
    Ok(i32::from_be_bytes(buf))
}

// TODO: validate that len is correct
// The len being passed is the len value from the message
async fn read_contents<T>(stream: &mut T, len: i32) -> Result<String, TcpErr> 
where T: AsyncReadExt + Unpin {
    let mut buf = vec![0; (len - 4) as usize];
    stream.read_exact(&mut buf).await?;
    let contents = String::from_utf8(buf)?
        .trim_matches('\0')
        .to_string();
    Ok(contents)
}

async fn read_char<T>(stream: &mut T) -> Result<char, TcpErr> 
where T: AsyncReadExt + Unpin {
    let mut buf = [0u8; 1];
    stream.read_exact(&mut buf).await?;
    Ok(buf[0] as char)
}
