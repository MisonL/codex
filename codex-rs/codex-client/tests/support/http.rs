use std::io;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;

pub struct PlainHttpOrigin {
    pub url: String,
    pub request_rx: Receiver<Result<String, String>>,
}

pub fn spawn_plain_http_origin() -> PlainHttpOrigin {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .unwrap_or_else(|error| panic!("bind HTTP origin failed: {error}"));
    let port = listener
        .local_addr()
        .unwrap_or_else(|error| panic!("read HTTP origin local addr failed: {error}"))
        .port();
    let url = format!("https://token-origin.test:{port}/oauth/token");
    let (request_tx, request_rx) = mpsc::channel();

    thread::spawn(move || {
        let result = listener
            .accept()
            .map_err(|error| format!("origin accept failed: {error}"))
            .and_then(|(mut stream, _)| {
                let request = read_http_request(&mut stream)
                    .map_err(|error| format!("origin read failed: {error}"))?;
                write_ok_response(&mut stream)
                    .map_err(|error| format!("origin write failed: {error}"))?;
                Ok(request)
            });
        let _ = request_tx.send(result);
    });

    PlainHttpOrigin { url, request_rx }
}

pub fn read_http_request(stream: &mut impl Read) -> io::Result<String> {
    let mut buffer = Vec::new();
    let header_end = loop {
        let mut chunk = [0u8; 512];
        let bytes_read = stream.read(&mut chunk)?;
        if bytes_read == 0 {
            break header_end(&buffer).unwrap_or(buffer.len());
        }
        buffer.extend_from_slice(&chunk[..bytes_read]);
        if let Some(header_end) = header_end(&buffer) {
            break header_end;
        }
    };

    let content_length = content_length(&buffer[..header_end])?;
    while buffer.len() < header_end + content_length {
        let mut chunk = [0u8; 512];
        let bytes_read = stream.read(&mut chunk)?;
        if bytes_read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..bytes_read]);
    }

    Ok(String::from_utf8_lossy(&buffer).into_owned())
}

pub fn write_ok_response(stream: &mut impl Write) -> io::Result<()> {
    stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
}

fn header_end(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|idx| idx + 4)
}

fn content_length(headers: &[u8]) -> io::Result<usize> {
    let header_text = String::from_utf8_lossy(headers);
    for line in header_text.lines() {
        if let Some(value) = line.strip_prefix("Content-Length:") {
            return value
                .trim()
                .parse::<usize>()
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error));
        }
    }
    Ok(0)
}
