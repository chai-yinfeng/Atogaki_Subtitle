//! Session-scoped, read-only streaming access to explicitly registered media.
use axum::{
    Router,
    body::{Body, Bytes},
    extract::{Path as RoutePath, State},
    http::{Method, Request, Response, StatusCode, header},
    routing::get,
};
use std::{
    collections::{HashMap, VecDeque},
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncReadExt, AsyncSeekExt},
    sync::oneshot,
};

const READ_SIZE: usize = 64 * 1024;

struct RegistryInner {
    files: Mutex<HashMap<String, PathBuf>>,
    address: std::net::SocketAddr,
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
}
impl Drop for RegistryInner {
    fn drop(&mut self) {
        if let Some(sender) = self.shutdown.lock().unwrap().take() {
            let _ = sender.send(());
        }
    }
}
#[derive(Clone)]
pub struct PlaybackMediaRegistry(Arc<RegistryInner>);
#[derive(Clone)]
struct ServerState {
    registry: std::sync::Weak<RegistryInner>,
    host: String,
}
impl PlaybackMediaRegistry {
    pub async fn start() -> io::Result<Self> {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        let (shutdown, stopped) = oneshot::channel();
        let registry = Self(Arc::new(RegistryInner {
            files: Mutex::new(HashMap::new()),
            address,
            shutdown: Mutex::new(Some(shutdown)),
        }));
        let state = ServerState {
            registry: Arc::downgrade(&registry.0),
            host: address.to_string(),
        };
        let app = Router::new()
            .route("/media/{token}", get(serve))
            .with_state(state);
        tokio::spawn(async move {
            // Dropping the last registry closes the listener. Active bodies own
            // only their file handles, never the registry or application state.
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await;
        });
        Ok(registry)
    }
    pub fn register(&self, path: &Path) -> Result<String, String> {
        let path = path
            .canonicalize()
            .map_err(|e| format!("failed to resolve local media: {e}"))?;
        if !path.is_file() {
            return Err("local media is not a file".into());
        }
        let mut files = self
            .0
            .files
            .lock()
            .map_err(|_| "local media registry lock was poisoned")?;
        let token = files
            .iter()
            .find(|(_, p)| **p == path)
            .map(|(t, _)| t.clone())
            .unwrap_or_else(|| {
                let token = uuid::Uuid::new_v4().to_string();
                files.insert(token.clone(), path);
                token
            });
        Ok(format!("http://{}/media/{token}", self.0.address))
    }
}

fn empty(status: StatusCode) -> Response<Body> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_LENGTH, "0")
        .body(Body::empty())
        .unwrap()
}
fn permitted_origin(value: &str) -> bool {
    // Packaged Tauri origins plus the existing local development frontend.
    matches!(
        value,
        "tauri://localhost" | "http://tauri.localhost" | "https://tauri.localhost"
    ) || (cfg!(debug_assertions) && value == "http://127.0.0.1:1420")
}
async fn serve(
    State(state): State<ServerState>,
    RoutePath(token): RoutePath<String>,
    request: Request<Body>,
) -> Response<Body> {
    if std::env::var_os("ATOGAKI_MEDIA_DIAGNOSTICS").is_some() {
        // Deliberately omit the URL token, path and media contents.
        eprintln!(
            "[media] method={} range={:?} origin={:?}",
            request.method(),
            request.headers().get(header::RANGE),
            request.headers().get(header::ORIGIN)
        );
    }
    if request
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        != Some(state.host.as_str())
    {
        return empty(StatusCode::FORBIDDEN);
    }
    let origin = request.headers().get(header::ORIGIN);
    if origin.is_some_and(|v| !v.to_str().is_ok_and(permitted_origin)) {
        return empty(StatusCode::FORBIDDEN);
    }
    let path = state
        .registry
        .upgrade()
        .and_then(|r| r.files.lock().ok()?.get(&token).cloned());
    let Some(path) = path else {
        return empty(StatusCode::NOT_FOUND);
    };
    let Ok(file) = tokio::fs::File::open(&path).await else {
        return empty(StatusCode::NOT_FOUND);
    };
    let Ok(metadata) = file.metadata().await else {
        return empty(StatusCode::INTERNAL_SERVER_ERROR);
    };
    let length = metadata.len();
    if std::env::var_os("ATOGAKI_MEDIA_DIAGNOSTICS").is_some() {
        eprintln!("[media] opened bytes={length}");
    }

    let content_type = media_content_type(&path);
    let mut response = Response::builder()
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "no-store")
        .header("X-Content-Type-Options", "nosniff");
    if let Some(origin) = origin {
        response = response
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin)
            .header(header::VARY, "Origin");
    }
    // HTTP Range applies to GET. HEAD describes the full representation.
    if request.method() == Method::HEAD {
        return response
            .header(header::CONTENT_TYPE, content_type)
            .header(header::CONTENT_LENGTH, length)
            .body(Body::empty())
            .unwrap();
    }
    let mut pieces = VecDeque::new();
    let output_length;
    if let Some(range) = request.headers().get(header::RANGE) {
        let ranges = range
            .to_str()
            .map_err(|_| ())
            .and_then(|v| parse_byte_ranges(v, length));
        let Ok(ranges) = ranges else {
            return response
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(header::CONTENT_RANGE, format!("bytes */{length}"))
                .header(header::CONTENT_LENGTH, 0)
                .body(Body::empty())
                .unwrap();
        };
        response = response.status(StatusCode::PARTIAL_CONTENT);
        if ranges.len() == 1 {
            let (start, end) = ranges[0];
            output_length = end - start + 1;
            pieces.push_back(Piece::File(start, output_length));
            response = response.header(header::CONTENT_TYPE, content_type).header(
                header::CONTENT_RANGE,
                format!("bytes {start}-{end}/{length}"),
            );
        } else {
            let boundary = "atogaki-media-boundary";
            let mut total = 0u64;
            for (start, end) in ranges {
                let prefix = format!(
                    "--{boundary}\r\nContent-Type: {content_type}\r\nContent-Range: bytes {start}-{end}/{length}\r\n\r\n"
                );
                let Some(next) = total
                    .checked_add(prefix.len() as u64)
                    .and_then(|n| n.checked_add(end - start + 1))
                    .and_then(|n| n.checked_add(2))
                else {
                    return empty(StatusCode::RANGE_NOT_SATISFIABLE);
                };
                total = next;
                pieces.push_back(Piece::Bytes(Bytes::from(prefix)));
                pieces.push_back(Piece::File(start, end - start + 1));
                pieces.push_back(Piece::Bytes(Bytes::from_static(b"\r\n")));
            }
            let suffix = format!("--{boundary}--\r\n");
            let Some(total) = total.checked_add(suffix.len() as u64) else {
                return empty(StatusCode::RANGE_NOT_SATISFIABLE);
            };
            output_length = total;
            pieces.push_back(Piece::Bytes(Bytes::from(suffix)));
            response = response.header(
                header::CONTENT_TYPE,
                format!("multipart/byteranges; boundary={boundary}"),
            );
        }
    } else {
        output_length = length;
        pieces.push_back(Piece::File(0, length));
        response = response.header(header::CONTENT_TYPE, content_type);
    }
    response
        .header(header::CONTENT_LENGTH, output_length)
        .body(stream_file(file, pieces))
        .unwrap()
}
enum Piece {
    Bytes(Bytes),
    File(u64, u64),
}
fn stream_file(file: tokio::fs::File, pieces: VecDeque<Piece>) -> Body {
    Body::from_stream(futures_util::stream::try_unfold(
        (file, pieces),
        |(mut file, mut pieces)| async move {
            loop {
                match pieces.pop_front() {
                    None => return Ok::<_, io::Error>(None),
                    Some(Piece::Bytes(bytes)) => return Ok(Some((bytes, (file, pieces)))),
                    Some(Piece::File(_, 0)) => continue,
                    Some(Piece::File(start, left)) => {
                        file.seek(io::SeekFrom::Start(start)).await?;
                        let mut bytes = vec![0; left.min(READ_SIZE as u64) as usize];
                        let count = file.read(&mut bytes).await?;
                        if count == 0 {
                            return Err(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                "media changed during playback",
                            ));
                        }
                        bytes.truncate(count);
                        if left > count as u64 {
                            pieces
                                .push_front(Piece::File(start + count as u64, left - count as u64));
                        }
                        return Ok(Some((Bytes::from(bytes), (file, pieces))));
                    }
                }
            }
        },
    ))
}
fn media_content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "mp3" => "audio/mpeg",
        "m4a" | "aac" => "audio/mp4",
        "wav" => "audio/wav",
        _ => "application/octet-stream",
    }
}

fn parse_byte_ranges(header: &str, length: u64) -> Result<Vec<(u64, u64)>, ()> {
    let value = header.strip_prefix("bytes=").ok_or(())?;
    if length == 0 {
        return Err(());
    }
    if value.split(',').count() > 16 {
        return Err(());
    }
    value
        .split(',')
        .map(|part| {
            let (start, end) = part.trim().split_once('-').ok_or(())?;
            let (start, end) = if start.is_empty() {
                let suffix = end.parse::<u64>().map_err(|_| ())?.min(length);
                if suffix == 0 {
                    return Err(());
                }
                (length - suffix, length - 1)
            } else {
                let start = start.parse::<u64>().map_err(|_| ())?;
                let end = if end.is_empty() {
                    length - 1
                } else {
                    end.parse::<u64>().map_err(|_| ())?.min(length - 1)
                };
                (start, end)
            };
            if start >= length || end < start {
                return Err(());
            }
            Ok((start, end))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;
    async fn fixture(data: &[u8]) -> (PlaybackMediaRegistry, String, PathBuf) {
        let path =
            std::env::temp_dir().join(format!("atogaki-stream-{}.mp4", uuid::Uuid::new_v4()));
        std::fs::write(&path, data).unwrap();
        let registry = PlaybackMediaRegistry::start().await.unwrap();
        let url = registry.register(&path).unwrap();
        (registry, url, path)
    }
    fn client() -> reqwest::Client {
        reqwest::Client::builder().no_proxy().build().unwrap()
    }
    #[tokio::test]
    async fn http_ranges_head_multipart_and_access_boundary() {
        let (registry, url, path) = fixture(b"0123456789").await;
        let c = client();
        assert_eq!(registry.register(&path).unwrap(), url);
        let head = c.head(&url).send().await.unwrap();
        assert_eq!(head.headers()["content-length"], "10");
        assert!(head.bytes().await.unwrap().is_empty());
        for (range, expected, span) in [
            ("bytes=2-4", "234", "2-4"),
            ("bytes=7-", "789", "7-9"),
            ("bytes=-3", "789", "7-9"),
        ] {
            let r = c.get(&url).header("range", range).send().await.unwrap();
            assert_eq!(r.status(), 206);
            assert_eq!(r.headers()["content-range"], format!("bytes {span}/10"));
            assert_eq!(r.text().await.unwrap(), expected);
        }
        let r = c
            .get(&url)
            .header("range", "bytes=0-1,8-9")
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 206);
        let length = r.content_length().unwrap();
        let text = r.text().await.unwrap();
        assert_eq!(text.len() as u64, length);
        assert!(text.contains("Content-Range: bytes 0-1/10\r\n\r\n01\r\n"));
        assert!(text.contains("Content-Range: bytes 8-9/10\r\n\r\n89\r\n"));
        for range in [
            "bytes=20-30",
            "bytes=5-2",
            "bytes=-0",
            "bytes=",
            "items=1-2",
        ] {
            assert_eq!(
                c.get(&url)
                    .header("range", range)
                    .send()
                    .await
                    .unwrap()
                    .status(),
                416
            );
        }
        assert_eq!(
            c.get(&url).send().await.unwrap().text().await.unwrap(),
            "0123456789"
        );
        assert_eq!(
            c.get(&url)
                .header("origin", "https://attacker.example")
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
        assert_eq!(
            c.get(&url)
                .header("host", "attacker.example")
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
        assert_eq!(c.post(&url).send().await.unwrap().status(), 405);
        let r = c
            .get(&url)
            .header("origin", "tauri://localhost")
            .send()
            .await
            .unwrap();
        assert_eq!(
            r.headers()["access-control-allow-origin"],
            "tauri://localhost"
        );
        let unknown = format!(
            "http://{}/media/{}",
            registry.0.address,
            uuid::Uuid::new_v4()
        );
        assert_eq!(c.get(unknown).send().await.unwrap().status(), 404);
        std::fs::remove_file(path).unwrap();
        assert_eq!(c.get(&url).send().await.unwrap().status(), 404);
    }
    #[tokio::test]
    async fn large_sparse_file_streams_unclipped_ranges_and_disconnects_without_full_buffer() {
        let (registry, url, path) = fixture(b"").await;
        let length = 9_200_000_123u64;
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(length)
            .unwrap();
        let c = client();
        let r = c.head(&url).send().await.unwrap();
        assert_eq!(r.headers()["content-length"], length.to_string());
        let r = c
            .get(&url)
            .header("range", format!("bytes={}-", length - 4))
            .send()
            .await
            .unwrap();
        assert_eq!(r.bytes().await.unwrap().as_ref(), &[0, 0, 0, 0]);
        let r = c
            .get(&url)
            .header("range", "bytes=0-3145727")
            .send()
            .await
            .unwrap();
        assert_eq!(r.content_length(), Some(3 * 1024 * 1024));
        assert_eq!(r.bytes().await.unwrap().len(), 3 * 1024 * 1024);
        let r = c.get(&url).send().await.unwrap();
        assert_eq!(r.status(), 200);
        assert_eq!(r.content_length(), Some(length));
        let mut body = r.bytes_stream();
        assert!(!body.next().await.unwrap().unwrap().is_empty());
        drop(body);
        // Directly exercise back-pressure: one poll reads at most 64 KiB,
        // regardless of the full-file length. No transport buffering inference.
        let f = tokio::fs::File::open(&path).await.unwrap();
        let body = stream_file(f, VecDeque::from([Piece::File(0, length)]));
        let mut stream = body.into_data_stream();
        assert_eq!(stream.next().await.unwrap().unwrap().len(), READ_SIZE);
        drop(stream);
        let address = registry.0.address;
        drop(registry);
        drop(c);
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if tokio::net::TcpStream::connect(address).await.is_err() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        std::fs::remove_file(path).unwrap();
    }
    #[tokio::test]
    async fn empty_and_truncated_files_have_explicit_boundaries() {
        let (_registry, url, path) = fixture(b"").await;
        let c = client();
        assert!(
            c.get(&url)
                .send()
                .await
                .unwrap()
                .bytes()
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            c.get(&url)
                .header("range", "bytes=0-")
                .send()
                .await
                .unwrap()
                .status(),
            416
        );
        let f = tokio::fs::File::open(&path).await.unwrap();
        let body = stream_file(f, VecDeque::from([Piece::File(0, 10)]));
        assert!(body.into_data_stream().next().await.unwrap().is_err());
        std::fs::remove_file(path).unwrap();
        assert!(parse_byte_ranges(&format!("bytes={}", vec!["0-1"; 17].join(",")), 10).is_err());
    }
}
