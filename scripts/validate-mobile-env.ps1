$ErrorActionPreference = "Stop"

$id = $env:LEXI_GOOGLE_WEB_CLIENT_ID
if (-not $id) { $id = $env:GOOGLE_WEB_CLIENT_ID }
if (-not $id) { $id = $env:GOOGLE_CLIENT_ID }

$url = $env:LEXI_SUPABASE_URL
if (-not $url) { $url = $env:SUPABASE_URL }

$key = $env:SUPABASE_PUBLISHABLE_KEY
if (-not $key) { $key = $env:LEXI_SUPABASE_PUBLISHABLE_KEY }

Write-Host "google_id_len=$($id.Length)"
Write-Host "google_id_has_hyphen=$($id -match '-')"
Write-Host "google_id_suffix_ok=$($id -match '\.apps\.googleusercontent\.com$')"
Write-Host "url_ok=$($url -match '^https://[a-z0-9-]+\.supabase\.co/?$')"
Write-Host "key_len=$($key.Length)"
Write-Host "key_prefix=$($key.Substring(0, [Math]::Min(15, $key.Length)))"

if ($id -match '^(\d+)-([a-zA-Z0-9_-]+)\.apps\.googleusercontent\.com$') {
    Write-Host "google_id_format=standard-web-or-android"
} elseif ($id -match '^(\d+)([a-zA-Z0-9_-]+)\.apps\.googleusercontent\.com$') {
    Write-Host "google_id_format=missing-hyphen-suspect"
} else {
    Write-Host "google_id_format=unexpected"
}
