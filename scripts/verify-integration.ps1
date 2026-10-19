param([string]$GameDirectory = 'dev-temp/games', [switch]$All, [switch]$DlcOnly)
$ErrorActionPreference = 'Stop'
$repoPath = Split-Path -Parent $PSScriptRoot
$gamePath = (Resolve-Path -LiteralPath (Join-Path $repoPath $GameDirectory)).Path
$testPath = Join-Path $repoPath 'dev-temp/integration'
$koreanPrefix = [string][char]0xD55C + [char]0xAE00
$workPath = Join-Path $testPath ($koreanPrefix + ' work & (test)')
$outputPath = Join-Path $testPath ($koreanPrefix + ' output & (test)')
New-Item -ItemType Directory -Path $workPath,$outputPath -Force | Out-Null
Push-Location $repoPath
try {
    cargo build --release --manifest-path src-tauri/Cargo.toml --example patch
    if ($LASTEXITCODE -ne 0) { throw 'Integration runner build failed' }
    $combinations = @(@{ Format='cia'; Dlc=$false })
    if ($All) { $combinations = @(@{ Format='cia'; Dlc=$false }, @{ Format='threeds'; Dlc=$false }, @{ Format='cia'; Dlc=$true }, @{ Format='threeds'; Dlc=$true }) }
    if ($DlcOnly) { $combinations = @(@{ Format='cia'; Dlc=$true }, @{ Format='threeds'; Dlc=$true }) }
    foreach ($combination in $combinations) {
        $request = @{
            base = Join-Path $gamePath 'base-decrypted.3ds'
            patch1 = Join-Path $repoPath 'dev-temp/SMT4Final 2.0.zip'
            patch2 = Join-Path $repoPath 'dev-temp/SMT4F_KO_FOR_REPACK_v1.1.2.zip'
            dlc = $null; dlcPatch = $null
            toolkit = Join-Path $repoPath 'dev-temp/HackingToolkit3DSv9'
            decryptor = Join-Path $repoPath 'dev-temp/Batch CIA 3DS Decryptor'
            work = $workPath; output = $outputPath; format = $combination.Format
        }
        if ($combination.Dlc) { $request.dlc = Join-Path $gamePath 'dlc.cia'; $request.dlcPatch = Join-Path $repoPath 'dev-temp/SMT4F_KO_DLC_Patch_v1.1.2.zip' }
        $tag = $combination.Format + '-' + $combination.Dlc
        $requestPath = Join-Path $testPath ($tag + '.json')
        [IO.File]::WriteAllText($requestPath, ($request | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
        & (Join-Path $repoPath 'src-tauri/target/release/examples/patch.exe') $requestPath
        if ($LASTEXITCODE -ne 0) { throw "Integration failed: $tag. Workspace and logs retained." }
    }
} finally { Pop-Location }
