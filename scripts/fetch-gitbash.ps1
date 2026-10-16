# Build src-tauri/resources/gitbash, the Git Bash tree tauri.windows.conf.json bundles into the Windows installer.
#
#   min   Bash, the core utilities and terminfo trimmed from PortableGit (~25MB). The default, used by
#         scripts/dev-desktop.sh. Users can download the full tree later from inside the app.
#   full  The whole PortableGit (git, ssh, mingw64) so SSH works out of the box.
#
# PortableGit is pinned by version and SHA-256. Its post-install.bat is not run: its effects are reproduced here
# so the result does not depend on the build machine. The download is cached under src-tauri/resources/gitbash-cache,
# and the tree is assembled in a staging directory that only replaces resources/gitbash once it is complete; the
# VLX_GITBASH_VERSION.txt marker is written last, so its presence means the tree is whole.
# Usage: fetch-gitbash.ps1 [-Variant min|full]
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
$CachedSfx = Join-Path $Cache $AssetName

# Stock Windows copies of the files 06-windows-files.post copies from %SystemRoot%\System32\drivers\etc, gzipped
# and base64-encoded so they survive line-ending and whitespace normalization byte for byte. They are embedded
# rather than copied so the bundle never picks up the build machine's own hosts entries.
$WindowsEtc = [ordered]@{
  hosts     = @'
H4sIAAAAAAACCm1SQW7bMBA8N0D+sIAPSYCWleymqX0r0hbtoW0AG+iZoVYWUYorcKnYelsPfVK/kCVlOzJQag8SlzM7M9S/P39ncE/dEOy2iXBt
bqBcLhdv5kWxhO/WBGKqo5wInbq8mEnBprEMUhpYt51D+PpzvVlDbeW1Z6zgcZggN/cPb789QE0Bfllf0Y7PeDLKkI/aeobYILS666zfMlANAtRV
FZAZpUnQEEfwukVW8FmbJtGgj2EAbqh3MhrhN3YRyIP2IPPsk6167cBZj0pG4oTzAEokguucNiLe+qyitkFGGXJ960W8c7QbnaWmoSD4joTeb19E
qdHV2Qjtqww5HZooZex00HHk1REcak7SMfFwJ3KOUX2sKhstee3c8FrGt624Zrjm3jSgc26MNxLdkHglSQyJlqYRJJ6UggQbDo6S+jFy00hn1Feh
p6MmuJpdAQ/tI7mjlC+Cxn2++NVhK6+ymKvbd2opdZc3glCS0kZyEcFwWmKN+mCS/fCE4UQAiw9qfqveL1RZ5O/9f8AjwR6Ms5JATvXyIjsjo91L
yJK9XF2KLP2pjdyCE0s7G0UUfPqxBhsZXZ1MvSrnd6qQpzzwn6hSc7Uqz4ZPms9eNJKTOwMAAA==
'@
  networks  = @'
H4sIAAAAAAACCmWOT2vDMAzF74F8B0Eu22Fekw3alNLLoLfttNvoQXXU1NS2jO2yP59+WtZ0KXsIoyfr/VAFTxw+o+kPGW70LdRt+3AnTwvPRkdO
vM+yEYMqi0oKXg8mwd5YAs0+o/EJPOV3jkfw6Oj+Yk5uRxEchmB8LxGO8JO3rNGOkaTg5Wo/AUaCSJp7b76oA+Oh45yl60gbJ1EBufGYjfSYl2e3
mh6yhokf2DIRvaE1mCgppbbiqpVm58jn9faPCfSBLlgawSLLHHaojwOjbubnsZa9U4JfNYtHVTeNqmfzS8p37P99L8qiLK6IUw30bwJOoXOXAQAA
'@
  protocols = @'
H4sIAAAAAAACCnWUS2/iMBDH70h8h1G57B4SHi1sW1WVKvriAIuAriqtepgmJrFK7Mh2eHz7jh1ikm03ipxkND//Z8Yz6cBY5gfFk9TAj+gn9K+u
zoNBrzeCKY+U1HJtyEPlYbvVoRtWKdew5hsGkRQGudBgUgYTYZgSzECupJGR3GhADTFbc8FieD/AFhWXhbZbLB7HOgRYMgapMfl1t7vb7UKOAkOp
ki5qzRORMWF0t9otEEX2zpSGYxSPUmVoro9fN5UbCMzYLcBNuQcplxyZ/uKGo2Y6DMM3oM/OTSQzK3L71m61WzyH6uq5dTKH09X5mh8hUVZB/RIZ
T+ffIbZQioLLmNaYsNoWSeJlz9369DSvqz6hYTs8BEn5rJEm8uTIratxg1wpFDrjVAYpfAAnnJ2EL9360BR+2FPsXCr4qpwXHu0P7Dp/aaDzu8UY
CsG3dFxImhh9NMpWxCf+l11f7hv8i2YKYjSYKMxqXOqrDQN3RM/TBvcstYFMCm6k4iKpkXuhA16qDlzEr7NlMKlUO/DKlNzDbAlka7fUKb6Bi2/R
jO9MMeqkdxqAKsizelPk29HR9aJf9pE3fNtHzqHkAiULw+DivOKChTNYzr7ZrFKGMdVnTUdTA9cKExK88OCjM1jQvtk2/0oy7RMduoI+LJs9ICLM
dbFBJ6xZVChuqBPwsJEYt1uYeuehS/XuuTEydwX9GIThEW1ATVjqHwMuZ2d46QOuZqdTTtE/+Qkp9gaGV959JmdksO4zCYLtzX8KI3OjaUJ6Hvzt
DBa8Z5oSK2MjN3roGq22sZ8vd36LP/f15KaTFSiWSTqeLVemoE6Puf74BHR/jKBOBQAA
'@
  services  = @'
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
HTq140QAAA==
'@
}

function Get-Sha256([string]$Path) {
  (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLower()
}

function Write-GzipBase64([string]$Path, [string]$Data) {
  $bytes = [Convert]::FromBase64String(($Data -replace '\s', ''))
  $packed = New-Object IO.MemoryStream (, $bytes)
  $gzip = New-Object IO.Compression.GZipStream ($packed, [IO.Compression.CompressionMode]::Decompress)
  $plain = New-Object IO.MemoryStream
  try { $gzip.CopyTo($plain) } finally { $gzip.Dispose() }
  [IO.File]::WriteAllBytes($Path, $plain.ToArray())
}

New-Item -ItemType Directory -Force -Path $Cache | Out-Null

# 1. Download, reusing the cached self-extractor when its hash still matches.
if ((Test-Path -LiteralPath $CachedSfx) -and ((Get-Sha256 $CachedSfx) -eq $AssetSha256)) {
  Write-Host "==> Using cached $AssetName"
} else {
  [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
  $part = "$CachedSfx.part"
  for ($attempt = 1; ; $attempt++) {
    if (Test-Path -LiteralPath $part) { Remove-Item -LiteralPath $part -Force }
    Write-Host "==> Downloading $AssetUrl (attempt $attempt of 3)"
    try {
      Invoke-WebRequest -Uri $AssetUrl -OutFile $part -UseBasicParsing
      break
    } catch {
      if ($attempt -ge 3) { throw "Downloading $AssetName failed after 3 attempts: $_" }
      Write-Host "    $_; retrying in $(5 * $attempt)s"
      Start-Sleep -Seconds (5 * $attempt)
    }
  }
  $actual = Get-Sha256 $part
  if ($actual -ne $AssetSha256) {
    Remove-Item -LiteralPath $part -Force
    throw "SHA-256 mismatch for $AssetName`n  expected $AssetSha256`n  actual   $actual"
  }
  Move-Item -LiteralPath $part -Destination $CachedSfx -Force
}

# 2. Self-extract into a clean staging directory. The executable that runs is a private copy whose hash is checked
#    after copying, so nothing can swap the cached file between the check and the run. The 7-Zip SFX is a GUI
#    executable, so wait on it explicitly.
if (Test-Path -LiteralPath $Staging) { Remove-Item -LiteralPath $Staging -Recurse -Force }
$runDir = Join-Path ([IO.Path]::GetTempPath()) "vlx-gitbash-$([Guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $runDir | Out-Null
try {
  $sfx = Join-Path $runDir $AssetName
  Copy-Item -LiteralPath $CachedSfx -Destination $sfx
  if ((Get-Sha256 $sfx) -ne $AssetSha256) { throw "SHA-256 mismatch for the copy of $AssetName at $sfx" }
  Write-Host "==> Extracting PortableGit ($Variant)"
  $proc = Start-Process -FilePath $sfx -ArgumentList '-y', "-o`"$Staging`"" -Wait -PassThru -NoNewWindow
  if ($proc.ExitCode -ne 0) { throw "PortableGit self-extraction failed with exit code $($proc.ExitCode)" }
} finally {
  Remove-Item -LiteralPath $runDir -Recurse -Force -ErrorAction SilentlyContinue
}
if (-not (Test-Path -LiteralPath (Join-Path $Staging 'usr\bin\bash.exe'))) {
  throw "usr\bin\bash.exe not found after extraction; unexpected PortableGit layout"
}

# 3. Reproduce post-install.bat (etc/post-install/*.post) as it leaves an installed tree.
#    06-windows-files copies the Windows etc files; the stock copies above stand in for them.
foreach ($name in $WindowsEtc.Keys) {
  Write-GzipBase64 (Join-Path $Staging "etc\$name") $WindowsEtc[$name]
}
#    03-mtab symlinks /etc/mtab to /proc/mounts, which a bundle cannot carry. The MSYS runtime reads its mount
#    table itself, so this static file is inert; it is upstream's snapshot, byte for byte, for parity.
$mtab = "D:/cygwin64/bin /usr/bin ntfs binary,auto 1 1`n" +
  "D:/cygwin64/lib /usr/lib ntfs binary,auto 1 1`n" +
  "D:/cygwin64 / ntfs binary,auto 1 1`n" +
  "C: /cygdrive/c ntfs binary,posix=0,user,noumount,auto 1 1`n" +
  "D: /cygdrive/d ntfs binary,posix=0,user,noumount,auto 1 1`n"
#    PortableGit ships etc/mtab as an MSYS symlink, a file with the System attribute that WriteAllText refuses
#    to overwrite, so remove it first.
$mtabPath = Join-Path $Staging 'etc\mtab'
if (Test-Path -LiteralPath $mtabPath) { Remove-Item -LiteralPath $mtabPath -Force }
[IO.File]::WriteAllText($mtabPath, $mtab, (New-Object Text.UTF8Encoding $false))
#    13-copy-dlls hardlinks git.exe as the dlls-copied sentinel; the DLLs it links already ship in git-core.
#    PortableGit may already ship the sentinel as a hardlink to git.exe, which Copy-Item rejects as copying a
#    file onto itself, so remove it first; removing the link leaves git.exe intact.
$sentinel = Join-Path $Staging 'mingw64\libexec\git-core\dlls-copied.exe'
if (Test-Path -LiteralPath $sentinel) { Remove-Item -LiteralPath $sentinel -Force }
Copy-Item -LiteralPath (Join-Path $Staging 'mingw64\bin\git.exe') -Destination $sentinel -Force
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

# 5. The terminal profile hook, then the marker agent/gitbash.rs reads as `<version> <variant> <arch>`. The marker
#    goes last: scripts/dev-desktop.sh treats a tree without it as incomplete.
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'gitbash\zz-vlx-term.sh') `
  -Destination (Join-Path $Staging 'etc\profile.d\zz-vlx-term.sh') -Force
[IO.File]::WriteAllText((Join-Path $Staging 'VLX_GITBASH_VERSION.txt'), "$GitVersion $Variant 64`r`n",
  (New-Object Text.ASCIIEncoding))

# 6. Swap the finished tree into place, keeping the previous one until the new one has moved in.
$old = $null
if (Test-Path -LiteralPath $Dest) {
  $old = Join-Path $Cache "gitbash.old-$([Guid]::NewGuid().ToString('N'))"
  Move-Item -LiteralPath $Dest -Destination $old
}
try {
  Move-Item -LiteralPath $Staging -Destination $Dest
} catch {
  if ($old) { Move-Item -LiteralPath $old -Destination $Dest }
  throw "Could not move the new Git Bash into $Dest; the previous tree was restored. $_"
}
if ($old) { Remove-Item -LiteralPath $old -Recurse -Force }
Write-Host "==> Git Bash $GitVersion ($Variant) ready at $Dest"
