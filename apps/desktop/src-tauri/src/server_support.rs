use serde_json::Value;
use slate_contracts::ServerStatusSummary;
use std::net::Ipv6Addr;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const DEFAULT_MINECRAFT_PORT: u16 = 25_565;
const MAX_STATUS_RESPONSE: usize = 2 * 1024 * 1024;
const STATUS_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, thiserror::Error)]
pub(super) enum ServerAddressError {
    #[error("server address is empty")]
    Empty,
    #[error("server address is not valid")]
    Invalid,
}

#[derive(Debug, thiserror::Error)]
enum ServerPingError {
    #[error("server connection failed")]
    Io(#[from] std::io::Error),
    #[error("server status response is invalid")]
    InvalidResponse,
    #[error("server status response is not JSON")]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ServerEndpoint {
    host: String,
    port: u16,
    display: String,
}

pub(super) fn normalize_server_address(address: &str) -> Result<String, ServerAddressError> {
    Ok(parse_server_address(address)?.display)
}

pub(super) async fn ping_minecraft_server(
    address: &str,
) -> Result<ServerStatusSummary, ServerAddressError> {
    let endpoint = parse_server_address(address)?;
    let display = endpoint.display.clone();
    let status = tokio::time::timeout(STATUS_TIMEOUT, ping_endpoint(endpoint)).await;
    Ok(match status {
        Ok(Ok(summary)) => summary,
        Ok(Err(_)) | Err(_) => ServerStatusSummary {
            address: display,
            online: false,
            latency_ms: None,
            version_name: None,
            protocol: None,
            players_online: None,
            players_max: None,
            description: None,
        },
    })
}

async fn ping_endpoint(endpoint: ServerEndpoint) -> Result<ServerStatusSummary, ServerPingError> {
    let started = Instant::now();
    let mut stream = TcpStream::connect((endpoint.host.as_str(), endpoint.port)).await?;
    stream.set_nodelay(true)?;

    let mut handshake = Vec::new();
    write_varint(0, &mut handshake);
    write_varint(-1, &mut handshake);
    write_string(&endpoint.host, &mut handshake)?;
    handshake.extend_from_slice(&endpoint.port.to_be_bytes());
    write_varint(1, &mut handshake);

    let mut framed_handshake = Vec::new();
    write_length(handshake.len(), &mut framed_handshake)?;
    framed_handshake.extend_from_slice(&handshake);
    stream.write_all(&framed_handshake).await?;
    stream.write_all(&[1, 0]).await?;

    let packet_length = read_varint(&mut stream).await?;
    let packet_length =
        usize::try_from(packet_length).map_err(|_| ServerPingError::InvalidResponse)?;
    if packet_length == 0 || packet_length > MAX_STATUS_RESPONSE {
        return Err(ServerPingError::InvalidResponse);
    }
    let mut packet = vec![0_u8; packet_length];
    stream.read_exact(&mut packet).await?;
    let mut cursor = packet.as_slice();
    if read_varint_from_slice(&mut cursor)? != 0 {
        return Err(ServerPingError::InvalidResponse);
    }
    let json_length = read_varint_from_slice(&mut cursor)?;
    let json_length = usize::try_from(json_length).map_err(|_| ServerPingError::InvalidResponse)?;
    if json_length > cursor.len() {
        return Err(ServerPingError::InvalidResponse);
    }
    let value: Value = serde_json::from_slice(&cursor[..json_length])?;
    let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

    Ok(ServerStatusSummary {
        address: endpoint.display,
        online: true,
        latency_ms: Some(latency_ms),
        version_name: value
            .pointer("/version/name")
            .and_then(Value::as_str)
            .map(str::to_owned),
        protocol: value
            .pointer("/version/protocol")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok()),
        players_online: value
            .pointer("/players/online")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok()),
        players_max: value
            .pointer("/players/max")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok()),
        description: description_text(value.get("description")),
    })
}

fn parse_server_address(address: &str) -> Result<ServerEndpoint, ServerAddressError> {
    let address = address.trim();
    if address.is_empty() {
        return Err(ServerAddressError::Empty);
    }
    if address.len() > 255
        || address.chars().any(char::is_whitespace)
        || address.contains(['/', '\\', '?', '#', '@'])
    {
        return Err(ServerAddressError::Invalid);
    }

    let (host, port) = if let Some(bracketed) = address.strip_prefix('[') {
        let closing = bracketed.find(']').ok_or(ServerAddressError::Invalid)?;
        let host = &bracketed[..closing];
        host.parse::<Ipv6Addr>()
            .map_err(|_| ServerAddressError::Invalid)?;
        let suffix = &bracketed[closing + 1..];
        let port = if suffix.is_empty() {
            DEFAULT_MINECRAFT_PORT
        } else {
            parse_port(
                suffix
                    .strip_prefix(':')
                    .ok_or(ServerAddressError::Invalid)?,
            )?
        };
        (host.to_ascii_lowercase(), port)
    } else if address.matches(':').count() > 1 {
        address
            .parse::<Ipv6Addr>()
            .map_err(|_| ServerAddressError::Invalid)?;
        (address.to_ascii_lowercase(), DEFAULT_MINECRAFT_PORT)
    } else if let Some((host, port)) = address.rsplit_once(':') {
        if host.is_empty() {
            return Err(ServerAddressError::Invalid);
        }
        (host.to_ascii_lowercase(), parse_port(port)?)
    } else {
        (address.to_ascii_lowercase(), DEFAULT_MINECRAFT_PORT)
    };

    if host.is_empty() || host.starts_with('.') || host.ends_with('.') || host.contains("..") {
        return Err(ServerAddressError::Invalid);
    }
    let display = if host.contains(':') {
        if port == DEFAULT_MINECRAFT_PORT {
            format!("[{host}]")
        } else {
            format!("[{host}]:{port}")
        }
    } else if port == DEFAULT_MINECRAFT_PORT {
        host.clone()
    } else {
        format!("{host}:{port}")
    };
    Ok(ServerEndpoint {
        host,
        port,
        display,
    })
}

fn parse_port(value: &str) -> Result<u16, ServerAddressError> {
    value
        .parse::<u16>()
        .ok()
        .filter(|port| *port > 0)
        .ok_or(ServerAddressError::Invalid)
}

fn write_string(value: &str, output: &mut Vec<u8>) -> Result<(), ServerPingError> {
    write_length(value.len(), output)?;
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn write_length(value: usize, output: &mut Vec<u8>) -> Result<(), ServerPingError> {
    let value = i32::try_from(value).map_err(|_| ServerPingError::InvalidResponse)?;
    write_varint(value, output);
    Ok(())
}

fn write_varint(value: i32, output: &mut Vec<u8>) {
    let mut value = value as u32;
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        output.push(byte);
        if value == 0 {
            break;
        }
    }
}

async fn read_varint(stream: &mut TcpStream) -> Result<i32, ServerPingError> {
    let mut value = 0_u32;
    for position in 0..5 {
        let byte = stream.read_u8().await?;
        value |= u32::from(byte & 0x7f) << (position * 7);
        if byte & 0x80 == 0 {
            return Ok(value as i32);
        }
    }
    Err(ServerPingError::InvalidResponse)
}

fn read_varint_from_slice(cursor: &mut &[u8]) -> Result<i32, ServerPingError> {
    let mut value = 0_u32;
    for position in 0..5 {
        let (byte, rest) = cursor
            .split_first()
            .ok_or(ServerPingError::InvalidResponse)?;
        *cursor = rest;
        value |= u32::from(byte & 0x7f) << (position * 7);
        if byte & 0x80 == 0 {
            return Ok(value as i32);
        }
    }
    Err(ServerPingError::InvalidResponse)
}

fn description_text(description: Option<&Value>) -> Option<String> {
    let mut output = String::new();
    collect_description(description?, &mut output);
    let normalized = output.split_whitespace().collect::<Vec<_>>().join(" ");
    (!normalized.is_empty()).then_some(normalized)
}

fn collect_description(value: &Value, output: &mut String) {
    if let Some(text) = value.as_str() {
        output.push_str(text);
        return;
    }
    let Some(object) = value.as_object() else {
        return;
    };
    if let Some(text) = object.get("text").and_then(Value::as_str) {
        output.push_str(text);
    }
    if let Some(extra) = object.get("extra").and_then(Value::as_array) {
        for part in extra {
            collect_description(part, output);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{description_text, parse_server_address};
    use serde_json::json;

    #[test]
    fn normalizes_server_addresses() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            parse_server_address("PLAY.Example.COM")?.display,
            "play.example.com"
        );
        assert_eq!(
            parse_server_address("play.example.com:25566")?.display,
            "play.example.com:25566"
        );
        assert_eq!(parse_server_address("[::1]:25566")?.display, "[::1]:25566");
        assert!(parse_server_address("https://example.com").is_err());
        Ok(())
    }

    #[test]
    fn flattens_structured_descriptions() {
        let value = json!({"text": "Welcome ", "extra": [{"text": "home"}]});
        assert_eq!(
            description_text(Some(&value)).as_deref(),
            Some("Welcome home")
        );
    }
}
