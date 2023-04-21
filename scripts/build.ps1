
[Diagnostics.CodeAnalysis.SuppressMessageAttribute('PSAvoidUsingCmdletAliases', '')]
[Diagnostics.CodeAnalysis.SuppressMessageAttribute('PSUseDeclaredVarsMoreThanAssignments', '')]

param()

$current = $PSScriptRoot
. "${current}/color.ps1"

$redCode   = '#FD1C35'
$greenCode = '#92B55F'
$textCode  = '#969696'
$yellowCode= '#E8DA5E'

$buildSource = 'term'

function output() {
  param([string]$text, [switch]$isError)

  if ($source -eq 'term'){ 
    Write-HostColor $text -ForegroundColor $($isError ? $redCode : $greenCode)
  }
  else { Write-Host $text }
}

# --| Build ----------------------
# --|-----------------------------
function RunBuild() {
  param(
    [string]$source
  )
  $buildSource = $source

  $canContinue = $true

  try { $output = $(cargo build 2>&1); $canContinue = $? }
  catch { output "Build Failed: ${_}" -isError; $canContinue = $false }

  $errorLines = $output | Select-String -Pattern "error: " -AllMatches -CaseSensitive | Select-Object 

  if($errorLines.Count -gt 0){
    $errorLines | ForEach-Object { output $_ -isError }
    $canContinue = $false
  }

  if ($canContinue) { output "Compilation Successful" }
  else { output "Failed to compile!" -isError; exit 1 }

}
