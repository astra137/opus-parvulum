param(
    [switch]$elevated,
    [string]$target = "debug",
    [string]$dir = $PWD
)

function Test-Admin {
    $currentUser = New-Object Security.Principal.WindowsPrincipal $([Security.Principal.WindowsIdentity]::GetCurrent())
    $currentUser.IsInRole([Security.Principal.WindowsBuiltinRole]::Administrator)
}

if ((Test-Admin) -eq $false)  {
    if ($elevated) {
        # tried to elevate, did not work, aborting
        # this prevents infinite loops
    } else {
        Start-Process powershell.exe -Verb RunAs -ArgumentList ('-noprofile -file "{0}" -elevated -dir "{1}"' -f $myinvocation.MyCommand.Definition, $dir)
    }
    exit
}

Set-Location $dir

# New VST Bundle Format
# https://developer.steinberg.help/pages/viewpage.action?pageId=9798275

$VST_link = "C:\Program Files\Common Files\VST3\opus_parvulum.vst3"
$VST_root = "target\$target\opus_parvulum.vst3"
$VST_dll = "target\$target\opus_parvulum.dll"

mkdir "$VST_root\Contents\x86_64-win"
New-Item -ItemType SymbolicLink -Path "$VST_root\Contents\x86_64-win\opus_parvulum.vst3" -Target "$VST_dll" -Force
New-Item -ItemType SymbolicLink -Path "$VST_link" -Target "$VST_root" -Force
