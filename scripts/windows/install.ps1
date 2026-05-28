$ErrorActionPreference = "Stop"

$Root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$ManagerDir = Join-Path $Root "apps\codex-manager"

Push-Location $ManagerDir
try {
  if (-not (Test-Path "node_modules")) {
    npm install
  }
  npm run build
} finally {
  Pop-Location
}

Write-Host "Codex Manager build completed." -ForegroundColor Green

