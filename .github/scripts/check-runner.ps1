$ErrorActionPreference = 'Stop'
$inputs = Get-Content -Raw -LiteralPath "$PSScriptRoot/../build-inputs.json" | ConvertFrom-Json
$expected = $inputs.runners.'windows-2022'
if ($env:ImageVersion -ne $expected -or $env:ImageOS -ne 'win22') {
    throw "Runner image changed: expected win22/$expected, got $env:ImageOS/$env:ImageVersion. Review and update .github/build-inputs.json before building a release."
}
