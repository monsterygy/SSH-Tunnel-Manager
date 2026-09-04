# SSH Tunnel Manager - Windows Packaging Script
# Usage: ./scripts/package-windows.ps1 [-SkipBuild] [-CliOnly] [-Archive]

[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [switch]$CliOnly,
    [switch]$Archive
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Split-Path -Parent $ScriptDir
$CargoToml = Join-Path $ProjectDir "Cargo.toml"
$ReleaseDir = Join-Path $ProjectDir "target\release"

$AppName = "SSH Tunnel Manager"
$PackageName = "ssh-tunnel-manager"
$GuiExeName = "$AppName.exe"
$CliExeName = "$PackageName.exe"
$Arch = "x64"

function Get-CargoMetadata {
    $name = $null
    $version = $null
    Get-Content -Path $CargoToml | ForEach-Object {
        if (-not $name -and $_ -match '^name\s*=\s*"([^"]+)"') {
            $name = $Matches[1]
        }
        elseif (-not $version -and $_ -match '^version\s*=\s*"([^"]+)"') {
            $version = $Matches[1]
        }
    }
    if (-not $name -or -not $version) {
        throw "Failed to read package name/version from Cargo.toml"
    }
    return @{ Name = $name; Version = $version }
}

function Invoke-Cargo {
    param([Parameter(Mandatory = $true)][string[]]$CargoArgs)
    & cargo @CargoArgs
    if ($LASTEXITCODE -ne 0) {
        throw "cargo $($CargoArgs -join ' ') failed with exit code $LASTEXITCODE"
    }
}

function Find-InnoSetupCompiler {
    $candidates = @(
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "${env:ProgramFiles}\Inno Setup 6\ISCC.exe"
    )
    foreach ($path in $candidates) {
        if ($path -and (Test-Path $path)) {
            return $path
        }
    }
    $command = Get-Command iscc -ErrorAction SilentlyContinue
    if ($command) {
        return $command.Source
    }
    return $null
}

function Install-InnoSetup {
    $iscc = Find-InnoSetupCompiler
    if ($iscc) {
        return $iscc
    }

    Write-Host "Installing Inno Setup..."
    $installer = Join-Path $env:TEMP "innosetup-installer.exe"
    $urls = @(
        "https://jrsoftware.org/download.php/is.exe",
        "https://jrsoftware.org/download.php/is.exe?site=1"
    )

    $downloaded = $false
    foreach ($url in $urls) {
        try {
            Invoke-WebRequest -Uri $url -OutFile $installer -UseBasicParsing
            $downloaded = $true
            break
        }
        catch {
            Write-Host "  Download failed from $url"
        }
    }

    if (-not $downloaded) {
        throw "Failed to download Inno Setup"
    }

    Start-Process -FilePath $installer -ArgumentList "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/SP-" -Wait
    $iscc = Find-InnoSetupCompiler
    if (-not $iscc) {
        throw "Inno Setup installation finished but ISCC.exe was not found"
    }
    return $iscc
}

$meta = Get-CargoMetadata
$PackageName = $meta.Name
$Version = $meta.Version
$CliExeName = "$PackageName.exe"

Write-Host "Packaging $AppName v$Version"
Write-Host "  Architecture: $Arch"
Write-Host ""

Set-Location $ProjectDir
New-Item -ItemType Directory -Force -Path $ReleaseDir | Out-Null

$CliBinary = Join-Path $ReleaseDir $CliExeName
$GuiBinary = Join-Path $ReleaseDir $GuiExeName

if (-not $SkipBuild) {
    Write-Host "Building CLI..."
    Invoke-Cargo -CargoArgs @("build", "--release")
    Copy-Item -Path $CliBinary -Destination (Join-Path $ReleaseDir "$PackageName-cli.exe") -Force

    if (-not $CliOnly) {
        Write-Host "Building GUI..."
        Invoke-Cargo -CargoArgs @("build", "--release", "--features", "gui")
        Copy-Item -Path $CliBinary -Destination $GuiBinary -Force
        Copy-Item -Path (Join-Path $ReleaseDir "$PackageName-cli.exe") -Destination $CliBinary -Force
    }
}

if (-not (Test-Path $CliBinary)) {
    throw "CLI binary not found: $CliBinary"
}
Write-Host "CLI binary: $CliBinary"

if ($CliOnly) {
    if ($Archive) {
        $cliZip = Join-Path $ReleaseDir "$PackageName-v$Version-windows-$Arch.zip"
        if (Test-Path $cliZip) { Remove-Item $cliZip -Force }
        tar -a -c -f $cliZip -C $ReleaseDir $CliExeName
        if ($LASTEXITCODE -ne 0) { throw "Failed to create CLI archive" }
        Write-Host "Archive: $cliZip"
    }
    exit 0
}

if (-not (Test-Path $GuiBinary)) {
    throw "GUI binary not found: $GuiBinary"
}
Write-Host "GUI binary: $GuiBinary"

$StagingDir = Join-Path $ReleaseDir "windows-package"
if (Test-Path $StagingDir) {
    Remove-Item $StagingDir -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $StagingDir | Out-Null

Copy-Item -Path $GuiBinary -Destination (Join-Path $StagingDir $GuiExeName) -Force
Copy-Item -Path $CliBinary -Destination (Join-Path $StagingDir $CliExeName) -Force

$LicenseFile = Join-Path $ProjectDir "LICENSE"
if (Test-Path $LicenseFile) {
    Copy-Item -Path $LicenseFile -Destination (Join-Path $StagingDir "LICENSE") -Force
}

if ($Archive) {
    Write-Host "Creating portable executable..."
    $PortableExe = Join-Path $ReleaseDir "SSH-Tunnel-Manager-v$Version-windows-$Arch-portable.exe"
    Copy-Item -Path $GuiBinary -Destination $PortableExe -Force
    Write-Host "  Portable: $PortableExe"

    Write-Host "Creating installer..."
    $iscc = Install-InnoSetup
    $iss = Join-Path $ScriptDir "windows-installer.iss"
    $setupName = "SSH-Tunnel-Manager-v$Version-windows-$Arch-setup"
    $setupExe = Join-Path $ReleaseDir "$setupName.exe"
    if (Test-Path $setupExe) { Remove-Item $setupExe -Force }

    $isccArgs = @(
        "/Q",
        "/DAppVersion=$Version",
        "/DSourceDir=$StagingDir",
        "/DOutputDir=$ReleaseDir",
        "/DOutputBaseFilename=$setupName",
        $iss
    )
    & $iscc @isccArgs
    if ($LASTEXITCODE -ne 0) {
        throw "Inno Setup compilation failed with exit code $LASTEXITCODE"
    }
    if (-not (Test-Path $setupExe)) {
        throw "Installer was not created: $setupExe"
    }
    Write-Host "  Installer: $setupExe"
}

$cliSize = (Get-Item $CliBinary).Length
$guiSize = (Get-Item $GuiBinary).Length

Write-Host ""
Write-Host "Packaging complete!"
Write-Host "  CLI:     $CliBinary ($([math]::Round($cliSize / 1MB, 1)) MB)"
Write-Host "  GUI:     $GuiBinary ($([math]::Round($guiSize / 1MB, 1)) MB)"
Write-Host "  Version: $Version"
