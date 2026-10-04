$ErrorActionPreference = "Stop"

# ============================================================
# 管理者権限を確認
# ============================================================

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = New-Object Security.Principal.WindowsPrincipal($identity)

if (-not $principal.IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator
)) {
    throw "Please run PowerShell as Administrator."
}


# ============================================================
# 設定
# ============================================================

$clsid = "{cedc333b-8783-4e6e-9bf5-45310d7bffae}"
$profileGuid = "{d75a0971-4669-49ea-8ffe-42dc31386891}"

# 日本語
$langId = 0x0411

# Keyboard TIP
$keyboardCategory = "{34745C63-B2F0-4784-8B67-5E12C8701A31}"


# ============================================================
# DLLを探す
# ============================================================

$dllCandidates = @(
    (Join-Path $PSScriptRoot "..\target\debug\ime_tsf.dll"),
    (Join-Path $PSScriptRoot "..\crates\ime_tsf\target\debug\ime_tsf.dll")
)

$dllPath = $null

foreach ($candidate in $dllCandidates) {
    if (Test-Path $candidate) {
        $dllPath = (Resolve-Path $candidate).Path
        break
    }
}

if (-not $dllPath) {
    throw "ime_tsf.dll was not found."
}

Write-Host "DLL: $dllPath"


# ============================================================
# COMサーバーを登録
# ============================================================

$clsidKey = "HKCU:\Software\Classes\CLSID\$clsid"
$inprocKey = "$clsidKey\InprocServer32"

New-Item `
    -Path $clsidKey `
    -Force |
    Out-Null

Set-ItemProperty `
    -Path $clsidKey `
    -Name "(Default)" `
    -Value "Samoyed IME"

New-Item `
    -Path $inprocKey `
    -Force |
    Out-Null

Set-ItemProperty `
    -Path $inprocKey `
    -Name "(Default)" `
    -Value $dllPath

Set-ItemProperty `
    -Path $inprocKey `
    -Name "ThreadingModel" `
    -Value "Apartment"

Write-Host "Registered COM server."


# ============================================================
# TSF COMインターフェース
# ============================================================

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class SamoyedTsfRegistration
{
    // ========================================================
    // ITfInputProcessorProfileMgr
    // ========================================================

    [ComImport]
    [Guid("71C6E74C-0F28-11D8-A82A-00065B84435C")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface ITfInputProcessorProfileMgr
    {
        [PreserveSig]
        int ActivateProfile(
            uint dwProfileType,
            ushort langid,
            ref Guid clsid,
            ref Guid guidProfile,
            IntPtr hkl,
            uint dwFlags
        );

        [PreserveSig]
        int DeactivateProfile(
            uint dwProfileType,
            ushort langid,
            ref Guid clsid,
            ref Guid guidProfile,
            IntPtr hkl,
            uint dwFlags
        );

        [PreserveSig]
        int GetProfile(
            uint dwProfileType,
            ushort langid,
            ref Guid clsid,
            ref Guid guidProfile,
            IntPtr hkl,
            IntPtr pProfile
        );

        [PreserveSig]
        int EnumProfiles(
            ushort langid,
            IntPtr ppEnum
        );

        [PreserveSig]
        int ReleaseInputProcessor(
            ref Guid clsid,
            uint dwFlags
        );

        [PreserveSig]
        int RegisterProfile(
            ref Guid clsid,
            ushort langid,
            ref Guid guidProfile,
            [MarshalAs(UnmanagedType.LPWStr)] string pchDesc,
            uint cchDesc,
            [MarshalAs(UnmanagedType.LPWStr)] string pchIconFile,
            uint cchFile,
            uint uIconIndex,
            IntPtr hklSubstitute,
            uint dwPreferredLayout,
            [MarshalAs(UnmanagedType.Bool)] bool bEnabledByDefault,
            uint dwFlags
        );

        [PreserveSig]
        int UnregisterProfile(
            ref Guid clsid,
            ushort langid,
            ref Guid guidProfile,
            uint dwFlags
        );

        [PreserveSig]
        int GetActiveProfile(
            ref Guid catid,
            IntPtr pProfile
        );
    }


    // ========================================================
    // ITfCategoryMgr
    // ========================================================

    [ComImport]
    [Guid("C3ACEFB5-F69D-4905-938F-FCADCF4BE830")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface ITfCategoryMgr
    {
        [PreserveSig]
        int RegisterCategory(
            ref Guid rclsid,
            ref Guid rcatid,
            ref Guid rguid
        );
    }


    // ========================================================
    // HRESULT
    // ========================================================

    private static void CheckHResult(
        int hr,
        string operation
    )
    {
        if (hr < 0)
        {
            throw new COMException(
                operation
                + " failed. HRESULT=0x"
                + ((uint)hr).ToString("X8"),
                hr
            );
        }
    }


    // ========================================================
    // TSF Profile登録
    // ========================================================

    public static void RegisterProfile(
        string clsidText,
        string profileGuidText,
        ushort langId,
        string description
    )
    {
        Guid clsid = new Guid(clsidText);
        Guid profileGuid = new Guid(profileGuidText);

        Guid profilesClsid =
            new Guid("33C53A50-F456-4884-B049-85FD643ECFED");

        Type profilesType =
            Type.GetTypeFromCLSID(profilesClsid, true);

        object comObject =
            Activator.CreateInstance(profilesType);

        IntPtr unk =
            Marshal.GetIUnknownForObject(comObject);

        try
        {
            ITfInputProcessorProfileMgr profiles =
                (ITfInputProcessorProfileMgr)
                Marshal.GetTypedObjectForIUnknown(
                    unk,
                    typeof(ITfInputProcessorProfileMgr)
                );

            int hr = profiles.RegisterProfile(
                ref clsid,
                langId,
                ref profileGuid,
                description,
                (uint)description.Length,
                null,
                0,
                0,
                IntPtr.Zero,
                0,
                false,
                0
            );

            CheckHResult(
                hr,
                "RegisterProfile"
            );
        }
        finally
        {
            Marshal.Release(unk);
        }
    }


    // ========================================================
    // Keyboard TIPカテゴリ登録
    // ========================================================

    public static void RegisterKeyboardCategory(
        string clsidText,
        string categoryText
    )
    {
        Guid clsid = new Guid(clsidText);
        Guid category = new Guid(categoryText);

        Guid categoryMgrClsid =
            new Guid("A4B544A1-438D-4B41-9325-869523E2D6C7");

        Type categoryMgrType =
            Type.GetTypeFromCLSID(
                categoryMgrClsid,
                true
            );

        object comObject =
            Activator.CreateInstance(categoryMgrType);

        IntPtr unk =
            Marshal.GetIUnknownForObject(comObject);

        try
        {
            ITfCategoryMgr categoryMgr =
                (ITfCategoryMgr)
                Marshal.GetTypedObjectForIUnknown(
                    unk,
                    typeof(ITfCategoryMgr)
                );

            int hr = categoryMgr.RegisterCategory(
                ref clsid,
                ref category,
                ref clsid
            );

            CheckHResult(
                hr,
                "RegisterCategory"
            );
        }
        finally
        {
            Marshal.Release(unk);
        }
    }
}
"@


# ============================================================
# TSFへ登録
# ============================================================

[SamoyedTsfRegistration]::RegisterProfile(
    $clsid,
    $profileGuid,
    [uint16]$langId,
    "Samoyed IME"
)

Write-Host "Registered TSF profile."


# ============================================================
# Keyboard TIPとして登録
# ============================================================

[SamoyedTsfRegistration]::RegisterKeyboardCategory(
    $clsid,
    $keyboardCategory
)

Write-Host "Registered keyboard TIP category."

Write-Host ""
Write-Host "Samoyed IME system registration completed."