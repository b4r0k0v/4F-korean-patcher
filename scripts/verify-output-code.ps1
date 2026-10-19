$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
function Read-BytesAt($Stream, [long]$Offset, [int]$Count) {
    $null = $Stream.Seek($Offset, [IO.SeekOrigin]::Begin)
    $buffer = [byte[]]::new($Count)
    $position = 0
    while ($position -lt $Count) { $read = $Stream.Read($buffer,$position,$Count-$position); if ($read -eq 0) { throw 'Unexpected EOF' }; $position += $read }
    return ,$buffer
}
function Align64([long]$Value) { return [long]([Math]::Ceiling($Value/64.0)*64) }
$repoPath = Split-Path -Parent $PSScriptRoot
$zip = [IO.Compression.ZipFile]::OpenRead((Join-Path $repoPath 'dev-temp/SMT4F_KO_FOR_REPACK_v1.1.2.zip'))
try {
    $entryStream = $zip.GetEntry('patch/ExtractedExeFS/code.bin').Open()
    $memory = [IO.MemoryStream]::new()
    try { $entryStream.CopyTo($memory); $expected = [Convert]::ToBase64String($memory.ToArray()) } finally { $entryStream.Dispose(); $memory.Dispose() }
    $outputs = Get-ChildItem -LiteralPath (Join-Path $repoPath 'dev-temp/integration') -File -Recurse | Where-Object { $_.Name -match '^SMT4F-KO-[0-9a-f-]+\.(cia|3ds)$' -and $_.Length -gt 512 }
    foreach ($file in $outputs) {
        $stream = [IO.File]::OpenRead($file.FullName)
        try {
            $header = Read-BytesAt $stream 0 512
            if ($file.Extension -eq '.cia') {
                $ncchOffset = (Align64 ([BitConverter]::ToUInt32($header,0))) + (Align64 ([BitConverter]::ToUInt32($header,8))) + (Align64 ([BitConverter]::ToUInt32($header,12))) + (Align64 ([BitConverter]::ToUInt32($header,16)))
            } else { $ncchOffset = [long][BitConverter]::ToUInt32($header,0x120)*512 }
            $ncch = Read-BytesAt $stream $ncchOffset 512
            if ([Text.Encoding]::ASCII.GetString($ncch,0x100,4) -ne 'NCCH') { throw ('NCCH missing: ' + $file.Name) }
            $exheader = Read-BytesAt $stream ($ncchOffset+512) 64
            if (($exheader[13] -band 1) -ne 0) { throw ('Code compression flag set: ' + $file.Name) }
            $exefsOffset = $ncchOffset + [long][BitConverter]::ToUInt32($ncch,0x1a0)*512
            $exefs = Read-BytesAt $stream $exefsOffset 512
            $found = $false
            foreach ($index in 0..9) {
                $at = $index*16
                $name = [Text.Encoding]::ASCII.GetString($exefs,$at,8).Trim([char]0)
                if ($name -eq '.code') {
                    $offset = [BitConverter]::ToUInt32($exefs,$at+8)
                    $size = [BitConverter]::ToUInt32($exefs,$at+12)
                    $code = Read-BytesAt $stream ($exefsOffset+512+$offset) $size
                    if ([Convert]::ToBase64String($code) -cne $expected) { throw ('Patched code differs: ' + $file.Name) }
                    $found = $true
                }
            }
            if (-not $found) { throw ('Code entry missing: ' + $file.Name) }
            [pscustomobject]@{ File=$file.Name; CodeMatchesPatch=$true; Compressed=$false } | ConvertTo-Json -Compress
        } finally { $stream.Dispose() }
    }
} finally { $zip.Dispose() }
