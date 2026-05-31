# Reads Lexi Supabase session from Windows Credential Manager (for maintenance scripts).
Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;

public class CredMan {
  [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
  public struct CREDENTIAL {
    public uint Flags;
    public uint Type;
    public IntPtr TargetName;
    public IntPtr Comment;
    public System.Runtime.InteropServices.ComTypes.FILETIME LastWritten;
    public uint CredentialBlobSize;
    public IntPtr CredentialBlob;
    public uint Persist;
    public uint AttributeCount;
    public IntPtr Attributes;
    public IntPtr TargetAlias;
    public IntPtr UserName;
  }

  [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
  public static extern bool CredRead(string target, uint type, uint reservedFlag, out IntPtr credentialPtr);

  [DllImport("advapi32.dll", SetLastError = true)]
  public static extern bool CredFree(IntPtr cred);

  public const uint CRED_TYPE_GENERIC = 1;
}
"@

$target = "io.github.cotrin8672.lexi:supabase-session"
$ptr = [IntPtr]::Zero
if (-not [CredMan]::CredRead($target, [CredMan]::CRED_TYPE_GENERIC, 0, [ref]$ptr)) {
  throw "Credential not found: $target"
}
try {
  $cred = [Runtime.InteropServices.Marshal]::PtrToStructure($ptr, [type][CredMan+CREDENTIAL])
  $bytes = New-Object byte[] $cred.CredentialBlobSize
  [Runtime.InteropServices.Marshal]::Copy($cred.CredentialBlob, $bytes, 0, $cred.CredentialBlobSize)
  [Text.Encoding]::UTF8.GetString($bytes)
} finally {
  [CredMan]::CredFree($ptr) | Out-Null
}
