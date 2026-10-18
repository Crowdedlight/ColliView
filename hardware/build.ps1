# Export every printed part (STL) and the laser-cut diffuser (DXF + SVG) for
# 2 mm and 3 mm acrylic into hardware\out\.
#
# Usage: .\build.ps1 [-Extra '<openscad -D overrides>']
#   e.g. .\build.ps1 -Extra '-D camera=\"ov9726_hbv\" -D barrel_od=50.7'
# Set $env:OPENSCAD to use an openscad.com that is not on PATH.
param([string]$Extra = "")
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
$openscad = if ($env:OPENSCAD) { $env:OPENSCAD } else { (Get-Command openscad).Source }
New-Item -ItemType Directory -Force out | Out-Null

# Start-Process passes the command line verbatim, which keeps the \"quoted\" OpenSCAD
# string values intact in both Windows PowerShell 5.1 and PowerShell 7.
function Run([string]$cmdline) {
    $p = Start-Process -FilePath $openscad -ArgumentList "$cmdline $Extra colliview_adapter.scad" -NoNewWindow -Wait -PassThru
    if ($p.ExitCode -ne 0) { throw "openscad failed: $cmdline" }
}

foreach ($t in 2, 3) {
    # The barrel's acrylic recess depends on the sheet thickness.
    Run "-o out/barrel_${t}mm.stl -D part=\`"barrel\`" -D acrylic_t=$t"
    Run "-o out/diffuser_${t}mm.dxf -D part=\`"diffuser_2d\`" -D acrylic_t=$t"
    Run "-o out/diffuser_${t}mm.svg -D part=\`"diffuser_2d\`" -D acrylic_t=$t"
}
foreach ($p in "head", "carrier", "baffle", "retainer", "cap", "cap_test", "tolerance_test", "fit_test", "jig") {
    Run "-o out/$p.stl -D part=\`"$p\`""
}
Write-Output "Done: $((Get-ChildItem out).Count) files in $PSScriptRoot\out"
