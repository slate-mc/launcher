[CmdletBinding()]
param(
    [Parameter()]
    [ValidatePattern('^https://')]
    [string] $BaseUrl,

    [Parameter()]
    [switch] $UploadSupportReport,

    [Parameter()]
    [switch] $VerifySupportObject
)

$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path -Parent $PSScriptRoot

Push-Location $repositoryRoot
try {
    $summaryJson = cargo run --quiet -p slate-modpack-api -- check-config
    if ($LASTEXITCODE -ne 0) {
        throw 'The modpack API production configuration is invalid.'
    }
    $summary = $summaryJson | ConvertFrom-Json
    if ($summary.environment -ne 'production') {
        throw 'SLATE_DEPLOYMENT_ENVIRONMENT must be production for this validation.'
    }
    foreach ($property in @(
        'sentryConfigured',
        'posthogConfigured',
        'otelConfigured',
        'supportReportStorageConfigured'
    )) {
        if (-not $summary.$property) {
            throw "Production service is not configured: $property"
        }
    }

    if (-not $BaseUrl) {
        Write-Output 'Production configuration is complete. Endpoint drills were not requested.'
        return
    }

    $normalizedBaseUrl = $BaseUrl.TrimEnd('/')
    $health = Invoke-RestMethod -Method Get -Uri "$normalizedBaseUrl/health"
    if (-not $health.success -or $health.data.status -ne 'ok') {
        throw 'The production health endpoint did not report ok.'
    }
    $ready = Invoke-RestMethod -Method Get -Uri "$normalizedBaseUrl/ready"
    if (-not $ready.success -or $ready.data.status -eq 'unavailable') {
        throw 'The production readiness endpoint is unavailable.'
    }

    $event = @{
        installationId = [guid]::NewGuid().ToString()
        event = 'launcher_started'
        appVersion = 'production-validation'
        platform = 'windows'
        architecture = 'x86_64'
        loader = $null
        provider = $null
    } | ConvertTo-Json
    $telemetry = Invoke-RestMethod `
        -Method Post `
        -Uri "$normalizedBaseUrl/v1/telemetry/events" `
        -ContentType 'application/json' `
        -Body $event
    if (-not $telemetry.success -or -not $telemetry.data.accepted) {
        throw 'The PostHog relay did not accept the validation event.'
    }

    if (-not $UploadSupportReport) {
        Write-Output 'Health, readiness, and PostHog relay validation passed.'
        return
    }

    $reportId = [guid]::NewGuid()
    $memory = [System.IO.MemoryStream]::new()
    try {
        $archive = [System.IO.Compression.ZipArchive]::new(
            $memory,
            [System.IO.Compression.ZipArchiveMode]::Create,
            $true
        )
        try {
            $entry = $archive.CreateEntry('validation.txt')
            $writer = [System.IO.StreamWriter]::new($entry.Open())
            try {
                $writer.Write('slate production support storage validation')
            } finally {
                $writer.Dispose()
            }
        } finally {
            $archive.Dispose()
        }
        $headers = @{ 'x-slate-report-id' = $reportId.ToString() }
        $receipt = Invoke-RestMethod `
            -Method Post `
            -Uri "$normalizedBaseUrl/v1/support/reports" `
            -ContentType 'application/zip' `
            -Headers $headers `
            -Body $memory.ToArray()
        if (-not $receipt.success -or $receipt.data.reportId -ne $reportId.ToString()) {
            throw 'The private support-report store did not return the expected receipt.'
        }
    } finally {
        $memory.Dispose()
    }

    if ($VerifySupportObject) {
        if (-not (Get-Command aws -ErrorAction SilentlyContinue)) {
            throw 'AWS CLI is required for object retrieval validation.'
        }
        $now = [DateTime]::UtcNow
        $prefix = if ($env:SLATE_SUPPORT_REPORTS_PREFIX) {
            $env:SLATE_SUPPORT_REPORTS_PREFIX.Trim('/')
        } else {
            'support-reports'
        }
        $key = '{0}/{1:yyyy}/{1:MM}/{1:dd}/{2}.zip' -f $prefix, $now, $reportId
        aws s3api head-object `
            --bucket $env:SLATE_SUPPORT_REPORTS_BUCKET `
            --key $key | Out-Null
        if ($LASTEXITCODE -ne 0) {
            throw 'The uploaded support report could not be retrieved with operator credentials.'
        }
    }

    Write-Output "Production service validation passed. Support report: $reportId"
} finally {
    Pop-Location
}
