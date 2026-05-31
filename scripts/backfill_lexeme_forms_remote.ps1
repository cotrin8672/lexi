# Backfill public.lexeme_forms on Supabase via PostgREST (004-equivalent).
# Requires Lexi admin session in Windows Credential Manager and .env Supabase URL/key.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path $PSScriptRoot -Parent
Set-Location $repoRoot

$sessionJson = & (Join-Path $PSScriptRoot 'read_supabase_session.ps1')
$session = $sessionJson | ConvertFrom-Json
if (-not $session.accessToken) {
    throw 'Supabase session missing accessToken. Sign in from Lexi first.'
}

$envFile = Join-Path $repoRoot '.env'
if (-not (Test-Path $envFile)) {
    throw '.env not found'
}

$base = ((Get-Content $envFile | Where-Object { $_ -match '^SUPABASE_URL=' }) -replace 'SUPABASE_URL=', '').TrimEnd('/')
$anon = ((Get-Content $envFile | Where-Object { $_ -match '^SUPABASE_(PUBLISHABLE_KEY|ANON_KEY)=' }) -split '=', 2)[1]
if (-not $base -or -not $anon) {
    throw 'SUPABASE_URL and SUPABASE_PUBLISHABLE_KEY required in .env'
}

$headers = @{
    apikey        = $anon
    Authorization = "Bearer $($session.accessToken)"
    'Content-Type' = 'application/json'
}

function Invoke-LexiRest {
    param(
        [string]$Method,
        [string]$Uri,
        $Body = $null,
        [string]$Prefer = $null
    )
    $h = $headers.Clone()
    if ($Prefer) { $h['Prefer'] = $Prefer }
    $params = @{ Method = $Method; Uri = $Uri; Headers = $h }
    if ($null -ne $Body) {
        $params['Body'] = ($Body | ConvertTo-Json -Depth 20 -Compress)
    }
    return Invoke-RestMethod @params
}

Write-Host 'Fetching user_lexemes...'
$lexemes = @()
$offset = 0
$pageSize = 200
do {
    $page = Invoke-LexiRest -Method Get -Uri "$base/rest/v1/user_lexemes?select=id,language,canonical_text,canonical_key&deleted_at=is.null&limit=$pageSize&offset=$offset"
  if ($page) { $lexemes += @($page) }
    $offset += $pageSize
} while ($page -and $page.Count -eq $pageSize)

Write-Host "Lexemes: $($lexemes.Count)"

$canonicalRows = foreach ($lexeme in $lexemes) {
    @{
        lexeme_id  = $lexeme.id
        language   = $lexeme.language
        form_text  = $lexeme.canonical_text
        form_key   = $lexeme.canonical_key
        relation   = 'canonical'
        source     = 'repair'
        confidence = 1.0
    }
}

if ($canonicalRows.Count -gt 0) {
    Write-Host "Upserting $($canonicalRows.Count) canonical forms..."
    Invoke-LexiRest -Method Post `
        -Uri "$base/rest/v1/lexeme_forms?on_conflict=user_id,language,form_key,lexeme_id,relation" `
        -Prefer 'resolution=merge-duplicates' `
        -Body $canonicalRows | Out-Null
}

Write-Host 'Fetching active card_snapshots...'
$cards = @()
$offset = 0
do {
    $page = Invoke-LexiRest -Method Get -Uri "$base/rest/v1/card_snapshots?select=lexeme_id,content&active=eq.true&limit=$pageSize&offset=$offset"
    if ($page) { $cards += @($page) }
    $offset += $pageSize
} while ($page -and $page.Count -eq $pageSize)

Write-Host "Active cards: $($cards.Count)"

$languageByLexeme = @{}
foreach ($lexeme in $lexemes) {
    $languageByLexeme[$lexeme.id] = $lexeme.language
}

$irregularRows = [System.Collections.Generic.List[object]]::new()
$seen = @{}

foreach ($card in $cards) {
    $language = $languageByLexeme[$card.lexeme_id]
    if (-not $language) { continue }
    $inflections = $card.content.inflections
    if (-not $inflections) { continue }
    foreach ($inflection in @($inflections)) {
        $formText = [string]$inflection.form
        if ([string]::IsNullOrWhiteSpace($formText)) { continue }
        $formKey = $formText.Trim().ToLowerInvariant()
        $dedupe = "$($card.lexeme_id)|$formKey|irregular"
        if ($seen.ContainsKey($dedupe)) { continue }
        $seen[$dedupe] = $true
        $irregularRows.Add(@{
            lexeme_id  = $card.lexeme_id
            language   = $language
            form_text  = $formText.Trim()
            form_key   = $formKey
            relation   = 'irregular'
            source     = 'repair'
            confidence = 1.0
        })
    }
}

if ($irregularRows.Count -gt 0) {
    Write-Host "Upserting $($irregularRows.Count) irregular forms..."
    $batchSize = 100
    for ($i = 0; $i -lt $irregularRows.Count; $i += $batchSize) {
        $batch = $irregularRows.GetRange($i, [Math]::Min($batchSize, $irregularRows.Count - $i))
        Invoke-LexiRest -Method Post `
            -Uri "$base/rest/v1/lexeme_forms?on_conflict=user_id,language,form_key,lexeme_id,relation" `
            -Prefer 'resolution=merge-duplicates' `
            -Body $batch | Out-Null
    }
}

Write-Host 'Verifying...'
$countHeaders = $headers.Clone()
$countHeaders['Prefer'] = 'count=exact'
$formsCount = (Invoke-WebRequest -Uri "$base/rest/v1/lexeme_forms?select=id" -Headers $countHeaders).Headers['Content-Range']
Write-Host "lexeme_forms total: $formsCount"

$goForms = Invoke-LexiRest -Method Get -Uri "$base/rest/v1/lexeme_forms?select=form_key,relation&form_key=eq.go"
Write-Host 'form_key=go rows:'
$goForms | Format-Table -AutoSize

$wentForms = Invoke-LexiRest -Method Get -Uri "$base/rest/v1/lexeme_forms?select=form_key,relation,lexeme_id&form_key=eq.went"
Write-Host 'form_key=went rows:'
$wentForms | Format-Table -AutoSize

Write-Host 'Backfill complete.'
