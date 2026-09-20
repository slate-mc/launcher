use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use hickory_resolver::TokioResolver;
use hickory_resolver::proto::rr::rdata::SRV;
use serde_json::Value;
use slate_contracts::{ServerStatusSummary, ServerTextSegment};
use std::cmp::Reverse;
use std::net::{IpAddr, Ipv6Addr};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const DEFAULT_MINECRAFT_PORT: u16 = 25_565;
const MAX_FAVICON_BYTES: usize = 192 * 1024;
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
            description_segments: Vec::new(),
            favicon: None,
        },
    })
}

async fn ping_endpoint(endpoint: ServerEndpoint) -> Result<ServerStatusSummary, ServerPingError> {
    let started = Instant::now();
    let (connect_host, connect_port) = resolve_connect_target(&endpoint).await;
    let mut stream = TcpStream::connect((connect_host.as_str(), connect_port)).await?;
    stream.set_nodelay(true)?;

    let mut handshake = Vec::new();
    write_varint(0, &mut handshake);
    write_varint(-1, &mut handshake);
    write_string(&endpoint.host, &mut handshake)?;
    handshake.extend_from_slice(&connect_port.to_be_bytes());
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

    let description_segments = parse_description(value.get("description"));
    let description = joined_description(&description_segments);
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
        description,
        description_segments,
        favicon: validated_favicon(value.get("favicon").and_then(Value::as_str)),
    })
}

async fn resolve_connect_target(endpoint: &ServerEndpoint) -> (String, u16) {
    if endpoint.port != DEFAULT_MINECRAFT_PORT || endpoint.host.parse::<IpAddr>().is_ok() {
        return (endpoint.host.clone(), endpoint.port);
    }
    let Ok(builder) = TokioResolver::builder_tokio() else {
        return (endpoint.host.clone(), endpoint.port);
    };
    let Ok(resolver) = builder.build() else {
        return (endpoint.host.clone(), endpoint.port);
    };
    let query = format!("_minecraft._tcp.{}.", endpoint.host);
    let Ok(lookup) = resolver.srv_lookup(query).await else {
        return (endpoint.host.clone(), endpoint.port);
    };
    lookup
        .answers()
        .iter()
        .filter_map(|record| record.try_borrow::<SRV>())
        .min_by_key(|record| {
            let data = record.data();
            (data.priority, Reverse(data.weight))
        })
        .map_or_else(
            || (endpoint.host.clone(), endpoint.port),
            |record| {
                let data = record.data();
                (
                    data.target.to_utf8().trim_end_matches('.').to_owned(),
                    data.port,
                )
            },
        )
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct TextStyle {
    color: Option<String>,
    bold: bool,
    italic: bool,
    underlined: bool,
    strikethrough: bool,
    obfuscated: bool,
}

fn parse_description(description: Option<&Value>) -> Vec<ServerTextSegment> {
    let mut output = Vec::new();
    if let Some(description) = description {
        collect_component(description, &TextStyle::default(), &mut output);
    }
    output
}

fn collect_component(value: &Value, inherited: &TextStyle, output: &mut Vec<ServerTextSegment>) {
    if let Some(text) = value.as_str() {
        append_legacy_text(text, inherited.clone(), output);
        return;
    }
    if let Some(array) = value.as_array() {
        for part in array {
            collect_component(part, inherited, output);
        }
        return;
    }
    let Some(object) = value.as_object() else {
        return;
    };
    let style = component_style(object, inherited);
    if let Some(text) = object.get("text").and_then(Value::as_str) {
        append_legacy_text(text, style.clone(), output);
    } else if let Some(arguments) = object.get("with").and_then(Value::as_array) {
        for argument in arguments {
            collect_component(argument, &style, output);
        }
    }
    if let Some(extra) = object.get("extra").and_then(Value::as_array) {
        for part in extra {
            collect_component(part, &style, output);
        }
    }
}

fn component_style(object: &serde_json::Map<String, Value>, inherited: &TextStyle) -> TextStyle {
    let mut style = inherited.clone();
    if let Some(color) = object.get("color").and_then(Value::as_str) {
        if color == "reset" {
            style = TextStyle::default();
        } else if let Some(color) = normalize_chat_color(color) {
            style.color = Some(color);
        }
    }
    override_bool(object, "bold", &mut style.bold);
    override_bool(object, "italic", &mut style.italic);
    override_bool(object, "underlined", &mut style.underlined);
    override_bool(object, "strikethrough", &mut style.strikethrough);
    override_bool(object, "obfuscated", &mut style.obfuscated);
    style
}

fn override_bool(object: &serde_json::Map<String, Value>, key: &str, target: &mut bool) {
    if let Some(value) = object.get(key).and_then(Value::as_bool) {
        *target = value;
    }
}

fn append_legacy_text(text: &str, mut style: TextStyle, output: &mut Vec<ServerTextSegment>) {
    let characters: Vec<char> = text.chars().collect();
    let mut buffer = String::new();
    let mut index = 0;
    while index < characters.len() {
        if characters[index] != '§' || index + 1 >= characters.len() {
            buffer.push(characters[index]);
            index += 1;
            continue;
        }
        push_segment(&mut buffer, &style, output);
        let code = characters[index + 1].to_ascii_lowercase();
        if code == 'x'
            && let Some((color, consumed)) = legacy_hex_color(&characters[index + 2..])
        {
            style = TextStyle {
                color: Some(color),
                ..TextStyle::default()
            };
            index += consumed + 2;
            continue;
        }
        match code {
            '0'..='9' | 'a'..='f' => {
                style = TextStyle {
                    color: legacy_color(code),
                    ..TextStyle::default()
                };
            }
            'k' => style.obfuscated = true,
            'l' => style.bold = true,
            'm' => style.strikethrough = true,
            'n' => style.underlined = true,
            'o' => style.italic = true,
            'r' => style = TextStyle::default(),
            _ => {}
        }
        index += 2;
    }
    push_segment(&mut buffer, &style, output);
}

fn legacy_hex_color(characters: &[char]) -> Option<(String, usize)> {
    if characters.len() < 12 {
        return None;
    }
    let mut color = String::from("#");
    for pair in characters[..12].chunks_exact(2) {
        if pair[0] != '§' || !pair[1].is_ascii_hexdigit() {
            return None;
        }
        color.push(pair[1].to_ascii_uppercase());
    }
    Some((color, 12))
}

fn push_segment(buffer: &mut String, style: &TextStyle, output: &mut Vec<ServerTextSegment>) {
    if buffer.is_empty() {
        return;
    }
    let segment = ServerTextSegment {
        text: std::mem::take(buffer),
        color: style.color.clone(),
        bold: style.bold,
        italic: style.italic,
        underlined: style.underlined,
        strikethrough: style.strikethrough,
        obfuscated: style.obfuscated,
    };
    if let Some(previous) = output.last_mut()
        && same_segment_style(previous, &segment)
    {
        previous.text.push_str(&segment.text);
    } else {
        output.push(segment);
    }
}

fn same_segment_style(left: &ServerTextSegment, right: &ServerTextSegment) -> bool {
    left.color == right.color
        && left.bold == right.bold
        && left.italic == right.italic
        && left.underlined == right.underlined
        && left.strikethrough == right.strikethrough
        && left.obfuscated == right.obfuscated
}

fn legacy_color(code: char) -> Option<String> {
    normalize_chat_color(match code {
        '0' => "black",
        '1' => "dark_blue",
        '2' => "dark_green",
        '3' => "dark_aqua",
        '4' => "dark_red",
        '5' => "dark_purple",
        '6' => "gold",
        '7' => "gray",
        '8' => "dark_gray",
        '9' => "blue",
        'a' => "green",
        'b' => "aqua",
        'c' => "red",
        'd' => "light_purple",
        'e' => "yellow",
        'f' => "white",
        _ => return None,
    })
}

fn normalize_chat_color(color: &str) -> Option<String> {
    let normalized = match color {
        "black" => "#000000",
        "dark_blue" => "#0000AA",
        "dark_green" => "#00AA00",
        "dark_aqua" => "#00AAAA",
        "dark_red" => "#AA0000",
        "dark_purple" => "#AA00AA",
        "gold" => "#FFAA00",
        "gray" => "#AAAAAA",
        "dark_gray" => "#555555",
        "blue" => "#5555FF",
        "green" => "#55FF55",
        "aqua" => "#55FFFF",
        "red" => "#FF5555",
        "light_purple" => "#FF55FF",
        "yellow" => "#FFFF55",
        "white" => "#FFFFFF",
        custom if is_hex_color(custom) => return Some(custom.to_ascii_uppercase()),
        _ => return None,
    };
    Some(normalized.to_owned())
}

fn is_hex_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

fn joined_description(segments: &[ServerTextSegment]) -> Option<String> {
    let joined = segments
        .iter()
        .map(|segment| segment.text.as_str())
        .collect::<String>();
    let normalized = joined.split_whitespace().collect::<Vec<_>>().join(" ");
    (!normalized.is_empty()).then_some(normalized)
}

fn validated_favicon(value: Option<&str>) -> Option<String> {
    const PREFIX: &str = "data:image/png;base64,";
    let value = value?;
    if !value.get(..PREFIX.len())?.eq_ignore_ascii_case(PREFIX) {
        return None;
    }
    let encoded = value
        .get(PREFIX.len()..)?
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect::<String>();
    if encoded.len() > MAX_FAVICON_BYTES.saturating_mul(2) {
        return None;
    }
    let decoded = BASE64_STANDARD.decode(encoded).ok()?;
    if decoded.len() > MAX_FAVICON_BYTES
        || decoded.len() < 24
        || decoded[..8] != [137, 80, 78, 71, 13, 10, 26, 10]
        || u32::from_be_bytes(decoded[16..20].try_into().ok()?) != 64
        || u32::from_be_bytes(decoded[20..24].try_into().ok()?) != 64
    {
        return None;
    }
    Some(format!("{PREFIX}{}", BASE64_STANDARD.encode(decoded)))
}

#[cfg(test)]
mod tests {
    use super::{joined_description, parse_description, parse_server_address, validated_favicon};
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
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
    fn parses_structured_and_legacy_descriptions() {
        let value = json!({
            "text": "§aWelcome ",
            "extra": [{"text": "home", "color": "gold", "bold": true}]
        });
        let segments = parse_description(Some(&value));
        assert_eq!(
            joined_description(&segments).as_deref(),
            Some("Welcome home")
        );
        assert_eq!(segments[0].color.as_deref(), Some("#55FF55"));
        assert_eq!(segments[1].color.as_deref(), Some("#FFAA00"));
        assert!(segments[1].bold);
    }

    #[test]
    fn accepts_only_bounded_64_pixel_png_favicons() {
        let mut png = vec![0_u8; 24];
        png[..8].copy_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);
        png[16..20].copy_from_slice(&64_u32.to_be_bytes());
        png[20..24].copy_from_slice(&64_u32.to_be_bytes());
        let encoded = BASE64_STANDARD.encode(png);
        let favicon = format!("data:image/png;base64,{encoded}");
        assert_eq!(
            validated_favicon(Some(&favicon)).as_deref(),
            Some(favicon.as_str())
        );
        let wrapped = format!("DATA:IMAGE/PNG;BASE64,\n {encoded}\n");
        assert_eq!(
            validated_favicon(Some(&wrapped)).as_deref(),
            Some(favicon.as_str())
        );
        assert!(validated_favicon(Some("data:image/svg+xml;base64,bad")).is_none());
    }
}
