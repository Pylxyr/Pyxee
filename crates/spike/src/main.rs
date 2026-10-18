//! Milestone 1 spike. Two subcommands:
//!   spike files <a> <b>            offline: decode two local files through the gapless path, report gaps
//!   spike play  "<query>" "<query>" resolve with yt-dlp and play track 1 gapless into track 2 on the default device
//! Optional: --ytdlp <path> (default: yt-dlp on PATH). Audio is decoded in-process by libav (no ffmpeg process).

mod player;

use anyhow::{bail, Context, Result};
use pyxee_audio::decode::{LibavSource, SampleSource};
use pyxee_audio::gap::find_gaps;
use pyxee_audio::gapless::{Event, GaplessSource, PreparedTrack};
use pyxee_audio::http::HttpRangeSource;
use std::path::{Path, PathBuf};
use std::time::Instant;

struct Opts {
    ytdlp: PathBuf,
    filter: String,
    positional: Vec<String>,
}

fn parse_opts(args: impl Iterator<Item = String>) -> Opts {
    let mut o = Opts {
        ytdlp: "yt-dlp".into(),
        filter: "all".into(),
        positional: Vec::new(),
    };
    let mut it = args.peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--ytdlp" => o.ytdlp = it.next().unwrap_or_default().into(),
            "--filter" => o.filter = it.next().unwrap_or_default(),
            _ => o.positional.push(a),
        }
    }
    o
}

/// Opens a stream URL with the in-process libav decoder. A format it cannot read is an error.
pub fn open_stream(
    url: &str,
    headers: &[(String, String)],
) -> Result<(Box<dyn SampleSource>, &'static str)> {
    let src = HttpRangeSource::open_with(url, headers)?;
    let s = LibavSource::open(Box::new(src))?;
    Ok((Box::new(s), "libav"))
}

fn files_report(a: &Path, b: &Path) -> Result<()> {
    const RATE: u32 = 48_000;
    let open =
        |p: &Path| -> Result<Box<dyn SampleSource>> { Ok(Box::new(LibavSource::open_path(p)?)) };
    let t = Instant::now();
    let mut g = GaplessSource::new();
    g.push(PreparedTrack::new(1, open(a)?, RATE)?);
    g.push(PreparedTrack::new(2, open(b)?, RATE)?);
    println!("both tracks opened and pre-decoded in {:?}", t.elapsed());
    let mut all = Vec::new();
    let mut buf = vec![0f32; 8192];
    loop {
        let n = g.fill(&mut buf);
        all.extend_from_slice(&buf[..n]);
        if n < buf.len() {
            break;
        }
    }
    let events = g.drain_events();
    let gaps = find_gaps(&all, 2, 1e-3, 8);
    println!("decoded {} frames in {:?}", all.len() / 2, t.elapsed());
    for e in &events {
        println!("  event: {e:?}");
    }
    if gaps.is_empty() {
        println!("RESULT: no silence found in the joined stream");
    } else {
        println!("RESULT: {} silent run(s): {gaps:?}", gaps.len());
    }
    Ok(())
}

fn play_report(qa: &str, qb: &str, o: &Opts) -> Result<()> {
    let t0 = Instant::now();
    let mut g = GaplessSource::new();

    println!("resolving track 1: {qa}");
    let r1 = pyxee_sources::ytdlp::resolve_query(&o.ytdlp, qa)?;
    println!(
        "  {} - {:?} | {} {:?} @ {:?} kbps | resolved in {:?}",
        r1.title,
        r1.uploader,
        r1.ext,
        r1.acodec,
        r1.abr_kbps,
        t0.elapsed()
    );

    let rate = player::default_rate()?;
    let t1 = Instant::now();
    let (s1, path1) = open_stream(&r1.stream_url, &r1.headers)?;
    g.push(PreparedTrack::new(1, s1, rate)?);
    println!(
        "  track 1 opened via {path1} and pre-decoded in {:?}",
        t1.elapsed()
    );

    let mut p = player::Player::start(g, rate)?;
    println!(
        "playing; total time from start to audio: {:?}",
        t0.elapsed()
    );

    println!("resolving track 2 while track 1 plays: {qb}");
    let t2 = Instant::now();
    let r2 = pyxee_sources::ytdlp::resolve_query(&o.ytdlp, qb)?;
    println!(
        "  {} - {:?} | {} {:?} @ {:?} kbps",
        r2.title, r2.uploader, r2.ext, r2.acodec, r2.abr_kbps
    );
    let (s2, path2) = open_stream(&r2.stream_url, &r2.headers)?;
    let prepared = PreparedTrack::new(2, s2, rate)?;
    let played = p.played_secs();
    p.push(prepared);
    p.close();
    println!(
        "  track 2 ready via {path2} in {:?}; track 1 had played {played:.1}s of {:?}s",
        t2.elapsed(),
        r1.duration_secs
    );

    p.wait_until_done();
    let rep = p.report();
    println!("\n===== REPORT =====");
    println!(
        "frames produced: {} | device underruns (silence inserted by callback): {}",
        rep.frames, rep.underruns
    );
    println!("engine events: {:?}", rep.events);
    println!("silent runs in produced stream: {:?}", rep.gaps);
    let boundary = rep.events.iter().find_map(|e| match e {
        Event::Finished {
            id: 1, at_frame, ..
        } => Some(*at_frame as usize),
        _ => None,
    });
    if let Some(b) = boundary {
        let near: Vec<_> = rep
            .gaps
            .iter()
            .filter(|g| g.start_frame.abs_diff(b) < rate as usize / 2)
            .collect();
        println!("boundary at frame {b}; silent runs within 500 ms of it: {near:?}");
    }
    Ok(())
}

/// Prints what the live search returns, with timings, so parsing can be checked against reality.
fn search_report(query: &str, o: &Opts) -> Result<()> {
    use pyxee_sources::{ytdlp::YtDlpSource, MusicSource, SearchFilter, SearchItem};
    let filter = match o.filter.as_str() {
        "songs" => SearchFilter::Songs,
        "videos" => SearchFilter::Videos,
        "albums" => SearchFilter::Albums,
        "artists" => SearchFilter::Artists,
        "playlists" => SearchFilter::Playlists,
        _ => SearchFilter::All,
    };
    let src = YtDlpSource::new(&o.ytdlp);
    let t = Instant::now();
    let items = src.search(query, filter)?;
    println!(
        "{} result(s) for {query:?} [{}] in {:?}",
        items.len(),
        o.filter,
        t.elapsed()
    );
    for (i, item) in items.iter().enumerate() {
        match item {
            SearchItem::Track(t) => println!(
                "{i:>2}. track    {} | {} | {:?} ms | {} | thumb: {}",
                t.title,
                t.artist,
                t.duration_ms,
                t.key,
                t.thumbnail.is_some()
            ),
            SearchItem::Playlist {
                title,
                uploader,
                key,
                ..
            } => println!("{i:>2}. playlist {title} | {uploader} | {key}"),
            SearchItem::Artist { name, key, .. } => println!("{i:>2}. artist   {name} | {key}"),
        }
    }
    let t = Instant::now();
    let s = src.suggest(query)?;
    println!("suggestions in {:?}: {s:?}", t.elapsed());
    Ok(())
}

/// Prints the autoplay-radio list for a track key.
fn related_report(key: &str, o: &Opts) -> Result<()> {
    use pyxee_sources::{ytdlp::YtDlpSource, MusicSource};
    let t = Instant::now();
    let list = YtDlpSource::new(&o.ytdlp).related(key, 15)?;
    println!(
        "{} related track(s) for {key} in {:?}",
        list.len(),
        t.elapsed()
    );
    for (i, t) in list.iter().enumerate() {
        println!(
            "{i:>2}. {} | {} | {:?} ms | {}",
            t.title, t.artist, t.duration_ms, t.key
        );
    }
    Ok(())
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_default();
    let o = parse_opts(args);
    match (cmd.as_str(), o.positional.as_slice()) {
        ("files", [a, b]) => files_report(Path::new(a), Path::new(b)),
        ("play", [qa, qb]) => play_report(qa, qb, &o).context("play spike failed"),
        ("search", [q]) => search_report(q, &o),
        ("related", [key]) => related_report(key, &o),
        _ => bail!("usage: spike search \"<query>\" [--filter all|songs|videos|artists|playlists] | spike related yt:<id> | spike files <a> <b> | spike play \"<query 1>\" \"<query 2>\"  [--ytdlp path]"),
    }
}
