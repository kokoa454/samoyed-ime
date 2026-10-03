$clsid = "{cedc333b-8783-4e6e-9bf5-45310d7bffae}"
Remove-Item -Path "HKCU:\Software\Classes\CLSID\$clsid" -Recurse -Force -ErrorAction SilentlyContinue
Write-Host "Unregistered COM server." 