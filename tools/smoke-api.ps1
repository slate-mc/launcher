[CmdletBinding()]
param(
    [ValidatePattern('^https://')]
    [string] $BaseUrl = 'https://api.slatemc.app'
)

$ErrorActionPreference = 'Stop'
$origin = $BaseUrl.TrimEnd('/')
foreach ($path in @(
    '/health',
    '/ready',
    '/v1/providers',
    '/v1/minecraft/versions',
    '/v1/modpacks?provider=modrinth&q=adventure&limit=2',
    '/v1/mods?provider=modrinth&q=sodium&minecraft_version=1.21.1&loader=fabric&limit=2',
    '/v1/launcher/config'
)) {
    $response = Invoke-RestMethod -Uri "$origin$path" -TimeoutSec 60
    if (-not $response.success) {
        throw "API request failed: $path"
    }
    if ($path -eq '/health' -and $response.data.status -ne 'ok') {
        throw 'The API process is unhealthy.'
    }
    if ($path -eq '/ready' -and $response.data.status -ne 'ok') {
        throw 'The API provider upstream is degraded.'
    }
    Write-Output "PASS $path"
}
