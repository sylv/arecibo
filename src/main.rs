use crate::metadata::{TorrentMetadata, TorrentMetadataFile};
use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use error::AppError;
use librqbit::AddTorrentResponse;
use librqbit::{dht::PersistentDhtConfig, AddTorrent, AddTorrentOptions};
use librqbit::{Session, SessionOptions};
use serde_json::json;
use std::{env, sync::Arc};
use std::{env::temp_dir, time::Duration};
use tokio::signal::{self};
use tokio::time::timeout;
use tower_http::compression::CompressionLayer;
use tracing::{debug, info};

mod error;
mod metadata;

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

#[tokio::main]
pub async fn main() {
    tracing_subscriber::fmt::init();
    dotenv::dotenv().ok();

    let output_dir = temp_dir().join("arecibo"); // we shouldn't write anything here anyway.
    let dht_file = env::var("ARECIBO_DHT_FILE").unwrap_or("dht.json".to_string());
    let disable_dht = env::var("ARECIBO_DISABLE_DHT").is_ok();
    let session = Session::new_with_opts(
        output_dir,
        SessionOptions {
            disable_upload: true,
            disable_dht: disable_dht,
            concurrent_init_limit: Some(100),
            listen_port_range: Some(4240..4260),
            dht_config: Some(PersistentDhtConfig {
                config_filename: Some(dht_file.into()),
                dump_interval: Some(Duration::from_secs(30)),
            }),
            ..Default::default()
        },
    )
    .await
    .expect("Failed to create session");

    let compression_layer = CompressionLayer::new();
    let app = Router::new()
        .route("/torrents/{info_hash}", get(torrent_meta))
        .route("/version", get(get_version))
        .layer(compression_layer)
        .with_state(session);

    let bind_host = env::var("ARECIBO_HOST").unwrap_or("0.0.0.0".to_string());
    let bind_port = env::var("ARECIBO_PORT").unwrap_or("3080".to_string());
    let bind_addr = format!("{}:{}", bind_host, bind_port);
    let listener = tokio::net::TcpListener::bind(bind_addr).await.unwrap();
    info!("Listening on {}", listener.local_addr().unwrap());
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

async fn torrent_meta(
    State(session): State<Arc<Session>>,
    Path(info_hash): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let info_hash = info_hash.to_lowercase();
    debug!("Getting metadata for {}", info_hash);

    let magnet_uri = {
        let magnet_uri = format!("magnet:?xt=urn:btih:{}", info_hash);
        let mut magnet_uri = url::Url::parse(&magnet_uri).unwrap();
        for tracker in DEFAULT_TRACKERS.iter() {
            magnet_uri.query_pairs_mut().append_pair("tr", tracker);
        }

        magnet_uri.to_string()
    };

    let timeout_secs = env::var("ARECIBO_LOCAL_TIMEOUT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    let timer = Duration::from_secs(timeout_secs);
    let handle = timeout(
        timer,
        session.add_torrent(
            AddTorrent::from_url(magnet_uri),
            Some(AddTorrentOptions {
                list_only: true,
                ..Default::default()
            }),
        ),
    )
    .await
    .ok();

    if handle.is_none() {
        return Ok((
            StatusCode::NOT_FOUND,
            "Could not find metadata for this torrent",
        )
            .into_response());
    }

    let handle = handle.unwrap()?;
    let data = match handle {
        AddTorrentResponse::ListOnly(data) => data,
        AddTorrentResponse::AlreadyManaged(_, _) | AddTorrentResponse::Added(_, _) => {
            return Ok((StatusCode::INTERNAL_SERVER_ERROR, "Unexpected response").into_response())
        }
    };

    let files = if let Some(files) = data.info.files {
        Some(
            files
                .into_iter()
                .map(|file| TorrentMetadataFile {
                    path: file.path.into_iter().map(|p| p.to_string()).collect(),
                    size: file.length,
                })
                .collect::<Vec<_>>(),
        )
    } else {
        None
    };

    let size = if let Some(files) = &files {
        files.iter().map(|f| f.size).sum()
    } else {
        data.info.length.expect("Torrent size not found")
    };

    let name = data.info.name.expect("Torrent name not found").to_string();
    Ok(Json(TorrentMetadata { name, size, files }).into_response())
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
