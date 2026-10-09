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

# カテゴリ GUID
$keyboardCategory = "{34745C63-B2F0-4784-8B67-5E12C8701A31}"
$inputModeCategory = "{CCF05DD7-4A87-11D7-A6E2-00065B84435C}"
$legacyInputModeCategory = "{06421B01-32FA-41FA-A41D-16FB3D3A3439}"
$systemTrayCategory = "{25504FB4-7BAB-4BC1-9C69-CF81890F0EF5}"


# ============================================================
# TSF登録解除用COMインターフェース
# ============================================================

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class SamoyedTsfUnregistration
{
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

    [ComImport]
    [Guid("1F02B6C5-7842-4EE6-8A0B-9A24183A95CA")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface ITfInputProcessorProfiles
    {
        [PreserveSig]
        int Register(ref Guid rclsid);

        [PreserveSig]
        int Unregister(ref Guid rclsid);
    }

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

        [PreserveSig]
        int UnregisterCategory(
            ref Guid rclsid,
            ref Guid rcatid,
            ref Guid rguid
        );
    }

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

    public static void UnregisterAllProfiles(
        string clsidText,
        string profileGuidText,
        ushort langId
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

            // TF_URP_ALLPROFILES = 0x00000002
            const uint TF_URP_ALLPROFILES = 0x00000002;

            int hr = profiles.UnregisterProfile(
                ref clsid,
                langId,
                ref profileGuid,
                TF_URP_ALLPROFILES
            );

            CheckHResult(
                hr,
                "ITfInputProcessorProfileMgr::UnregisterProfile"
            );
        }
        finally
        {
            Marshal.Release(unk);
        }
    }

    public static void UnregisterLegacyTextService(
        string clsidText
    )
    {
        Guid clsid = new Guid(clsidText);

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
            ITfInputProcessorProfiles profiles =
                (ITfInputProcessorProfiles)
                Marshal.GetTypedObjectForIUnknown(
                    unk,
                    typeof(ITfInputProcessorProfiles)
                );

            int hr = profiles.Unregister(ref clsid);

            CheckHResult(
                hr,
                "ITfInputProcessorProfiles::Unregister"
            );
        }
        finally
        {
            Marshal.Release(unk);
        }
    }

    public static void UnregisterCategory(
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

            int hr = categoryMgr.UnregisterCategory(
                ref clsid,
                ref category,
                ref clsid
            );

            CheckHResult(
                hr,
                "ITfCategoryMgr::UnregisterCategory"
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
# TSF登録を解除
# ============================================================

$profileUnregistered = $false

try {
    [SamoyedTsfUnregistration]::UnregisterAllProfiles(
        $clsid,
        $profileGuid,
        [uint16]$langId
    )

    $profileUnregistered = $true
    Write-Host "Unregistered TSF profiles."
}
catch {
    Write-Warning "Modern TSF unregistration failed: $($_.Exception.Message)"
    Write-Host "Trying legacy TSF unregistration."

    try {
        [SamoyedTsfUnregistration]::UnregisterLegacyTextService($clsid)

        $profileUnregistered = $true
        Write-Host "Unregistered TSF text service using the legacy API."
    }
    catch {
        Write-Warning "Legacy TSF unregistration also failed: $($_.Exception.Message)"
    }
}

if (-not $profileUnregistered) {
    throw "TSF unregistration failed. COM registration was not removed."
}


# ============================================================
# TSFカテゴリ登録を解除
# ============================================================

$categories = @(
    $keyboardCategory,
    $inputModeCategory,
    $legacyInputModeCategory,
    $systemTrayCategory
)

foreach ($category in $categories) {
    try {
        [SamoyedTsfUnregistration]::UnregisterCategory(
            $clsid,
            $category
        )

        Write-Host "Unregistered TSF category: $category"
    }
    catch {
        Write-Warning "Category unregistration failed for $category : $($_.Exception.Message)"
    }
}


# ============================================================
# COMサーバー登録を解除 (HKLM / HKCU)
# ============================================================

$clsidKeys = @(
    "HKLM:\Software\Classes\CLSID\$clsid",
    "HKCU:\Software\Classes\CLSID\$clsid"
)

foreach ($clsidKey in $clsidKeys) {
    if (Test-Path -LiteralPath $clsidKey) {
        Remove-Item `
            -LiteralPath $clsidKey `
            -Recurse `
            -Force

        Write-Host "Removed COM registration: $clsidKey"
    }
    else {
        Write-Host "COM registration was not found: $clsidKey"
    }
}


Write-Host ""
Write-Host "Samoyed IME unregistration completed."