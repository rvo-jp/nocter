$ErrorActionPreference = "Stop"

$Out = "$env:USERPROFILE\.nocter\bin\nocter.exe"
$BinDir = Split-Path -Parent $Out
$SrcDir = "src"

if (!(Test-Path $BinDir)) {
    New-Item -ItemType Directory -Path $BinDir | Out-Null
}

Write-Host "Compiling all C files in $SrcDir..."

$Files = Get-ChildItem -Path $SrcDir -Recurse -Filter *.c | ForEach-Object { $_.FullName }

if (-not $Files) {
    Write-Error "No C source files found in $SrcDir"
    exit 1
}

gcc $Files -o $Out -Wall -O2

Write-Host "Build successful: $Out"

$envKey = "HKCU:\Environment"
$pathValue = (Get-ItemProperty -Path $envKey -Name PATH -ErrorAction SilentlyContinue).PATH

if ($pathValue -and $pathValue -like "*$BinDir*") {
    Write-Host "$BinDir is already in user PATH."
} else {
    Write-Host "Adding $BinDir to user PATH..."
    if ($pathValue) {
        $newPath = "$pathValue;$BinDir"
    } else {
        $newPath = $BinDir
    }
    Set-ItemProperty -Path $envKey -Name PATH -Value $newPath
    Write-Host "Added $BinDir to PATH. Please restart your terminal or log out and back in."
}

