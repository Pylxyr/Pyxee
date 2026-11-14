use crate::dto::{
    state_dto, AddResultDto, CommandDto, ExploreShelfDto, FilterInfoDto, GenreDto, HistoryItemDto,
    HomeDto, LibrarySnapshotDto, LogInfoDto, PerfReportDto, PlayerStateDto, PlaylistDto,
    PlaylistSnapshotDto, RemovedEntryDto, SearchErrorDto, SearchFilterDto, SearchItemDto,
    SearchResponseDto, TrackDto, WindowInfoDto,
};
use crate::sim::catalog;
use crate::state::{now_ms, publish, with_library, AppState};
use tauri::{AppHandle, State};

#[tauri::command]
pub fn dispatch(app: AppHandle, command: CommandDto) {
    publish(&app, |p| p.command(command.into()));
}

#[tauri::command]
pub fn get_state(state: State<AppState>) -> Result<PlayerStateDto, String> {
    let p = state
        .player
        .lock()
        .map_err(|_| "player state unavailable".to_string())?;
    let mut dto = state_dto(&p);
    dto.seq = state.seq.load(std::sync::atomic::Ordering::SeqCst);
    Ok(dto)
}

#[tauri::command]
pub fn window_info(state: State<AppState>) -> WindowInfoDto {
    WindowInfoDto {
        backdrop: state.backdrop.clone(),
    }
}

/// Demo tracks, only when running against the simulated engine (`PYXEE_SIM=1`); empty otherwise,
/// since demo keys cannot be resolved by the real sources.
#[tauri::command]
pub fn dev_catalog() -> Vec<TrackDto> {
    if std::env::var_os("PYXEE_SIM").is_some() {
        catalog().iter().map(TrackDto::from).collect()
    } else {
        Vec::new()
    }
}

/// Searches the music source. Runs on a worker thread because yt-dlp can take seconds.
#[tauri::command]
pub async fn search(
    state: State<'_, AppState>,
    query: String,
    filter: SearchFilterDto,
) -> Result<SearchResponseDto, SearchErrorDto> {
    let source = std::sync::Arc::clone(&state.source);
    tauri::async_runtime::spawn_blocking(move || source.search(&query, filter.into()))
        .await
        .map_err(|e| SearchErrorDto {
            kind: crate::dto::SearchErrorKind::Other,
            message: e.to_string(),
        })?
        .map(|items| SearchResponseDto {
            items: items.iter().map(SearchItemDto::from).collect(),
        })
        .map_err(|e| SearchErrorDto::from(&e))
}

/// Live suggestions. Failures are quiet: no suggestions is better than an error while typing.
#[tauri::command]
pub async fn suggest(state: State<'_, AppState>, prefix: String) -> Result<Vec<String>, ()> {
    let source = std::sync::Arc::clone(&state.source);
    Ok(
        tauri::async_runtime::spawn_blocking(move || source.suggest(&prefix).unwrap_or_default())
            .await
            .unwrap_or_default(),
    )
}

/// Songs similar to `key` (a YouTube mix), for Explore and the Related tab. At most `count`, never the seed itself.
/// Like `search` it runs on a worker thread and rejects with a typed error the UI can branch on.
#[tauri::command]
pub async fn related_get(
    state: State<'_, AppState>,
    key: String,
    count: u32,
) -> Result<Vec<TrackDto>, SearchErrorDto> {
    let source = std::sync::Arc::clone(&state.source);
    let count = count.clamp(1, 30) as usize;
    tauri::async_runtime::spawn_blocking(move || {
        pyxee_sources::gate::background(|| source.related(&key, count))
    })
    .await
    .map_err(|e| SearchErrorDto {
        kind: crate::dto::SearchErrorKind::Other,
        message: e.to_string(),
    })?
    .map(|tracks| tracks.iter().map(TrackDto::from).collect())
    .map_err(|e| SearchErrorDto::from(&e))
}

/// The shelves of YouTube Music's Explore page (trending songs, new releases...). Like `related_get` it runs on a worker
/// thread at background priority and rejects with a typed error the UI can branch on.
#[tauri::command]
pub async fn explore_get(
    state: State<'_, AppState>,
) -> Result<Vec<ExploreShelfDto>, SearchErrorDto> {
    let source = std::sync::Arc::clone(&state.source);
    tauri::async_runtime::spawn_blocking(move || {
        pyxee_sources::gate::background(|| source.explore())
    })
    .await
    .map_err(|e| SearchErrorDto {
        kind: crate::dto::SearchErrorKind::Other,
        message: e.to_string(),
    })?
    .map(|shelves| shelves.iter().map(ExploreShelfDto::from).collect())
    .map_err(|e| SearchErrorDto::from(&e))
}

/// YouTube Music's genres and moods (K-Pop, Anime, Chill...), for the chips on Explore. Same worker-thread shape as
/// `explore_get`.
#[tauri::command]
pub async fn genres_get(state: State<'_, AppState>) -> Result<Vec<GenreDto>, SearchErrorDto> {
    let source = std::sync::Arc::clone(&state.source);
    tauri::async_runtime::spawn_blocking(move || {
        pyxee_sources::gate::background(|| source.genres())
    })
    .await
    .map_err(|e| SearchErrorDto {
        kind: crate::dto::SearchErrorKind::Other,
        message: e.to_string(),
    })?
    .map(|genres| genres.iter().map(GenreDto::from).collect())
    .map_err(|e| SearchErrorDto::from(&e))
}

/// The shelves of one genre (`params` from `genres_get`): YouTube Music's playlists and songs for it. The listener
/// clicked a genre, so unlike the automatic loads it is not run at background priority.
#[tauri::command]
pub async fn genre_get(
    state: State<'_, AppState>,
    params: String,
) -> Result<Vec<ExploreShelfDto>, SearchErrorDto> {
    let source = std::sync::Arc::clone(&state.source);
    tauri::async_runtime::spawn_blocking(move || source.genre(&params))
        .await
        .map_err(|e| SearchErrorDto {
            kind: crate::dto::SearchErrorKind::Other,
            message: e.to_string(),
        })?
        .map(|shelves| shelves.iter().map(ExploreShelfDto::from).collect())
        .map_err(|e| SearchErrorDto::from(&e))
}

/// The songs of a YouTube Music playlist, to play. The listener asked for it, so it is not run at background priority.
#[tauri::command]
pub async fn playlist_tracks_get(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<TrackDto>, SearchErrorDto> {
    let source = std::sync::Arc::clone(&state.source);
    tauri::async_runtime::spawn_blocking(move || source.playlist_tracks(&id))
        .await
        .map_err(|e| SearchErrorDto {
            kind: crate::dto::SearchErrorKind::Other,
            message: e.to_string(),
        })?
        .map(|tracks| tracks.iter().map(TrackDto::from).collect())
        .map_err(|e| SearchErrorDto::from(&e))
}

#[tauri::command]
pub fn search_filters(state: State<AppState>) -> Vec<FilterInfoDto> {
    let supported = state.source.supported_filters();
    use pyxee_sources::SearchFilter as F;
    [
        F::All,
        F::Songs,
        F::Videos,
        F::Albums,
        F::Artists,
        F::Playlists,
    ]
    .into_iter()
    .map(|f| FilterInfoDto {
        filter: f.into(),
        supported: supported.contains(&f),
    })
    .collect()
}

// ------------------------------------------------------------------ library

fn read_library<T>(
    state: &State<AppState>,
    f: impl FnOnce(&crate::library_service::LibraryService) -> Result<T, String>,
) -> Result<T, String> {
    let lib = state
        .library
        .lock()
        .map_err(|_| "The library is unavailable.".to_string())?;
    f(&lib)
}

#[tauri::command]
pub fn get_library(state: State<AppState>) -> Result<LibrarySnapshotDto, String> {
    read_library(&state, |l| l.snapshot())
}

#[tauri::command]
pub fn library_set_liked(app: AppHandle, track: TrackDto, liked: bool) -> Result<(), String> {
    with_library(&app, |l| l.set_liked(&track, liked, now_ms()))
}

#[tauri::command]
pub fn liked_get(state: State<AppState>) -> Result<Vec<TrackDto>, String> {
    read_library(&state, |l| l.liked())
}

#[tauri::command]
pub fn playlist_create(app: AppHandle, name: String, tracks: Vec<TrackDto>) -> Result<i64, String> {
    with_library(&app, |l| l.create_playlist(&name, &tracks, now_ms()))
}

#[tauri::command]
pub fn playlist_rename(app: AppHandle, id: i64, name: String) -> Result<(), String> {
    with_library(&app, |l| l.rename_playlist(id, &name, now_ms()))
}

#[tauri::command]
pub fn playlist_delete(app: AppHandle, id: i64) -> Result<PlaylistSnapshotDto, String> {
    with_library(&app, |l| l.delete_playlist(id))
}

#[tauri::command]
pub fn playlist_restore(app: AppHandle, snapshot: PlaylistSnapshotDto) -> Result<(), String> {
    with_library(&app, |l| l.restore_playlist(&snapshot))
}

#[tauri::command]
pub fn playlist_add(
    app: AppHandle,
    id: i64,
    tracks: Vec<TrackDto>,
) -> Result<AddResultDto, String> {
    with_library(&app, |l| l.add_to_playlist(id, &tracks, now_ms()))
}

#[tauri::command]
pub fn playlist_get(state: State<AppState>, id: i64) -> Result<PlaylistDto, String> {
    read_library(&state, |l| l.playlist(id))
}

#[tauri::command]
pub fn playlist_remove_entry(app: AppHandle, entry_id: i64) -> Result<RemovedEntryDto, String> {
    with_library(&app, |l| l.remove_entry(entry_id, now_ms()))
}

#[tauri::command]
pub fn playlist_restore_entry(app: AppHandle, removed: RemovedEntryDto) -> Result<i64, String> {
    with_library(&app, |l| l.restore_entry(&removed, now_ms()))
}

#[tauri::command]
pub fn playlist_move_entry(app: AppHandle, entry_id: i64, to: usize) -> Result<(), String> {
    with_library(&app, |l| l.move_entry(entry_id, to, now_ms()))
}

#[tauri::command]
pub fn history_get(state: State<AppState>, limit: usize) -> Result<Vec<HistoryItemDto>, String> {
    read_library(&state, |l| l.history(limit))
}

#[tauri::command]
pub fn history_clear(app: AppHandle) -> Result<Vec<HistoryItemDto>, String> {
    with_library(&app, |l| l.clear_history())
}

#[tauri::command]
pub fn history_restore(app: AppHandle, items: Vec<HistoryItemDto>) -> Result<(), String> {
    with_library(&app, |l| l.restore_history(&items))
}

#[tauri::command]
pub fn home_get(state: State<AppState>) -> Result<HomeDto, String> {
    read_library(&state, |l| l.home(now_ms()))
}

/// Output volume, 0-100. Takes effect within the next audio buffer, ramped so it never clicks.
#[tauri::command]
pub fn set_volume(state: State<AppState>, percent: u8) {
    state.volume.set_percent(percent);
}

/// Whether the app was started with `PYXEE_PERF=1` (the window then reports what it does; see `perf_log`).
pub(crate) fn perf_on() -> bool {
    std::env::var("PYXEE_PERF")
        .ok()
        .is_some_and(|v| pyxee_sources::ytmusic::parse_switch(&v) == Ok(true))
}

/// One line for the performance log, only when `PYXEE_PERF=1`: what the app side did (a window put to sleep or woken,
/// memory handed back), so it can be read next to the page's own `perf:` lines.
pub(crate) fn perf_note(what: &str) {
    if perf_on() {
        log::info!("perf: {what}");
    }
}

#[tauri::command]
pub fn perf_enabled() -> bool {
    perf_on()
}

/// The system the app runs on (`windows`, `linux` or `macos`), so the window can say "Windows" only where it is true.
#[tauri::command]
pub fn platform() -> &'static str {
    std::env::consts::OS
}

/// How the seek bar fills in (`PYXEE_SEEK_MODE`): `step` (once a second, the default), `pixel`, `glide` or `steps`.
/// Anything else is `step`; the window decides what each mode does.
#[tauri::command]
pub fn seek_mode() -> String {
    seek_mode_from(std::env::var("PYXEE_SEEK_MODE").ok().as_deref())
}

pub(crate) fn seek_mode_from(value: Option<&str>) -> String {
    let v = value.unwrap_or("").trim().to_ascii_lowercase();
    match v.as_str() {
        "pixel" | "glide" | "steps" => v,
        _ => "step".into(),
    }
}

/// Writes one interval of the window's own counts to the log. Only when `PYXEE_PERF=1`; each number is capped so a
/// page cannot write more than a line of plain digits.
#[tauri::command]
pub fn perf_log(report: PerfReportDto) {
    if !perf_on() {
        return;
    }
    log::info!("{}", perf_line(&report));
}

/// The log line for a report: digits only. The event counts are capped; the sizes are 32-bit numbers, so at most 10 digits.
fn perf_line(report: &PerfReportDto) -> String {
    let c = |n: u32| n.min(10_000_000);
    format!(
        "perf: {}s state={} position={} other={} dom-changes={} animations={} long-tasks={} long-frames={} hidden={} heap-kb={} heap-total-kb={} dom-nodes={} images={} image-kpx={} canvases={} canvas-kpx={}",
        report.seconds.clamp(1, 3600),
        c(report.state_events),
        c(report.position_events),
        c(report.other_events),
        c(report.dom_mutations),
        c(report.running_animations),
        c(report.long_tasks),
        c(report.long_frames),
        report.hidden,
        report.heap_used_kb,
        report.heap_total_kb,
        report.dom_nodes,
        report.images,
        report.image_kpx,
        report.canvases,
        report.canvas_kpx
    )
}

#[tauri::command]
pub fn get_log_info(state: State<AppState>) -> LogInfoDto {
    LogInfoDto {
        dir: state.log.dir.display().to_string(),
        file: state.log.file.display().to_string(),
        error: state.log_error.clone(),
    }
}

#[tauri::command]
pub fn open_log_folder(state: State<AppState>) -> Result<(), String> {
    crate::logging::open_folder(&state.log.dir)
}

/// Opens the releases page of Pyxee in the default browser (the page's own links do not leave the app window).
#[tauri::command]
pub fn open_release_page() -> Result<(), String> {
    crate::opener::open_url(crate::opener::RELEASES_URL)
}

/// Makes the native window (title bar, Mica backdrop) follow the theme the user picked, not only the
/// Windows setting. `None` follows the system again.
#[tauri::command]
pub fn set_window_theme(window: tauri::WebviewWindow, theme: Option<String>) -> Result<(), String> {
    window
        .set_theme(parse_window_theme(theme.as_deref())?)
        .map_err(|e| e.to_string())
}

/// Colours the window's border (and the Windows title bar and its text) like the cover-tinted theme: `#rrggbb`
/// values, or `None` to give a part back to the system.
#[tauri::command]
pub fn set_window_tint(
    window: tauri::WebviewWindow,
    border: Option<String>,
    caption: Option<String>,
    text: Option<String>,
) -> Result<(), String> {
    let parse = |v: Option<String>| match v {
        None => Ok(None),
        Some(hex) => crate::window::colorref(&hex)
            .map(Some)
            .ok_or_else(|| format!("not a #rrggbb colour: {hex}")),
    };
    crate::window::set_tint(&window, parse(border)?, parse(caption)?, parse(text)?);
    Ok(())
}

fn parse_window_theme(theme: Option<&str>) -> Result<Option<tauri::Theme>, String> {
    match theme {
        None => Ok(None),
        Some("dark") => Ok(Some(tauri::Theme::Dark)),
        Some("light") => Ok(Some(tauri::Theme::Light)),
        Some(other) => Err(format!("unknown window theme: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_performance_line_is_plain_numbers_each_capped() {
        let line = perf_line(&PerfReportDto {
            seconds: 0,
            state_events: 2,
            position_events: 10,
            other_events: 1,
            dom_mutations: u32::MAX,
            running_animations: 0,
            long_tasks: 3,
            long_frames: 4,
            hidden: true,
            heap_used_kb: 31_000,
            heap_total_kb: 45_000,
            dom_nodes: 1200,
            images: 14,
            image_kpx: 2300,
            canvases: 1,
            canvas_kpx: 4,
        });
        assert_eq!(
            line,
            "perf: 1s state=2 position=10 other=1 dom-changes=10000000 animations=0 long-tasks=3 long-frames=4 hidden=true heap-kb=31000 heap-total-kb=45000 dom-nodes=1200 images=14 image-kpx=2300 canvases=1 canvas-kpx=4"
        );
        assert!(!line.contains('\n'));
    }

    #[test]
    fn the_window_theme_is_dark_light_or_the_system() {
        assert_eq!(parse_window_theme(None), Ok(None));
        assert_eq!(
            parse_window_theme(Some("dark")),
            Ok(Some(tauri::Theme::Dark))
        );
        assert_eq!(
            parse_window_theme(Some("light")),
            Ok(Some(tauri::Theme::Light))
        );
        assert!(parse_window_theme(Some("neon")).is_err());
    }
}
