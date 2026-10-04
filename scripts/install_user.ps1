$ErrorActionPreference = "Stop"

# ============================================================
# 設定
# ============================================================

$clsid = "{cedc333b-8783-4e6e-9bf5-45310d7bffae}"
$profileGuid = "{d75a0971-4669-49ea-8ffe-42dc31386891}"

# 日本語
$langId = 0x0411


# ============================================================
# TSF / input.dll
# ============================================================

Add-Type -TypeDefinition @"
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;

public static class SamoyedUserInstall
{
    // ========================================================
    // COM
    // ========================================================

    [DllImport(
        "ole32.dll",
        CallingConvention = CallingConvention.StdCall
    )]
    private static extern int CoInitializeEx(
        IntPtr pvReserved,
        uint dwCoInit
    );

    [DllImport(
        "ole32.dll",
        CallingConvention = CallingConvention.StdCall
    )]
    private static extern void CoUninitialize();


    // ========================================================
    // input.dll
    // ========================================================

    [DllImport(
        "kernel32.dll",
        CharSet = CharSet.Unicode,
        SetLastError = true
    )]
    private static extern IntPtr LoadLibrary(
        string lpFileName
    );

    [DllImport(
        "kernel32.dll",
        SetLastError = true
    )]
    private static extern bool FreeLibrary(
        IntPtr hModule
    );

    [DllImport(
        "kernel32.dll",
        CharSet = CharSet.Ansi,
        SetLastError = true
    )]
    private static extern IntPtr GetProcAddress(
        IntPtr hModule,
        string lpProcName
    );


    // ========================================================
    // InstallLayoutOrTip
    // ========================================================

    [UnmanagedFunctionPointer(
        CallingConvention.StdCall,
        CharSet = CharSet.Unicode
    )]
    [return: MarshalAs(UnmanagedType.Bool)]
    private delegate bool InstallLayoutOrTipDelegate(
        [MarshalAs(UnmanagedType.LPWStr)]
        string psz,
        uint dwFlags
    );


    // ========================================================
    // SetDefaultLayoutOrTip
    // ========================================================

    [UnmanagedFunctionPointer(
        CallingConvention.StdCall,
        CharSet = CharSet.Unicode
    )]
    [return: MarshalAs(UnmanagedType.Bool)]
    private delegate bool SetDefaultLayoutOrTipDelegate(
        [MarshalAs(UnmanagedType.LPWStr)]
        string psz,
        uint dwFlags
    );


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
    // input.dllの関数取得
    // ========================================================

    private static T GetFunction<T>(
        IntPtr module,
        string name
    )
    {
        IntPtr proc =
            GetProcAddress(
                module,
                name
            );

        if (proc == IntPtr.Zero)
        {
            throw new Win32Exception(
                Marshal.GetLastWin32Error(),
                name + " could not be found."
            );
        }

        return (T)(object)
            Marshal.GetDelegateForFunctionPointer(
                proc,
                typeof(T)
            );
    }


    // ========================================================
    // InstallLayoutOrTip
    // ========================================================

    public static void InstallLayoutOrTip(
        string profile
    )
    {
        string inputDll =
            System.IO.Path.Combine(
                Environment.GetFolderPath(
                    Environment.SpecialFolder.System
                ),
                "input.dll"
            );

        IntPtr module =
            LoadLibrary(inputDll);

        if (module == IntPtr.Zero)
        {
            throw new Win32Exception(
                Marshal.GetLastWin32Error(),
                "input.dll could not be loaded."
            );
        }

        try
        {
            InstallLayoutOrTipDelegate install =
                GetFunction<InstallLayoutOrTipDelegate>(
                    module,
                    "InstallLayoutOrTip"
                );

            if (!install(profile, 0))
            {
                throw new Win32Exception(
                    Marshal.GetLastWin32Error(),
                    "InstallLayoutOrTip failed."
                );
            }
        }
        finally
        {
            FreeLibrary(module);
        }
    }


    // ========================================================
    // SetDefaultLayoutOrTip
    // ========================================================

    public static void SetDefaultLayoutOrTip(
        string profile
    )
    {
        string inputDll =
            System.IO.Path.Combine(
                Environment.GetFolderPath(
                    Environment.SpecialFolder.System
                ),
                "input.dll"
            );

        IntPtr module =
            LoadLibrary(inputDll);

        if (module == IntPtr.Zero)
        {
            throw new Win32Exception(
                Marshal.GetLastWin32Error(),
                "input.dll could not be loaded."
            );
        }

        try
        {
            SetDefaultLayoutOrTipDelegate setDefault =
                GetFunction<SetDefaultLayoutOrTipDelegate>(
                    module,
                    "SetDefaultLayoutOrTip"
                );

            if (!setDefault(profile, 0))
            {
                throw new Win32Exception(
                    Marshal.GetLastWin32Error(),
                    "SetDefaultLayoutOrTip failed."
                );
            }
        }
        finally
        {
            FreeLibrary(module);
        }
    }


    // ========================================================
    // ActivateProfile
    // ========================================================

    public static void ActivateProfile(
        string clsidText,
        string profileGuidText,
        ushort langId
    )
    {
        // STAでCOMを初期化
        int coinit =
            CoInitializeEx(
                IntPtr.Zero,
                0x2
            );

        // S_OK(0) / S_FALSE(1) は続行
        if (coinit != 0 && coinit != 1)
        {
            throw new COMException(
                "CoInitializeEx failed. HRESULT=0x"
                + ((uint)coinit).ToString("X8"),
                coinit
            );
        }

        try
        {
            Guid clsid =
                new Guid(clsidText);

            Guid profileGuid =
                new Guid(profileGuidText);

            Guid profileMgrClsid =
                new Guid(
                    "33C53A50-F456-4884-B049-85FD643ECFED"
                );

            Type profileMgrType =
                Type.GetTypeFromCLSID(
                    profileMgrClsid,
                    true
                );

            object comObject =
                Activator.CreateInstance(
                    profileMgrType
                );

            IntPtr unk =
                Marshal.GetIUnknownForObject(
                    comObject
                );

            try
            {
                ITfInputProcessorProfileMgr mgr =
                    (ITfInputProcessorProfileMgr)
                    Marshal.GetTypedObjectForIUnknown(
                        unk,
                        typeof(ITfInputProcessorProfileMgr)
                    );

                // Text Service
                const uint TF_PROFILETYPE_INPUTPROCESSOR = 0x0001;

                // 現在のデスクトップセッション全体
                const uint TF_IPPMF_FORSESSION = 0x20000000;

                // ユーザー側のProfileを有効化
                const uint TF_IPPMF_ENABLEPROFILE = 0x00000001;

                // 現在の入力言語が0x0411でなくても
                // エラーにせず、日本語へ切り替えた際に有効化
                const uint TF_IPPMF_DONTCARECURRENTINPUTLANGUAGE = 0x00000004;

                uint flags =
                    TF_IPPMF_FORSESSION
                    | TF_IPPMF_ENABLEPROFILE
                    | TF_IPPMF_DONTCARECURRENTINPUTLANGUAGE;

                int hr =
                    mgr.ActivateProfile(
                        TF_PROFILETYPE_INPUTPROCESSOR,
                        langId,
                        ref clsid,
                        ref profileGuid,
                        IntPtr.Zero,
                        flags
                    );

                CheckHResult(
                    hr,
                    "ITfInputProcessorProfileMgr::ActivateProfile"
                );
            }
            finally
            {
                Marshal.Release(unk);
            }
        }
        finally
        {
            if (coinit == 0 || coinit == 1)
            {
                CoUninitialize();
            }
        }
    }
}
"@


# ============================================================
# Profile文字列
# ============================================================

$profile =
    "0x{0:X4}:{1}{2}" -f `
    $langId, `
    $clsid, `
    $profileGuid

Write-Host "Installing for current user:"
Write-Host $profile
Write-Host ""


# ============================================================
# 現在ユーザーの入力方式へ追加
# ============================================================

[SamoyedUserInstall]::InstallLayoutOrTip(
    $profile
)

Write-Host "Installed Samoyed IME."


# ============================================================
# デフォルト入力方式へ設定
# ============================================================

[SamoyedUserInstall]::SetDefaultLayoutOrTip(
    $profile
)

Write-Host "Set Samoyed IME as default input profile."


# ============================================================
# TSFセッションへActivate
# ============================================================

[SamoyedUserInstall]::ActivateProfile(
    $clsid,
    $profileGuid,
    [uint16]$langId
)

Write-Host "Activated Samoyed IME profile."

Write-Host ""
Write-Host "Samoyed IME user installation completed."