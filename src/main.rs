use arecibo::{
    infohash::InfoHash,
    metadata::{TorrentMetadata, TorrentMetadataFile},
    torrent::TorrentBytes,
};
use axum::{
    extract::{Path, State},
    http::{header::CONTENT_TYPE, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use bendy::decoding::FromBencode;
use librqbit::AddTorrentResponse;
use librqbit::{dht::PersistentDhtConfig, AddTorrent, AddTorrentOptions};
use librqbit::{Session, SessionOptions};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{env, io::Cursor, path::PathBuf, sync::Arc, time::SystemTime};
use std::{env::temp_dir, time::Duration};
use tokio::time::timeout;
use tokio::{
    signal::{self},
    sync::Semaphore,
};
use tower_http::compression::CompressionLayer;

// these trackers are generally used for most torrents so they should give us good enough coverage.
// the rest can come from dht
const DEFAULT_TRACKERS: [&str; 11] = [
    "udp://tracker.coppersurfer.tk:6969/announce",
    "udp://open.demonii.com:1337/announce",
    "udp://open.tracker.cl:1337/announce",
    "udp://explodie.org:6969/announce",
    "udp://tracker.leechers-paradise.org:6969/announce",
    "udp://exodus.desync.com:6969/announce",
    "udp://tracker-udp.gbitt.info:80/announce",
    "udp://tracker.opentrackr.org:1337/announce",
    "udp://tracker.torrent.eu.org:451/announce",
    "udp://open.stealth.si:80/announce",
    "udp://open.demonii.com:1337/announce",
];

#[derive(Debug, thiserror::Error)]
pub enum TorrentError {
    #[error("timed out getting torrent file")]
    TimedOut,
    #[error("could not parse torrent")]
    ParseFailed,
    #[error("unexpected response from rqbit")]
    UnexpectedResponse,
    #[error("no slots availalbe")]
    NoSlotsAvailable,
    #[error("failed to get item from cache")]
    CacheFailed,
}

impl IntoResponse for TorrentError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error_message": self.to_string(),
            })),
        )
            .into_response()
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct Cached {
    // zstd-encoded torrent file
    torrent_bytes: Vec<u8>,
    last_used: SystemTime,
}

#[derive(Clone)]
struct AppState {
    db: sled::Db,
    session: Arc<Session>,
    limiter: Arc<Semaphore>,
    cache_size: usize,
    timeout: Duration,
}

#[tokio::main]
pub async fn main() {
    tracing_subscriber::fmt::init();

    let output_dir = temp_dir().join("arecibo"); // we shouldn't write anything here anyway.
    let data_dir = PathBuf::from(env::var("ARECIBO_DATA_DIR").unwrap_or(".arecibo".to_string()));
    let cache_size = env::var("ARECIBO_CACHE_SIZE")
        .unwrap_or("50000".to_string())
        .parse()
        .expect("could not parse ARECIBO_CACHE_SIZE");

    let limiter = {
        let concurrency = env::var("ARECIBO_CONCURRENCY")
            .unwrap_or("100".to_string())
            .parse()
            .expect("could not parse ARECIBO_CONCURRENCY");

        Arc::new(Semaphore::new(concurrency))
    };

    let session = {
        let dht_file = data_dir.join("dht.json");
        let disable_dht = env::var("ARECIBO_DISABLE_DHT").is_ok();

        Session::new_with_opts(
            output_dir,
            SessionOptions {
                disable_upload: true,
                disable_dht: disable_dht,
                concurrent_init_limit: Some(100),
                listen_port_range: Some(4240..4260),
                dht_config: Some(PersistentDhtConfig {
                    config_filename: Some(dht_file.into()),
                    dump_interval: Some(Duration::from_secs(10)),
                }),
                ..Default::default()
            },
        )
        .await
        .expect("Failed to create session")
    };

    let db = {
        let cache_path = data_dir.join("cache");
        sled::open(cache_path).unwrap()
    };

    let timeout = {
        let secs = env::var("ARECIBO_TIMEOUT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);

        Duration::from_secs(secs)
    };

    let state = AppState {
        db,
        session,
        limiter,
        cache_size,
        timeout,
    };

    let compression_layer = CompressionLayer::new();
    let app = Router::new()
        .route("/torrent/{info_hash}/file", get(get_torrent_file))
        .route("/torrent/{info_hash}/metadata", get(get_torrent_metadata))
        .route("/version", get(get_version))
        .layer(compression_layer)
        .with_state(state);

    let bind_host = env::var("ARECIBO_HOST").unwrap_or("0.0.0.0".to_string());
    let bind_port = env::var("ARECIBO_PORT").unwrap_or("3080".to_string());
    let bind_addr = format!("{}:{}", bind_host, bind_port);
    let listener = tokio::net::TcpListener::bind(bind_addr).await.unwrap();
    tracing::info!("Listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn get_version() -> impl IntoResponse {
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
    }))
    .into_response()
}

async fn get_torrent_file(
    State(state): State<AppState>,
    Path(info_hash): Path<InfoHash>,
) -> Result<impl IntoResponse, TorrentError> {
    let (data, was_cached) = get_torrent_bytes(&info_hash, &state).await?;
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "application/x-bittorrent".parse().unwrap());
    headers.insert("x-cached", was_cached.to_string().parse().unwrap());
    Ok((headers, data).into_response())
}

async fn get_torrent_metadata(
    State(state): State<AppState>,
    Path(info_hash): Path<InfoHash>,
) -> Result<impl IntoResponse, TorrentError> {
    let (data, was_cached) = get_torrent_bytes(&info_hash, &state).await?;
    let data = TorrentBytes::from_bencode(&data).map_err(|_| TorrentError::ParseFailed)?;

    let files = if let Some(files) = data.info.files {
        files
            .into_iter()
            .map(|file| {
                let mut path = file
                    .path
                    .into_iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>();

                path.insert(0, data.info.name.clone());
                TorrentMetadataFile {
                    path,
                    size: file.length,
                }
            })
            .collect::<Vec<_>>()
    } else {
        vec![TorrentMetadataFile {
            path: vec![data.info.name.clone()],
            size: data.info.file_length.ok_or(TorrentError::ParseFailed)?,
        }]
    };

    let size = files.iter().map(|f| f.size).sum();
    let name = data.info.name;

    let mut headers = HeaderMap::new();
    headers.insert("x-cached", was_cached.to_string().parse().unwrap());

    Ok((
        headers,
        Json(TorrentMetadata {
            name,
            size,
            created_at: None,
            files,
        }),
    )
        .into_response())
}

async fn get_torrent_bytes(
    info_hash: &InfoHash,
    state: &AppState,
) -> Result<(Vec<u8>, bool), TorrentError> {
    let cached = state.db.get(info_hash.as_bytes()).ok().flatten();
    if let Some(cached) = cached {
        let (data, _): (Cached, _) =
            bincode::serde::decode_from_slice(&cached, bincode::config::standard())
                .map_err(|_| TorrentError::CacheFailed)?;

        let torrent_bytes = Cursor::new(data.torrent_bytes);
        let torrent_bytes =
            zstd::stream::decode_all(torrent_bytes).map_err(|_| TorrentError::CacheFailed)?;

        tracing::debug!("hit cache for {}", info_hash);
        return Ok((torrent_bytes, true));
    }

    let magnet_uri = {
        let magnet_uri = format!("magnet:?xt=urn:btih:{info_hash}");
        let mut magnet_uri = url::Url::parse(&magnet_uri).unwrap();
        for tracker in DEFAULT_TRACKERS.iter() {
            magnet_uri.query_pairs_mut().append_pair("tr", tracker);
        }

        magnet_uri.to_string()
    };

    let guard = state
        .limiter
        .try_acquire()
        .map_err(|_| TorrentError::NoSlotsAvailable)?;

    tracing::debug!("fetching torrent {}", info_hash);
    let handle = timeout(
        state.timeout,
        state.session.add_torrent(
            AddTorrent::from_url(magnet_uri),
            Some(AddTorrentOptions {
                list_only: true,
                ..Default::default()
            }),
        ),
    )
    .await
    .ok();

    drop(guard);
    let Some(handle) = handle else {
        return Err(TorrentError::TimedOut);
    };

    let data = match handle {
        Ok(AddTorrentResponse::ListOnly(data)) => data,
        _ => {
            tracing::error!("unexpected response for hash {}", info_hash);
            return Err(TorrentError::UnexpectedResponse);
        }
    };

    {
        let cursor = Cursor::new(&data.torrent_bytes);
        let compressed = zstd::stream::encode_all(cursor, 6).unwrap();
        let serialized = bincode::serde::encode_to_vec(
            &Cached {
                torrent_bytes: compressed,
                last_used: SystemTime::now(),
            },
            bincode::config::standard(),
        )
        .unwrap();

        state.db.insert(info_hash.as_bytes(), serialized).ok();
        sweep_cache(state);
    }

    Ok((data.torrent_bytes.to_vec(), false))
}

fn sweep_cache(state: &AppState) {
    // Early exit if we're under the cache size limit
    if state.db.len() <= state.cache_size {
        return;
    }

    // Find the item with the oldest last_used timestamp
    let mut oldest_key: Option<Vec<u8>> = None;
    let mut oldest_time: Option<SystemTime> = None;

    for result in state.db.iter() {
        if let Ok((key, value)) = result {
            // Decode the cached entry to get the last_used timestamp
            if let Ok((cached, _)) =
                bincode::serde::decode_from_slice::<Cached, _>(&value, bincode::config::standard())
            {
                // Update oldest if this is the first item or if this item is older
                if oldest_time.map_or(true, |t| cached.last_used < t) {
                    oldest_time = Some(cached.last_used);
                    oldest_key = Some(key.to_vec());
                }
            }
        }
    }

    // Remove the oldest item if we found one
    if let Some(key) = oldest_key {
        if let Err(e) = state.db.remove(key) {
            tracing::warn!("Failed to remove oldest cache entry: {}", e);
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
