//! flock-fetch: query OpenStreetMap's Overpass API for automated license plate
//! readers (Flock Safety and others) inside a bounding box and write a
//! normalized JSON file the Node server can serve.
//!
//! Data source: crowdsourced OSM nodes tagged man_made=surveillance with
//! surveillance:type=ALPR and/or manufacturer=Flock Safety (same source
//! DeFlock uses). © OpenStreetMap contributors, ODbL.

use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

/// Denver metro default bbox: south,west,north,east
const DENVER_BBOX: [f64; 4] = [39.45, -105.35, 40.15, -104.55];

#[derive(Parser, Debug)]
#[command(name = "flock-fetch", about = "Fetch ALPR camera locations from OpenStreetMap")]
struct Args {
    /// Output JSON path
    #[arg(short, long, default_value = "../data/cameras.json")]
    out: PathBuf,

    /// Bounding box as south,west,north,east (defaults to Denver metro)
    #[arg(long)]
    bbox: Option<String>,

    /// Only keep nodes whose manufacturer tag mentions Flock
    #[arg(long)]
    flock_only: bool,

    /// Overpass endpoint
    #[arg(long, default_value = "https://overpass-api.de/api/interpreter")]
    endpoint: String,

    /// Retries for transient Overpass failures (429/5xx, timeouts, truncated bodies)
    #[arg(long, default_value_t = 3)]
    retries: u32,
}

#[derive(Deserialize)]
struct OverpassResponse {
    elements: Vec<OverpassNode>,
}

#[derive(Deserialize)]
struct OverpassNode {
    id: u64,
    lat: f64,
    lon: f64,
    #[serde(default)]
    tags: BTreeMap<String, String>,
}

#[derive(Serialize)]
struct Camera {
    id: u64,
    lat: f64,
    lon: f64,
    manufacturer: Option<String>,
    operator: Option<String>,
    /// camera:direction in degrees, if mapped
    direction: Option<f64>,
    surveillance_type: Option<String>,
    is_flock: bool,
    osm_url: String,
    tags: BTreeMap<String, String>,
}

#[derive(Serialize)]
struct Output {
    generated_at: String,
    bbox: [f64; 4],
    source: &'static str,
    license: &'static str,
    count: usize,
    cameras: Vec<Camera>,
}

fn parse_bbox(s: &str) -> Result<[f64; 4]> {
    let parts: Vec<f64> = s
        .split(',')
        .map(|p| p.trim().parse::<f64>())
        .collect::<std::result::Result<_, _>>()
        .context("bbox must be four comma-separated numbers")?;
    anyhow::ensure!(parts.len() == 4, "bbox must be south,west,north,east");
    Ok([parts[0], parts[1], parts[2], parts[3]])
}

fn build_query(b: &[f64; 4]) -> String {
    let bb = format!("{},{},{},{}", b[0], b[1], b[2], b[3]);
    format!(
        r#"[out:json][timeout:180];
(
  node["man_made"="surveillance"]["surveillance:type"="ALPR"]({bb});
  node["man_made"="surveillance"]["manufacturer"~"[Ff]lock"]({bb});
  node["man_made"="surveillance"]["brand"~"[Ff]lock"]({bb});
);
out body;"#
    )
}

/// Overpass is chronically overloaded; these mean "try again shortly" rather
/// than "the query is wrong".
fn is_transient(status: reqwest::StatusCode) -> bool {
    matches!(status.as_u16(), 429 | 502 | 503 | 504)
}

/// One request, tagged with whether the failure is worth another go.
fn attempt(
    client: &reqwest::blocking::Client,
    endpoint: &str,
    query: &str,
) -> std::result::Result<OverpassResponse, (anyhow::Error, bool)> {
    // Connection resets and read timeouts are transient by nature.
    let resp = client
        .post(endpoint)
        .form(&[("data", query)])
        .send()
        .map_err(|e| (anyhow::Error::new(e).context("Overpass request failed"), true))?;

    let status = resp.status();
    if let Err(e) = resp.error_for_status_ref() {
        let transient = is_transient(status);
        return Err((anyhow::Error::new(e).context(format!("Overpass returned {status}")), transient));
    }

    // An overloaded instance sometimes answers with 200 carrying an HTML error
    // page, so a parse failure here is also worth retrying.
    resp.json()
        .map_err(|e| (anyhow::Error::new(e).context("Overpass response was not valid JSON"), true))
}

fn fetch_with_retry(
    client: &reqwest::blocking::Client,
    endpoint: &str,
    query: &str,
    retries: u32,
) -> Result<OverpassResponse> {
    let mut tried = 0;
    loop {
        match attempt(client, endpoint, query) {
            Ok(r) => return Ok(r),
            Err((e, transient)) => {
                if !transient {
                    return Err(e);
                }
                if tried >= retries {
                    // Only now is "the server is busy" the right diagnosis.
                    return Err(e).context(
                        "Overpass stayed unavailable across every retry; \
                         wait a minute, or pass --endpoint with a mirror",
                    );
                }
                // 5s, 10s, 20s, then 40s for any further attempts.
                let wait = Duration::from_secs(5 << tried.min(3));
                eprintln!("  attempt {} failed: {e:#}", tried + 1);
                eprintln!("  retrying in {}s ...", wait.as_secs());
                std::thread::sleep(wait);
                tried += 1;
            }
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let bbox = match &args.bbox {
        Some(s) => parse_bbox(s)?,
        None => DENVER_BBOX,
    };

    let query = build_query(&bbox);
    eprintln!("Querying {} for bbox {:?} ...", args.endpoint, bbox);

    let client = reqwest::blocking::Client::builder()
        .user_agent("flock-denver/0.1 (personal ALPR map)")
        .timeout(Duration::from_secs(200))
        .build()?;

    let resp = fetch_with_retry(&client, &args.endpoint, &query, args.retries)?;

    let mut cameras: Vec<Camera> = resp
        .elements
        .into_iter()
        .map(|n| {
            let manufacturer = n
                .tags
                .get("manufacturer")
                .or_else(|| n.tags.get("brand"))
                .cloned();
            let is_flock = manufacturer
                .as_deref()
                .map(|m| m.to_ascii_lowercase().contains("flock"))
                .unwrap_or(false);
            Camera {
                id: n.id,
                lat: n.lat,
                lon: n.lon,
                manufacturer,
                operator: n.tags.get("operator").cloned(),
                direction: n
                    .tags
                    .get("camera:direction")
                    .and_then(|d| d.parse::<f64>().ok()),
                surveillance_type: n.tags.get("surveillance:type").cloned(),
                is_flock,
                osm_url: format!("https://www.openstreetmap.org/node/{}", n.id),
                tags: n.tags,
            }
        })
        .filter(|c| !args.flock_only || c.is_flock)
        .collect();

    cameras.sort_by_key(|c| c.id);

    let out = Output {
        generated_at: chrono::Utc::now().to_rfc3339(),
        bbox,
        source: "OpenStreetMap via Overpass API",
        license: "ODbL — © OpenStreetMap contributors",
        count: cameras.len(),
        cameras,
    };

    if let Some(parent) = args.out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.out, serde_json::to_string_pretty(&out)?)
        .with_context(|| format!("could not write {}", args.out.display()))?;

    eprintln!("Wrote {} cameras to {}", out.count, args.out.display());
    Ok(())
}
