@echo off
rem Downloads yt-dlp and ffmpeg (checksum-verified) into crates\app\binaries. Add --update to refresh.
cargo xtask tools %*
