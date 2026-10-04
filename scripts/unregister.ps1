$ErrorActionPreference = "Stop"

# 管理者権限を確認
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = New-Object Security.Principal.WindowsPrincipal($identity)

if (-not $principal.IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator
)) {
    Write-Host "Please run PowerShell as Administrator."
    exit 1
}


# 設定
$clsid = "{cedc333b-8783-4e6e-9bf5-45310d7bffae}"


# TSF登録解除用COMインターフェース
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class SamoyedTsfUnregistration
{
    [ComImport]
    [Guid("1F02B6C5-7842-4EE6-8A0B-9A24183A95CA")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface ITfInputProcessorProfiles
    {
        [PreserveSig]
        int Register(ref Guid rclsid);

        [PreserveSig]
        int Unregister(ref Guid rclsid);

        [PreserveSig]
        int AddLanguageProfile(
            ref Guid rclsid,
            ushort langid,
            ref Guid guidProfile,
            [MarshalAs(UnmanagedType.LPWStr)] string pchDesc,
            uint cchDesc,
            [MarshalAs(UnmanagedType.LPWStr)] string pchIconFile,
            uint cchFile,
            uint uIconIndex
        );

        [PreserveSig]
        int RemoveLanguageProfile(
            ref Guid rclsid,
            ushort langid,
            ref Guid guidProfile
        );
    }

    public static void UnregisterProfile(
        string clsidText
    )
    {
        Guid clsid = new Guid(clsidText);

        Guid profilesClsid =
            new Guid("33C53A50-F456-4884-B049-85FD643ECFED");

        Type profilesType =
            Type.GetTypeFromCLSID(profilesClsid, true);

        ITfInputProcessorProfiles profiles =
            (ITfInputProcessorProfiles)
                Activator.CreateInstance(profilesType);

        int hr = profiles.Unregister(ref clsid);

        // 既に登録されていない場合も終了できるようにする
        if (hr < 0)
        {
            throw new COMException(
                "ITfInputProcessorProfiles::Unregister failed. HRESULT=0x"
                + ((uint)hr).ToString("X8"),
                hr
            );
        }
    }
}
"@


# TSF登録を解除
[SamoyedTsfUnregistration]::UnregisterProfile($clsid)

Write-Host "Unregistered TSF text service."


# COMサーバー登録を解除
$clsidKey = "HKCU:\Software\Classes\CLSID\$clsid"

if (Test-Path $clsidKey) {
    Remove-Item `
        -Path $clsidKey `
        -Recurse `
        -Force

    Write-Host "Unregistered COM server."
}
else {
    Write-Host "COM registration was not found."
}


Write-Host ""
Write-Host "Samoyed IME unregistration completed."