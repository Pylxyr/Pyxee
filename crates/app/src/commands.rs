use crate::dto::{
    state_dto, AddResultDto, CommandDto, FilterInfoDto, HistoryItemDto, HomeDto,
    LibrarySnapshotDto, LogInfoDto, PlayerStateDto, PlaylistDto, PlaylistSnapshotDto,
    RemovedEntryDto, SearchErrorDto, SearchFilterDto, SearchItemDto, SearchResponseDto, TrackDto,
    WindowInfoDto,
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
    Ok(state_dto(&p))
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
