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
public static class Win {
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint x, uint y, uint data, UIntPtr extra);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
}
"@
# Screenshots, screen bounds and SetCursorPos then all use physical pixels,
# so a position found in a screenshot is where the click lands at any
# display scaling.
[Win]::SetProcessDPIAware() | Out-Null
function Capture($name) {
    $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
    $bitmap.Save((Join-Path $OutDir $name))
}
function Check($name, $color, $minimum = 20) {
    python "$PSScriptRoot/check_screenshot.py" (Join-Path $OutDir $name) $color $minimum
    if ($LASTEXITCODE -ne 0) { throw "screenshot check failed: $name" }
}
# Returns x, y: where the screenshot shows the given color.
function Locate($name, $color) {
    $at = python "$PSScriptRoot/check_screenshot.py" --locate (Join-Path $OutDir $name) $color
    if ($LASTEXITCODE -ne 0) { throw "color not found: $name $color" }
    return [int[]]($at -split " ")
}
# Clicks the primary button at x, y in the screenshot's pixels.
function Click-At($x, $y) {
    [Win]::SetCursorPos($x, $y) | Out-Null
    [Win]::mouse_event(0x2, 0, 0, 0, [UIntPtr]::Zero)   # left button down
    [Win]::mouse_event(0x4, 0, 0, 0, [UIntPtr]::Zero)   # left button up
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
# command, then the install and Manage extensions rows.
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

# The Rust command's seventh item is declared for Windows only, its eighth
# for macOS and Linux only. Here the first runs and the second is explained
# without running.
Send "{ENTER}"; Start-Sleep -Seconds 3
Send "{DOWN}{DOWN}{DOWN}{DOWN}{DOWN}{DOWN}{ENTER}"; Start-Sleep -Seconds 2
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

# Install the settings sample, save a choice with it, then disable it in
# Manage extensions. Root lists the three samples, Rust sample, Greeting, the
# install row, then Manage extensions... last; the extension list holds Rust
# sample, then Settings sample.
$process = Start-Pane "stderr-settings.log" @("--install", "target/guests/packages/sample-settings")
Send "{ENTER}"; Start-Sleep -Seconds 2   # Install; Greeting is selected
Send "{ENTER}"; Start-Sleep -Seconds 3   # open Greeting
Send "{ENTER}"; Start-Sleep -Seconds 2   # "Use a formal greeting"
Capture "16-setting-saved.png"
Check "16-setting-saved.png" "9fd8a8"   # "Saved the formal greeting"
Send "{ESC}"; Start-Sleep -Seconds 1
Send "{DOWN 10}"   # the last row
Send "{ENTER}"; Start-Sleep -Seconds 1
Send "{DOWN}{ENTER}"; Start-Sleep -Seconds 2
Capture "17-disabled.png"
Check "17-disabled.png" "9fd8a8"   # "Disabled Settings sample"
Stop-Pane $process
if (-not (Select-String -Quiet -SimpleMatch '"disabled": true' (Join-Path $data "extensions/installed.json"))) { throw "disabled state not recorded" }
if (-not (Select-String -Quiet -SimpleMatch '"greeting-style": "formal"' (Join-Path $data "extensions/settings.json"))) { throw "setting not saved" }

# After a restart Greeting is no longer in root search: root looks exactly as
# it did before the settings sample was installed. Enabling the package again
# brings it back with its setting: "Greet me" answers in the saved formal
# style, where without a saved style it reports an error.
$process = Start-Pane "stderr-reenable.log"
Capture "18-restarted-disabled.png"
Check "18-restarted-disabled.png" "8a96a3"
python "$PSScriptRoot/check_screenshot.py" --same (Join-Path $OutDir "12-restarted.png") (Join-Path $OutDir "18-restarted-disabled.png")
if ($LASTEXITCODE -ne 0) { throw "screenshot check failed: root after the restart lists the disabled package" }
Send "{DOWN 10}"
Send "{ENTER}"; Start-Sleep -Seconds 1
Send "{DOWN}{ENTER}"; Start-Sleep -Seconds 2
Capture "19-enabled.png"
Check "19-enabled.png" "9fd8a8"   # "Enabled Settings sample"
Send "{ESC}"; Start-Sleep -Seconds 1
Send "{DOWN 4}"   # Greeting
Send "{ENTER}"; Start-Sleep -Seconds 3
Send "{DOWN}{DOWN}{ENTER}"; Start-Sleep -Seconds 2   # "Greet me"
Capture "20-greeted.png"
Check "20-greeted.png" "9fd8a8"   # "Good day to you"
Stop-Pane $process

# Restarted, root lists Greeting again, after Rust sample.
$process = Start-Pane "stderr-color.log"

# The Rust command's color picker (its sixth item), which the guest draws:
# Right chooses purple, and a click on the dark green swatch chooses it. The
# chosen color fills its swatch and the preview, far more pixels than any
# other swatch covers.
Send "{ENTER}"; Start-Sleep -Seconds 3
Send "{DOWN}{DOWN}{DOWN}{DOWN}{DOWN}{ENTER}"; Start-Sleep -Seconds 2
Capture "21-color.png"
Check "21-color.png" "1e88e5" 3000   # blue, chosen when the view opens
Send "{RIGHT}"; Start-Sleep -Seconds 1
Capture "22-color-key.png"
Check "22-color-key.png" "8e24aa" 3000   # purple
$x, $y = Locate "22-color-key.png" "1b5e20"
Click-At $x $y; Start-Sleep -Seconds 1
Capture "23-color-click.png"
Check "23-color-click.png" "1b5e20" 3000   # dark green
Send "{ESC}{ESC}"; Start-Sleep -Seconds 1
Stop-Pane $process

# Reload a development package while Pane stays open. Its command starts as
# the Rust sample; a new build of it is the JavaScript sample. Root lists the
# three samples, Rust sample, Greeting, Dev sample, the install row, then
# Manage extensions... last; the extension list holds Rust sample, Settings
# sample, Dev, then Reload Rust sample, Reload Settings sample, Reload Dev.
$dev = Join-Path $OutDir "dev"
New-Item -ItemType Directory -Force -Path $dev | Out-Null
Copy-Item "target/guests/sample_rust.wasm" (Join-Path $dev "command.wasm")
@'
{
  "manifestVersion": 1,
  "title": "Dev",
  "apiVersion": "0.1",
  "commands": [{ "id": "sample", "title": "Dev sample", "component": "command.wasm" }]
}
'@ | Set-Content -Encoding ascii (Join-Path $dev "pane.json")
$process = Start-Pane "stderr-reload.log" @("--install", $dev)
Send "{ENTER}"; Start-Sleep -Seconds 2   # Install; Dev sample is selected
Send "{ENTER}"; Start-Sleep -Seconds 3
Send "{ENTER}"; Start-Sleep -Seconds 2   # "Say hello"
Capture "24-dev-before.png"
Check "24-dev-before.png" "9fd8a8"   # "Hello from the Rust guest"
Send "{ESC}"; Start-Sleep -Seconds 1
Copy-Item -Force "target/guests/sample_js.wasm" (Join-Path $dev "command.wasm")
Send "{DOWN 10}"   # the last row
Send "{ENTER}"; Start-Sleep -Seconds 1
Send "{DOWN 5}"   # Reload Dev
Send "{ENTER}"; Start-Sleep -Seconds 3
Capture "25-reloaded.png"
Check "25-reloaded.png" "9fd8a8"   # "Reloaded Dev"
Send "{ESC}"; Start-Sleep -Seconds 1
Send "{DOWN 5}"   # Dev sample
Send "{ENTER}"; Start-Sleep -Seconds 3
Send "{ENTER}"; Start-Sleep -Seconds 2   # "Say hello"
Capture "26-dev-after.png"
Check "26-dev-after.png" "9fd8a8"   # "Hello from the JavaScript guest"
python "$PSScriptRoot/check_screenshot.py" --distinct (Join-Path $OutDir "24-dev-before.png") (Join-Path $OutDir "26-dev-after.png")
if ($LASTEXITCODE -ne 0) { throw "screenshot check failed: the reloaded command shows its earlier code" }
Send "{ESC}"; Start-Sleep -Seconds 1

# A build that fails the install checks (here its component is missing) is
# not reloaded: the working code keeps running, exactly as before.
Remove-Item (Join-Path $dev "command.wasm")
Send "{DOWN 10}"
Send "{ENTER}"; Start-Sleep -Seconds 1
Send "{DOWN 5}"
Send "{ENTER}"; Start-Sleep -Seconds 2
Capture "27-not-reloaded.png"
Check "27-not-reloaded.png" "f08c8c"   # "Dev was not reloaded: ..."
Send "{ESC}"; Start-Sleep -Seconds 1
Send "{DOWN 5}"
Send "{ENTER}"; Start-Sleep -Seconds 3
Send "{ENTER}"; Start-Sleep -Seconds 2
Capture "28-still-running.png"
python "$PSScriptRoot/check_screenshot.py" --same (Join-Path $OutDir "26-dev-after.png") (Join-Path $OutDir "28-still-running.png")
if ($LASTEXITCODE -ne 0) { throw "screenshot check failed: a build that failed its checks replaced the working code" }
Send "{ESC}"; Start-Sleep -Seconds 1

# A build whose start fails is reported with Retry, after Reload Dev; this
# one saves a setting and fails its first start only, so Retry starts it.
Copy-Item "target/guests/failing_start.wasm" (Join-Path $dev "command.wasm")
Send "{DOWN 10}"
Send "{ENTER}"; Start-Sleep -Seconds 1
Send "{DOWN 5}"
Send "{ENTER}"; Start-Sleep -Seconds 3
Capture "29-start-failed.png"
Check "29-start-failed.png" "f08c8c"   # "Reloaded Dev, but it failed to start; ..."
Send "{DOWN}{ENTER}"; Start-Sleep -Seconds 3   # Retry starting Dev
Capture "30-retried.png"
Check "30-retried.png" "9fd8a8"   # "Started Dev"
Stop-Pane $process
if (-not (Select-String -Quiet -SimpleMatch '"start-attempted": "yes"' (Join-Path $data "extensions/settings.json"))) { throw "the failed start's setting was not kept" }
Write-Output "screenshots in $OutDir"
