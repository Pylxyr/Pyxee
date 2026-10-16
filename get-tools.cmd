@echo off
rem Downloads yt-dlp (checksum-verified) into crates\app\binaries. Add --update to refresh.
cargo xtask tools %*
