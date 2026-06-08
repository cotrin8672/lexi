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

# Prefer English compiler messages to avoid mojibake in mixed-encoding shells.
if ([string]::IsNullOrWhiteSpace($env:JAVA_TOOL_OPTIONS)) {
    $env:JAVA_TOOL_OPTIONS = "-Duser.language=en -Duser.country=US"
} elseif ($env:JAVA_TOOL_OPTIONS -notmatch "user\.language=") {
    $env:JAVA_TOOL_OPTIONS = "$($env:JAVA_TOOL_OPTIONS) -Duser.language=en -Duser.country=US"
}

function Get-ConfigValue {
    param(
        [string[]] $Names
    )

    foreach ($name in $Names) {
        $value = [Environment]::GetEnvironmentVariable($name)
        if ($value -and $value.Trim()) {
            return $value.Trim()
        }
    }

    return ""
}

$propertyMappings = @(
    @{
        GradleProperty = "LEXI_SUPABASE_URL"
        EnvNames       = @("LEXI_SUPABASE_URL", "SUPABASE_URL")
    },
    @{
        GradleProperty = "SUPABASE_PUBLISHABLE_KEY"
        EnvNames       = @(
            "SUPABASE_PUBLISHABLE_KEY",
            "LEXI_SUPABASE_PUBLISHABLE_KEY",
            "LEXI_SUPABASE_ANON_KEY",
            "SUPABASE_ANON_KEY"
        )
    },
    @{
        GradleProperty = "LEXI_GOOGLE_WEB_CLIENT_ID"
        EnvNames       = @(
            "LEXI_GOOGLE_WEB_CLIENT_ID",
            "GOOGLE_WEB_CLIENT_ID",
            "LEXI_GOOGLE_CLIENT_ID",
            "GOOGLE_CLIENT_ID"
        )
    }
)

$gradlePropArgs = @()
$configStatus = @()
foreach ($mapping in $propertyMappings) {
    $value = Get-ConfigValue -Names $mapping.EnvNames
    if ($value) {
        $gradlePropArgs += "-P$($mapping.GradleProperty)=$value"
        $configStatus += "$($mapping.GradleProperty)=set"
    } else {
        $configStatus += "$($mapping.GradleProperty)=missing"
    }
}

Write-Host ("Mobile build config: " + ($configStatus -join ", "))

& .\gradlew.bat @gradlePropArgs @GradleArgs
exit $LASTEXITCODE
