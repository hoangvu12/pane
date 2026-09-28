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
# Brings Pane's window to the front, so that key events reach it.
function Focus-Pane($process) {
    [Win]::SetForegroundWindow($process.MainWindowHandle) | Out-Null
    Start-Sleep -Milliseconds 500
}
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
    Focus-Pane $process
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

# The Rust command's form (its fifth item): submitting it empty is rejected
# and focus returns to the name, so typing there and choosing a greeting with
# Tab and Down makes the guest answer.
Send "{ENTER}"; Start-Sleep -Seconds 3
Send "{DOWN}{DOWN}{DOWN}{DOWN}{ENTER}"; Start-Sleep -Seconds 1
Capture "6-form.png"
Send "{ENTER}"; Start-Sleep -Seconds 2
Capture "7-form-error.png"
Check "7-form-error.png" "f08c8c"   # the rejected field's message
Send "Ada{TAB}{DOWN}{ENTER}"; Start-Sleep -Seconds 2
Capture "8-form-result.png"
Check "8-form-result.png" "9fd8a8"   # the guest's answer
Send "{ESC}{ESC}"; Start-Sleep -Seconds 1
Stop-Pane $process

# Install the assembled Rust sample package (the folder the picker would
# return), then run its command. Root lists the three samples, the installed
# command, then the install row.
$process = Start-Pane "stderr-install.log" @("--install", "target/guests/packages/sample-rust")
Capture "9-package.png"
Check "9-package.png" "aab4c0"   # the package's identity and compatibility lines
Send "{ENTER}"; Start-Sleep -Seconds 2
Capture "10-installed.png"
Check "10-installed.png" "9fd8a8"   # "Installed Rust sample"
Send "{ENTER}"; Start-Sleep -Seconds 3
Send "{ENTER}"; Start-Sleep -Seconds 2
Capture "11-installed-result.png"
Check "11-installed-result.png" "9fd8a8"   # the installed guest's answer
Stop-Pane $process

# The installed command is still listed after a restart.
$process = Start-Pane "stderr-restart.log"
Capture "12-restarted.png"
Check "12-restarted.png" "8a96a3"
if (-not (Test-Path (Join-Path $data "extensions/installed.json"))) { throw "no install record" }
Focus-Pane $process

# The Rust command's sixth item is declared for Windows only, its seventh
# for macOS and Linux only. Here the first runs and the second is explained
# without running.
Send "{ENTER}"; Start-Sleep -Seconds 3
Send "{DOWN}{DOWN}{DOWN}{DOWN}{DOWN}{ENTER}"; Start-Sleep -Seconds 2
Capture "13-windows-only.png"
Check "13-windows-only.png" "9fd8a8"   # Windows: the guest's answer
Send "{DOWN}{ENTER}"; Start-Sleep -Seconds 2
Capture "14-not-windows.png"
Check "14-not-windows.png" "d6a36a"   # the row's reason
Check "14-not-windows.png" "f08c8c"   # Windows: the reason as the error
Send "{ESC}"; Start-Sleep -Seconds 1
Stop-Pane $process

# A package that supports only the other two systems has nothing for this
# one: it is explained instead of offered for installation.
$elsewhere = Join-Path $OutDir "elsewhere"
New-Item -ItemType Directory -Force -Path $elsewhere | Out-Null
Copy-Item "target/guests/sample_rust.wasm" $elsewhere
@'
{
  "manifestVersion": 1,
  "title": "Elsewhere",
  "apiVersion": "0.1",
  "platforms": ["macos", "linux"],
  "commands": [{ "id": "sample", "title": "Elsewhere sample", "component": "sample_rust.wasm" }]
}
'@ | Set-Content -Encoding ascii (Join-Path $elsewhere "pane.json")
$process = Start-Pane "stderr-elsewhere.log" @("--install", $elsewhere)
Capture "15-no-compatible-package.png"
Check "15-no-compatible-package.png" "f08c8c"   # "Not available on Windows: ..."
Stop-Pane $process
Write-Output "screenshots in $OutDir"
