# Copy the dev-only libmpv DLLs into the debug target, where the plugin
# searches for them. Re-run after cargo clean or a fresh target directory.
# The DLLs stay gitignored and are never bundled.
$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$source = Join-Path $root "src-tauri\lib"
$target = Join-Path $root "src-tauri\target\debug\lib"
$files = @("libmpv-wrapper.dll", "libmpv-2.dll")

foreach ($file in $files) {
  $path = Join-Path $source $file
  if (!(Test-Path $path)) {
    throw "Missing $path. Put the dev DLLs in src-tauri/lib first."
  }
}

New-Item -ItemType Directory -Force -Path $target | Out-Null
foreach ($file in $files) {
  Copy-Item (Join-Path $source $file) (Join-Path $target $file) -Force
}

Write-Output "Copied libmpv dev DLLs to $target"
