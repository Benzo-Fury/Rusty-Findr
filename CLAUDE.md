# Rusty-Findr

A Rust rewrite of [Findr](../Findr), a self-hosted media acquisition tool that automatically downloads, sterilizes, and organizes movies and TV shows for Plex.

## What it does

Findr automates the full pipeline from an IMDb ID to a clean, properly-named file in your media library:

1. **Search** — Query torrent indexers (YTS for movies, EZTV for TV) for available torrents
2. **Rank** — Score torrents on resolution, codec, seeders, release type, file size, uploader reputation, and recency
3. **Sterilize** — Download via qBittorrent, then re-encode through ffmpeg (hardware-accelerated) to strip metadata and normalize to H.264/AAC
4. **Save** — Fetch proper titles from TMDB, apply naming templates, and move files into the Plex library structure

## Why Rust

The original Findr is a TypeScript/Bun monorepo (Hono API + React SPA + shared packages). Rust is a better fit because:

- **Single binary distribution** — no runtime, no node_modules, just one executable
- **Self-updating** — easier to implement in-place binary updates
- **Systems work** — Findr manages filesystems and interacts with torrent networks; Rust handles this natively and safely
- **Performance** — concurrent I/O, ffmpeg orchestration, and file operations benefit from Rust's async model

## Key features to port

- Web UI (API + frontend) for browsing, searching, and monitoring downloads
- Job queue with concurrent processing and resume-on-restart
- Smart torrent ranking algorithm (7-dimension weighted scoring)
- ffmpeg sterilization with hardware encoder auto-detection
- Plex-ready file organization with TMDB metadata
- Authentication
- Signup support (configurable enable/disable)