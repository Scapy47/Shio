<div id="top">
    <div align="center">
        <h1>Shio</h1>
        <p>inspired by <a href="https://github.com/pystardust/ani-cli">ani-cli from pystardust</a></p>
    </div>
</div>

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

Download the files for your os from release and extract it.

```sh
tar -xvzf <file>.tar.gz --one-top-level="temp"
```

```sh
unzip <file>.zip -d "temp"
```

make binary file executable

```sh
chmod u+x shio
```

add binary to `$PATH`. Recommended approach to move the binary to `$XDG_BIN_HOME` or `~/.local/bin` as most systems automatically include this directory in `$PATH`.
Alternatively, you can run the binary directly.
