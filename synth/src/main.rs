//! Renders the album "Night Bus Polaroids": six tracks synthesized sample by sample, moving from
//! Boards of Canada / Four Tet / Ben Böhmer tape nostalgia into Fred again.. vocal-chop techno and
//! Burial's rain-soaked 2-step and dubstep.
//!
//! Usage: soundscape [OUT_DIR]   (default: ./out, or $SOUNDSCAPE_OUT)
//!   SOUNDSCAPE_ONLY=slug[,slug]  render a subset (album.json still lists every track rendered so far)
//!   SOUNDSCAPE_JOBS=n            parallel renders (default: min(cores, 3); each track holds ~0.5 GB)
//! Writes OUT_DIR/<slug>.wav + OUT_DIR/<slug>.json per track and OUT_DIR/album.json.

mod dsp;
mod instruments;
mod track;
mod tracks;
mod voices;
mod wav;

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use track::*;

const ALBUM: &str = "Night Bus Polaroids";
const ALBUM_BLURB: &str = "Cassette nostalgia goes out after dark: Boards of Canada, Four Tet and Ben Böhmer \
meet Fred again.. vocal-chop techno and Burial's rain-soaked 2-step.";

struct Summary {
    slug: &'static str,
    title: &'static str,
    influences: &'static [&'static str],
    blurb: &'static str,
    bpm: f32,
    key: &'static str,
    duration: f32,
    render_ms: u128,
}

fn render_one(number: usize, render: fn() -> Track, out_dir: &Path) -> std::io::Result<Summary> {
    let started = Instant::now();
    let track = render();
    let render_ms = started.elapsed().as_millis();
    wav::write_wav16(&out_dir.join(format!("{}.wav", track.slug)), &track.audio)?;
    write_metadata(&out_dir.join(format!("{}.json", track.slug)), &track, number, render_ms)?;
    let d = duration(&track);
    eprintln!("✓ {:>2}. {} — {:.0}:{:02.0}, rendered in {:.1}s", number, track.title, (d / 60.0).floor(), d % 60.0, render_ms as f32 / 1000.0);
    Ok(Summary {
        slug: track.slug,
        title: track.title,
        influences: track.influences,
        blurb: track.blurb,
        bpm: track.bpm,
        key: track.key,
        duration: d,
        render_ms,
    })
}

/// album.json lists every track whose metadata exists in OUT_DIR, in album order.
fn write_album(out_dir: &Path, fresh: &[Summary]) -> std::io::Result<usize> {
    let mut entries = Vec::new();
    for (i, &(slug, _)) in tracks::ALL.iter().enumerate() {
        if let Some(s) = fresh.iter().find(|s| s.slug == slug) {
            entries.push(format!(
                "    {{\"number\": {}, \"slug\": {}, \"title\": {}, \"influences\": [{}], \"blurb\": {}, \"bpm\": {}, \"key\": {}, \"duration\": {:.3}, \"renderMs\": {}, \"audio\": {}}}",
                i + 1, json_str(s.slug), json_str(s.title), json_list(s.influences), json_str(s.blurb),
                s.bpm, json_str(s.key), s.duration, s.render_ms, json_str(&format!("/audio/{}.wav", s.slug))
            ));
        } else if let Ok(prev) = fs::read_to_string(out_dir.join("album.json")) {
            // Keep the entry from an earlier full render when re-rendering a subset.
            if let Some(line) = prev.lines().find(|l| l.contains(&format!("\"slug\": \"{slug}\""))) {
                entries.push(line.trim_end_matches(',').to_string());
            }
        }
    }
    let mut j = String::from("{\n");
    let _ = writeln!(j, "  \"title\": {},", json_str(ALBUM));
    let _ = writeln!(j, "  \"artist\": {},", json_str(ARTIST));
    let _ = writeln!(j, "  \"blurb\": {},", json_str(ALBUM_BLURB));
    let _ = writeln!(j, "  \"tracks\": [\n{}\n  ]", entries.join(",\n"));
    j.push_str("}\n");
    fs::write(out_dir.join("album.json"), j)?;
    Ok(entries.len())
}

fn main() -> std::io::Result<()> {
    let out_dir = PathBuf::from(
        std::env::args().nth(1).or_else(|| std::env::var("SOUNDSCAPE_OUT").ok()).unwrap_or_else(|| "out".into()),
    );
    fs::create_dir_all(&out_dir)?;
    let only = std::env::var("SOUNDSCAPE_ONLY").ok();
    let todo: Vec<(usize, fn() -> Track)> = tracks::ALL
        .iter()
        .enumerate()
        .filter(|(_, (slug, _))| only.as_deref().is_none_or(|o| o.split(',').any(|s| s.trim() == *slug)))
        .map(|(i, &(_, f))| (i + 1, f))
        .collect();
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let jobs = std::env::var("SOUNDSCAPE_JOBS").ok().and_then(|j| j.parse().ok()).unwrap_or(cores.min(3)).max(1);

    let started = Instant::now();
    eprintln!("rendering \"{ALBUM}\" — {} track(s), {jobs} at a time", todo.len());
    let next = AtomicUsize::new(0);
    let done = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..jobs.min(todo.len()) {
            scope.spawn(|| loop {
                let k = next.fetch_add(1, Ordering::Relaxed);
                let Some(&(number, render)) = todo.get(k) else { break };
                let result = render_one(number, render, &out_dir);
                done.lock().unwrap().push(result);
            });
        }
    });
    let summaries = done.into_inner().unwrap().into_iter().collect::<std::io::Result<Vec<_>>>()?;
    let listed = write_album(&out_dir, &summaries)?;
    let total: f32 = summaries.iter().map(|s| s.duration).sum();
    eprintln!(
        "done in {:.1}s → {} ({} rendered, {:.0}:{:02.0} of audio; album.json lists {listed})",
        started.elapsed().as_secs_f32(),
        out_dir.display(),
        summaries.len(),
        (total / 60.0).floor(),
        total % 60.0
    );
    Ok(())
}
