param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $GradleArgs
)

$ErrorActionPreference = "Stop"

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

Push-Location $mobileRoot
try {
    $dotenvCommand = @("run") + $dotenvArgs + @("--", ".\gradlew.bat") + $GradleArgs
    & dotenvx @dotenvCommand
    exit $LASTEXITCODE
} finally {
    Pop-Location
}
