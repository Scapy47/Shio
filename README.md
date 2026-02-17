<div id="top">
    <div align="center">
        <h1>Shio</h1>
        <p>inspired by <a href="https://github.com/pystardust/ani-cli">ani-cli from pystardust</a></p>
    </div>
</div>

[![blazingly fast](https://www.blazingly.fast/api/badge.svg?repo=Scapy47%2FShio)](https://www.blazingly.fast)

## Quick Links

- [intro](#Introduction)
- [Quickstart](#Getting-started)

## Introduction

**Shio** is a blazingly fast command-line TUI anime search and browser application. that allows you to discover, browse, and watch anime directly from your terminal — no browser required.

### Features

- 🔍 Search for anime instantly
- 📺 Browse titles through an interactive TUI
- ▶️ Stream and watch episodes from the command line
- 🎬 Supports any video player with a command-line interface (CLI)
- ⚡ Fast, lightweight, and keyboard-driven experience


Built for those who want a seamless experience without ever leaving the terminal.

## Getting started

### Setup player

```sh
export SHIO_PLAYER_CMD="mpv --user-agent={user_agent} --http-header-fields='Referer: {referer}' {url}"
```

> [!NOTE]
> some sources require `{user_agent}` and `{referer}` to work properly.
> If your player supports custom header configuration, it is recommended to set these up.
> otherwise some sources may not work.

### Installation

Download the binary for your OS from the [Releases](https://github.com/scapy_47/shio/releases) page.

**Linux / macOS**

1.  Rename the downloaded file (e.g., `shio-Linux-x86_64`) to `shio`.
2.  Make it executable.
3.  Move it to your `$PATH` (e.g., `~/.local/bin`).

```sh
# 1. Rename (Replace filename with the one you downloaded)
mv shio-Linux-x86_64 shio

# 2. Make executable
chmod u+x shio

# 3. Move to path
mkdir -p ~/.local/bin
mv shio ~/.local/bin/
```

**Windows**

1. Download shio-Windows-x86_64.exe.
2. Rename it to shio.exe.
3. Move it to a folder in your System PATH or run it directly from PowerShell/CMD.

