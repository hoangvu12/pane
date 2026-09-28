# Native GUI smoke on Windows: launches Pane, drives it with real key events
# and captures the screen. Pane keeps installed packages in <output-dir>\data,
# not the user's data folder.
# Usage: scripts/smoke-windows.ps1 -OutDir <output-dir>
param([string]$OutDir = "smoke")
$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$data = Join-Path $OutDir "data"
if (Test-Path $data) { Remove-Item -Recurse -Force $data }
$env:PANE_DATA_DIR = $data
# The tested OS version and architecture
"$([System.Environment]::OSVersion.VersionString) $env:PROCESSOR_ARCHITECTURE" | Set-Content (Join-Path $OutDir "system.txt")
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @"
using System; using System.Runtime.InteropServices;
public static class Win { [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h); }
"@
function Capture($name) {
    $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
    $bitmap.Save((Join-Path $OutDir $name))
}
function Check($name, $color) {
    python "$PSScriptRoot/check_screenshot.py" (Join-Path $OutDir $name) $color
    if ($LASTEXITCODE -ne 0) { throw "screenshot check failed: $name" }
}
function Send($keys) { [System.Windows.Forms.SendKeys]::SendWait($keys) }
# Starts Pane with the given arguments, writing its errors to $log, and
# brings its window to the front.
function Start-Pane($log, [string[]]$arguments) {
    $options = @{
        FilePath = "target/debug/pane.exe"
        PassThru = $true
        RedirectStandardError = (Join-Path $OutDir $log)
    }
    if ($arguments) { $options.ArgumentList = $arguments }
    $process = Start-Process @options
    for ($i = 0; $i -lt 50 -and $process.MainWindowHandle -eq 0; $i++) {
        Start-Sleep -Milliseconds 200; $process.Refresh()
    }
    if ($process.MainWindowHandle -eq 0) { throw "Pane window did not appear" }
    Start-Sleep -Seconds 2
    [Win]::SetForegroundWindow($process.MainWindowHandle) | Out-Null
    return $process
}
function Stop-Pane($process) {
    if ($process.HasExited) { throw "Pane exited during the smoke" }
    Stop-Process -Id $process.Id
    $process.WaitForExit()
}

$process = Start-Pane "stderr.log"
Capture "1-root.png"
Check "1-root.png" "8a96a3"   # the hint line: text renders
# Open each sample command (Rust, JavaScript, TypeScript) and run an item.
foreach ($index in 0..2) {
    for ($i = 0; $i -lt $index; $i++) { Send "{DOWN}" }
    Send "{ENTER}"; Start-Sleep -Seconds 3
    Capture "$($index + 2)-command-$index.png"
    Send "{DOWN}{ENTER}"; Start-Sleep -Seconds 2
    Capture "$($index + 2)-result-$index.png"
    Check "$($index + 2)-result-$index.png" "9fd8a8"   # the guest's answer
    Send "{ESC}"; Start-Sleep -Seconds 1
}
Capture "5-back-to-root.png"
# Each command must have answered from its own guest, not the same view twice.
python "$PSScriptRoot/check_screenshot.py" --distinct @(2..4 | ForEach-Object { Join-Path $OutDir "$_-result-$($_ - 2).png" })
if ($LASTEXITCODE -ne 0) { throw "screenshot check failed: result screenshots are not distinct" }
Stop-Pane $process

# Install the assembled Rust sample package (the folder the picker would
# return), then run its command. Root lists the three samples, the installed
# command, then the install and Manage extensions rows.
$process = Start-Pane "stderr-install.log" @("--install", "target/guests/packages/sample-rust")
Capture "6-package.png"
Check "6-package.png" "aab4c0"   # the package's identity and compatibility lines
Send "{ENTER}"; Start-Sleep -Seconds 2
Capture "7-installed.png"
Check "7-installed.png" "9fd8a8"   # "Installed Rust sample"
Send "{ENTER}"; Start-Sleep -Seconds 3
Send "{ENTER}"; Start-Sleep -Seconds 2
Capture "8-installed-result.png"
Check "8-installed-result.png" "9fd8a8"   # the installed guest's answer
Stop-Pane $process

# The installed command is still listed after a restart.
$process = Start-Pane "stderr-restart.log"
Capture "9-restarted.png"
Check "9-restarted.png" "8a96a3"
if (-not (Test-Path (Join-Path $data "extensions/installed.json"))) { throw "no install record" }
Stop-Pane $process

# Install the settings sample, save a choice with it, then disable it in
# Manage extensions. Root lists the three samples, Rust sample, Greeting, the
# install row, then Manage extensions... last; the extension list holds Rust
# sample, then Settings sample.
$process = Start-Pane "stderr-settings.log" @("--install", "target/guests/packages/sample-settings")
Send "{ENTER}"; Start-Sleep -Seconds 2   # Install; Greeting is selected
Send "{ENTER}"; Start-Sleep -Seconds 3   # open Greeting
Send "{ENTER}"; Start-Sleep -Seconds 2   # "Use a formal greeting"
Capture "10-setting-saved.png"
Check "10-setting-saved.png" "9fd8a8"   # "Saved the formal greeting"
Send "{ESC}"; Start-Sleep -Seconds 1
Send "{DOWN 10}"   # the last row
Send "{ENTER}"; Start-Sleep -Seconds 1
Send "{DOWN}{ENTER}"; Start-Sleep -Seconds 2
Capture "11-disabled.png"
Check "11-disabled.png" "9fd8a8"   # "Disabled Settings sample"
Stop-Pane $process
if (-not (Select-String -Quiet -SimpleMatch '"disabled": true' (Join-Path $data "extensions/installed.json"))) { throw "disabled state not recorded" }
if (-not (Select-String -Quiet -SimpleMatch '"greeting-style": "formal"' (Join-Path $data "extensions/settings.json"))) { throw "setting not saved" }

# After a restart Greeting is no longer in root search. Enabling the package
# again brings it back with its setting: "Greet me" answers in the saved
# formal style, where without a saved style it reports an error.
$process = Start-Pane "stderr-reenable.log"
Capture "12-restarted-disabled.png"
Check "12-restarted-disabled.png" "8a96a3"
Send "{DOWN 10}"
Send "{ENTER}"; Start-Sleep -Seconds 1
Send "{DOWN}{ENTER}"; Start-Sleep -Seconds 2
Capture "13-enabled.png"
Check "13-enabled.png" "9fd8a8"   # "Enabled Settings sample"
Send "{ESC}"; Start-Sleep -Seconds 1
Send "{DOWN 4}"   # Greeting
Send "{ENTER}"; Start-Sleep -Seconds 3
Send "{DOWN}{DOWN}{ENTER}"; Start-Sleep -Seconds 2   # "Greet me"
Capture "14-greeted.png"
Check "14-greeted.png" "9fd8a8"   # "Good day to you"
Stop-Pane $process
Write-Output "screenshots in $OutDir"
