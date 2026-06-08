param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $GradleArgs
)

$ErrorActionPreference = "Stop"

# Keep Gradle/Java output readable in UTF-8 terminals.
$utf8 = [System.Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $utf8
[Console]::InputEncoding = $utf8
$OutputEncoding = $utf8
if ($IsWindows -or $env:OS -like "*Windows*") {
    chcp 65001 | Out-Null
}

$repoRoot = Split-Path -Parent $PSScriptRoot
$mobileRoot = Join-Path $repoRoot "mobile"
$dotenvArgs = @()

$secretEnv = Join-Path $repoRoot ".env.secret"
$defaultEnv = Join-Path $repoRoot ".env"

if (Test-Path -LiteralPath $secretEnv) {
    $dotenvArgs += @("-f", $secretEnv)
}
if (Test-Path -LiteralPath $defaultEnv) {
    $dotenvArgs += @("-f", $defaultEnv)
}

$invokeScript = Join-Path $PSScriptRoot "invoke-mobile-gradle.ps1"

Push-Location $mobileRoot
try {
    $dotenvCommand = @("run") + $dotenvArgs + @(
        "--",
        "powershell",
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        $invokeScript
    ) + $GradleArgs
    & dotenvx @dotenvCommand
    exit $LASTEXITCODE
} finally {
    Pop-Location
}
