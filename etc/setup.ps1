$ErrorActionPreference = "Stop"

$OWNER = "Scapy47"
$REPO = "Shio"
$BASE_URL = "https://github.com/$OWNER/$REPO/releases/latest/download"
$FILENAME = "shio-Windows-x86_64.exe"
$DOWNLOAD_URL = "$BASE_URL/$FILENAME"

while ($true) {
    $choice = Read-Host "Try shio before installation? (!! Run directly !!) (y/n)"
    switch ($choice.ToLower()) {
        "y" {
            $TmpDir = Join-Path $env:TEMP ([System.IO.Path]::GetRandomFileName())
            New-Item -ItemType Directory -Path $TmpDir | Out-Null
            $TmpFile = Join-Path $TmpDir "shio.exe"

            Write-Host "Downloading..."
            try {
                Invoke-WebRequest -Uri $DOWNLOAD_URL -OutFile $TmpFile
            } catch {
                Write-Host "Download failed"; exit 1
            }

            & $TmpFile @args
            break
        }
        "n" {
            exit
        }
        default {
            Write-Host "Please answer y or n."
            continue
        }
    }
    break
}

$INSTALL_DIR = if ($env:XDG_BIN_HOME) { $env:XDG_BIN_HOME } else { Join-Path $env:USERPROFILE ".local\bin" }
$FINAL_PATH = Join-Path $INSTALL_DIR "shio.exe"

New-Item -ItemType Directory -Path $INSTALL_DIR -Force | Out-Null
Write-Host "Downloading to $FINAL_PATH"

try {
    Invoke-WebRequest -Uri $DOWNLOAD_URL -OutFile $FINAL_PATH
} catch {
    Write-Host "Download failed"; exit 1
}

Write-Host "Installed to $FINAL_PATH"

$currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($currentPath -notlike "*$INSTALL_DIR*") {
    Write-Host ""
    Write-Host "Warning: $INSTALL_DIR is not in your PATH"
    Write-Host "Run the following to add it permanently:"
    Write-Host "  [Environment]::SetEnvironmentVariable('PATH', `"`$env:USERPROFILE\.local\bin;`$env:PATH`", 'User')"
}

Write-Host "Run 'shio --version' to verify."
Write-Host "To enable playback, add to your PowerShell profile (`$PROFILE):"
Write-Host '  $env:SHIO_PLAYER_CMD = "mpv --user-agent={user_agent} --http-header-fields=''Referer: {referer}'' {url}"'
