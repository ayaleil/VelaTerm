# Build src-tauri/resources/gitbash, the Git Bash tree tauri.windows.conf.json bundles into the Windows installer.
#
#   min   Bash, the core utilities and terminfo trimmed from PortableGit (~25MB). The default, used by
#         scripts/dev-desktop.sh. Users can download the full tree later from inside the app.
#   full  The whole PortableGit (git, ssh, mingw64) so SSH works out of the box.
#
# PortableGit is pinned by version and SHA-256. Its post-install.bat is not run: its effects are reproduced here
# so the result does not depend on running Bash during the build. The download is cached under
# src-tauri/resources/gitbash-cache, and the tree is assembled in a staging directory that only replaces
# resources/gitbash once it is complete. Usage: fetch-gitbash.ps1 [-Variant min|full]
param(
  [ValidateSet('min', 'full')]
  [string]$Variant = 'min'
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$GitVersion = '2.55.0.windows.2'
$AssetName = 'PortableGit-2.55.0.2-64-bit.7z.exe'
$AssetSha256 = 'b20d42da3afa228e9fa6174480de820282667e799440d655e308f700dfa0d0df'
$AssetUrl = "https://github.com/git-for-windows/git/releases/download/v$GitVersion/$AssetName"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$Resources = Join-Path $RepoRoot 'src-tauri\resources'
$Dest = Join-Path $Resources 'gitbash'
$Cache = Join-Path $Resources 'gitbash-cache'
$Staging = Join-Path $Cache "staging-$Variant"
$Sfx = Join-Path $Cache $AssetName

function Test-Sha256([string]$Path) {
  (Test-Path -LiteralPath $Path) -and ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash -eq $AssetSha256)
}

New-Item -ItemType Directory -Force -Path $Cache | Out-Null

# 1. Download, reusing the cached self-extractor when its hash still matches.
if (Test-Sha256 $Sfx) {
  Write-Host "==> Using cached $AssetName"
} else {
  Write-Host "==> Downloading $AssetUrl"
  [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
  $part = "$Sfx.part"
  Invoke-WebRequest -Uri $AssetUrl -OutFile $part -UseBasicParsing
  $actual = (Get-FileHash -LiteralPath $part -Algorithm SHA256).Hash.ToLower()
  if ($actual -ne $AssetSha256) {
    Remove-Item -LiteralPath $part -Force
    throw "SHA-256 mismatch for $AssetName`n  expected $AssetSha256`n  actual   $actual"
  }
  Move-Item -LiteralPath $part -Destination $Sfx -Force
}

# 2. Self-extract into a clean staging directory. The 7-Zip SFX is a GUI executable, so wait on it explicitly.
if (Test-Path -LiteralPath $Staging) { Remove-Item -LiteralPath $Staging -Recurse -Force }
Write-Host "==> Extracting PortableGit ($Variant)"
$proc = Start-Process -FilePath $Sfx -ArgumentList '-y', "-o`"$Staging`"" -Wait -PassThru -NoNewWindow
if ($proc.ExitCode -ne 0) { throw "PortableGit self-extraction failed with exit code $($proc.ExitCode)" }
if (-not (Test-Path -LiteralPath (Join-Path $Staging 'usr\bin\bash.exe'))) {
  throw "usr\bin\bash.exe not found after extraction; unexpected PortableGit layout"
}

# 3. Reproduce post-install.bat (etc/post-install/*.post) as it leaves an installed tree.
#    06-windows-files copies these from the Windows etc directory, truncating names to 8 characters.
$winEtc = Join-Path $env:SystemRoot 'System32\drivers\etc'
foreach ($name in 'hosts', 'protocols', 'services', 'networks') {
  $source = Join-Path $winEtc ($name.Substring(0, [Math]::Min(8, $name.Length)))
  if (-not (Test-Path -LiteralPath $source)) { throw "$source does not exist; cannot create etc/$name" }
  Copy-Item -LiteralPath $source -Destination (Join-Path $Staging "etc\$name") -Force
}
#    03-mtab symlinks /etc/mtab to /proc/mounts, which a bundle cannot carry. The MSYS runtime reads its mount
#    table itself, so this static file is inert; it is upstream's snapshot, byte for byte, for parity.
$mtab = "D:/cygwin64/bin /usr/bin ntfs binary,auto 1 1`n" +
  "D:/cygwin64/lib /usr/lib ntfs binary,auto 1 1`n" +
  "D:/cygwin64 / ntfs binary,auto 1 1`n" +
  "C: /cygdrive/c ntfs binary,posix=0,user,noumount,auto 1 1`n" +
  "D: /cygdrive/d ntfs binary,posix=0,user,noumount,auto 1 1`n"
[IO.File]::WriteAllText((Join-Path $Staging 'etc\mtab'), $mtab, (New-Object Text.UTF8Encoding $false))
#    13-copy-dlls hardlinks git.exe as the dlls-copied sentinel; the DLLs it links already ship in git-core.
Copy-Item -LiteralPath (Join-Path $Staging 'mingw64\bin\git.exe') `
  -Destination (Join-Path $Staging 'mingw64\libexec\git-core\dlls-copied.exe') -Force
#    99-post-install-cleanup and post-install.bat delete themselves. dev/ and tmp/ only hold empty directories,
#    and the documentation is dropped to keep the installer small.
foreach ($path in 'post-install.bat', 'etc\post-install', 'dev', 'tmp', 'mingw64\share\doc') {
  $full = Join-Path $Staging $path
  if (Test-Path -LiteralPath $full) { Remove-Item -LiteralPath $full -Recurse -Force }
}

# 4. Trim to the min set listed in scripts/gitbash/min-files.txt.
if ($Variant -eq 'min') {
  $keep = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'gitbash\min-files.txt') |
    Where-Object { $_ -and -not $_.StartsWith('#') }
  $keepDirs = @($keep | Where-Object { $_.EndsWith('/') })
  $keepFiles = @{}
  $keep | Where-Object { -not $_.EndsWith('/') } | ForEach-Object { $keepFiles[$_] = $true }
  foreach ($file in Get-ChildItem -LiteralPath $Staging -Recurse -File -Force) {
    $rel = $file.FullName.Substring($Staging.Length + 1).Replace('\', '/')
    $kept = $keepFiles.ContainsKey($rel) -or ($keepDirs | Where-Object { $rel.StartsWith($_) })
    if (-not $kept) { Remove-Item -LiteralPath $file.FullName -Force }
  }
  Get-ChildItem -LiteralPath $Staging -Recurse -Directory -Force |
    Sort-Object { $_.FullName.Length } -Descending |
    Where-Object { -not (Get-ChildItem -LiteralPath $_.FullName -Force) } |
    ForEach-Object { Remove-Item -LiteralPath $_.FullName -Force }
  $missing = @($keepFiles.Keys | Where-Object { -not (Test-Path -LiteralPath (Join-Path $Staging $_)) })
  if ($missing.Count -gt 0) { throw "PortableGit is missing files the min variant needs: $($missing -join ', ')" }
}

# 5. The terminal profile hook and the marker agent/gitbash.rs reads as `<version> <variant> <arch>`.
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'gitbash\zz-vlx-term.sh') `
  -Destination (Join-Path $Staging 'etc\profile.d\zz-vlx-term.sh') -Force
[IO.File]::WriteAllText((Join-Path $Staging 'VLX_GITBASH_VERSION.txt'), "$GitVersion $Variant 64`r`n",
  (New-Object Text.ASCIIEncoding))

# 6. Swap the finished tree into place.
if (Test-Path -LiteralPath $Dest) { Remove-Item -LiteralPath $Dest -Recurse -Force }
Move-Item -LiteralPath $Staging -Destination $Dest
Write-Host "==> Git Bash $GitVersion ($Variant) ready at $Dest"
