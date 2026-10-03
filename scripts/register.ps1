$ErrorActionPreference = "Stop"

$clsid = "{cedc333b-8783-4e6e-9bf5-45310d7bffae}"
$dllPath = (Resolve-Path "..\crates\ime_tsf\target\debug\ime_tsf.dll" -ErrorAction SilentlyContinue)

if (-not $dllPath) {
    $dllPath = (Resolve-Path "..\target\debug\ime_tsf.dll").Path
} else {
    $dllPath = $dllPath.Path
}

New-Item -Path "HKCU:\Software\Classes\CLSID\$clsid" -Force | Out-Null
New-ItemProperty -Path "HKCU:\Software\Classes\CLSID\$clsid" -Name "(Default)" -Value "Samoyed IME" -Force | Out-Null

New-Item -Path "HKCU:\Software\Classes\CLSID\$clsid\InprocServer32" -Force | Out-Null
New-ItemProperty -Path "HKCU:\Software\Classes\CLSID\$clsid\InprocServer32" -Name "(Default)" -Value $dllPath -Force | Out-Null
New-ItemProperty -Path "HKCU:\Software\Classes\CLSID\$clsid\InprocServer32" -Name "ThreadingModel" -Value "Apartment" -Force | Out-Null

Write-Host "Registered COM server."