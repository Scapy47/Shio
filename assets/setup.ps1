$ErrorActionPreference = 'Stop'

$OWNER = "Scapy47"
$REPO = "Shio"
$BASE_URL = "https://github.com/$OWNER/$REPO/releases/latest/download"

# Architecture Check
$ARCH = ""
switch ($env:PROCESSOR_ARCHITECTURE) {
    "AMD64" { $ARCH = "x86_64" }
    "ARM64" { Write-Host "arm64 architecture is currently Unsupported"; exit 1 }
    default { Write-Host "Unsupported architecture"; exit 1 }
}

$FILENAME = "shio-Windows-${ARCH}.exe"

Write-Host -NoNewline "Try shio before installation? (!! Run directly !!) (y/n): "
while ($true) {
    $choice = Read-Host
    switch -Regex ($choice) {
        "^[yY]$" {
            # Create a temporary directory
            $TMP_DIR = Join-Path ([System.IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
            New-Item -ItemType Directory -Path $TMP_DIR -Force | Out-Null
            $TMP_FILE = Join-Path $TMP_DIR "shio.exe"
            
            try {
                Invoke-WebRequest -Uri "$BASE_URL/$FILENAME" -OutFile $TMP_FILE
            } catch {
                Write-Host "Failed to Download $BASE_URL/$FILENAME"
                Remove-Item -Path $TMP_DIR -Recurse -Force
                exit 1
            }
            
            # Execute the temp file
            & $TMP_FILE $args
            
            # Clean up
            Remove-Item -Path $TMP_DIR -Recurse -Force

            Write-Host -NoNewline "Proceed with installation? (y/n): "
            $install_choice = Read-Host
            if ($install_choice -match "^[yY]$") {
                break
            } else {
                exit 0
            }
        }
        "^[nN]$" {
            exit 0
        }
        default {
            Write-Host "Please answer y or n."
            Write-Host -NoNewline "Try shio before installation? (!! Run directly !!) (y/n): "
        }
    }
}

# Install Directory setup
$INSTALL_DIR = if ($env:XDG_BIN_HOME) { $env:XDG_BIN_HOME } else { Join-Path $env:USERPROFILE ".local\bin" }
$FINAL_PATH = Join-Path $INSTALL_DIR "shio.exe"

if (-not (Test-Path -Path $INSTALL_DIR)) {
    New-Item -ItemType Directory -Path $INSTALL_DIR -Force | Out-Null
}

Write-Host "Downloading to $FINAL_PATH"
try {
    Invoke-WebRequest -Uri "$BASE_URL/$FILENAME" -OutFile $FINAL_PATH
} catch {
    Write-Host "Download failed"
    exit 1
}

Write-Host "Installed to $FINAL_PATH"

# Check if INSTALL_DIR is in PATH
$pathArray = $env:PATH -split ';'
if ($pathArray -notcontains $INSTALL_DIR) {
    Write-Host ""
    Write-Host "Warning: $INSTALL_DIR is not in your PATH"
    Write-Host "Add it to your PowerShell profile (e.g., by running 'notepad `$PROFILE'):"
    Write-Host "  `$env:PATH += `";$INSTALL_DIR`""
}

Write-Host ""
Write-Host "Run 'shio --version' to verify."
Write-Host ""
Write-Host "To enable playback, add one of the following to your PowerShell profile:"
Write-Host "  # mpv"
Write-Host '  $env:SHIO_PLAYER_CMD="mpv --user-agent={user_agent} --http-header-fields=\`"Referer: {referer}\`" {url}"'
Write-Host "  # VLC"
Write-Host '  $env:SHIO_PLAYER_CMD="vlc --http-user-agent={user_agent} --http-referrer={referer} {url}"'
