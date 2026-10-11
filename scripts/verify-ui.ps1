param([Parameter(Mandatory = $true)][int]$ProcessId, [switch]$PhysicalHotkey)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
if (-not ('MiniTermUiProbe' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class MiniTermUiProbe {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct Point { public int X,Y; }
    [StructLayout(LayoutKind.Sequential)] public struct MouseInput { public int X,Y; public uint Data,Flags,Time; public UIntPtr Extra; }
    [StructLayout(LayoutKind.Sequential)] public struct KeyInput { public ushort Vk,Scan; public uint Flags,Time; public UIntPtr Extra; }
    [StructLayout(LayoutKind.Explicit)] public struct Union { [FieldOffset(0)] public MouseInput Mouse; [FieldOffset(0)] public KeyInput Key; }
    [StructLayout(LayoutKind.Sequential)] public struct Input { public uint Type; public Union Data; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out Point point);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd,uint message,IntPtr wparam,IntPtr lparam);
    [DllImport("user32.dll",SetLastError=true)] static extern uint SendInput(uint count, Input[] input, int size);
    [DllImport("user32.dll")] static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
    public static Rect Bounds(IntPtr hwnd) { Rect rect; if (!GetWindowRect(hwnd,out rect)) throw new Exception("No window bounds"); return rect; }
    public static void Click(int x,int y) { SetCursorPos(x,y); System.Threading.Thread.Sleep(35); mouse_event(2,0,0,0,UIntPtr.Zero); System.Threading.Thread.Sleep(30); mouse_event(4,0,0,0,UIntPtr.Zero); }
    static Input Key(ushort vk, ushort scan, uint flags) { return new Input { Type=1, Data=new Union { Key=new KeyInput { Vk=vk, Scan=scan, Flags=flags } } }; }
    public static void Text(IntPtr hwnd,string text) {
        if (GetForegroundWindow()!=hwnd) throw new Exception("Target window lost focus; text injection cancelled");
        Input[] input = new Input[text.Length*2];
        for (int i=0;i<text.Length;i++) { input[i*2]=Key(0,text[i],4); input[i*2+1]=Key(0,text[i],6); }
        if (SendInput((uint)input.Length,input,Marshal.SizeOf(typeof(Input)))!=input.Length) throw new Exception("SendInput failed");
    }
    public static void Enter() { Input[] input={Key(13,0,0),Key(13,0,2)}; SendInput(2,input,Marshal.SizeOf(typeof(Input))); }
    static void Send(Input[] input) { uint count=SendInput((uint)input.Length,input,Marshal.SizeOf(typeof(Input))); if (count!=input.Length) throw new Exception("SendInput: "+count+" / "+input.Length+", Win32 error "+Marshal.GetLastWin32Error()); }
    public static void Hotkey() { Send(new[]{Key(17,29,0),Key(18,56,0)}); System.Threading.Thread.Sleep(50); Send(new[]{Key(32,57,0),Key(32,57,2)}); System.Threading.Thread.Sleep(50); Send(new[]{Key(18,56,2),Key(17,29,2)}); }
}
'@
}
$taskApp = Get-Process -Id $ProcessId
if ($taskApp.ProcessName -ne 'miniterm') { throw 'MiniTerm process required' }
$taskWindow = $taskApp.MainWindowHandle
if ($taskWindow -eq [IntPtr]::Zero) { throw 'MiniTerm is not visible' }
$taskPreviousFocus = [MiniTermUiProbe]::GetForegroundWindow()
$taskPreviousPointer = New-Object MiniTermUiProbe+Point
[MiniTermUiProbe]::GetCursorPos([ref]$taskPreviousPointer) | Out-Null
$taskShells = @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$ProcessId AND Name='bash.exe'" | Select-Object -ExpandProperty ProcessId)
if ($taskShells.Count -ne 1) { throw 'Run this probe in a fresh single Bash session' }
$taskMini = [MiniTermUiProbe]::Bounds($taskWindow)
try {
    [MiniTermUiProbe]::Click($taskMini.Left + 20,$taskMini.Top + 55)
    Start-Sleep -Milliseconds 100
    if ([MiniTermUiProbe]::GetForegroundWindow() -ne $taskWindow) { throw 'Single click did not activate MiniTerm' }
    [MiniTermUiProbe]::Text($taskWindow,"HISTFILE=/dev/null; HISTSIZE=0; printf 'GUI_INPUT_OK 한글 GPU\n'")
    [MiniTermUiProbe]::Enter()
    Start-Sleep -Milliseconds 350
    [MiniTermUiProbe]::Click($taskMini.Left + 40,$taskMini.Top + 12)
    Start-Sleep -Milliseconds 80
    [MiniTermUiProbe]::Click($taskMini.Left + 40,$taskMini.Top + 12)
    Start-Sleep -Milliseconds 250
    $taskExpanded = [MiniTermUiProbe]::Bounds($taskWindow)
    if ($taskExpanded.Bottom -ne $taskMini.Bottom -or $taskExpanded.Right -ne $taskMini.Right -or $taskExpanded.Bottom - $taskExpanded.Top -le $taskMini.Bottom - $taskMini.Top) { throw "Expansion bounds: mini=$($taskMini | ConvertTo-Json -Compress) expanded=$($taskExpanded | ConvertTo-Json -Compress)" }
    $taskImage = New-Object System.Drawing.Bitmap ($taskExpanded.Right - $taskExpanded.Left),($taskExpanded.Bottom - $taskExpanded.Top)
    $taskGraphics = [System.Drawing.Graphics]::FromImage($taskImage)
    try {
        $taskGraphics.CopyFromScreen($taskExpanded.Left,$taskExpanded.Top,0,0,$taskImage.Size)
        $taskImage.Save((Join-Path (Split-Path -Parent $PSScriptRoot) 'artifacts/native-ui-expanded.png'))
    } finally { $taskGraphics.Dispose(); $taskImage.Dispose() }
    [MiniTermUiProbe]::Click($taskExpanded.Right - 15,$taskExpanded.Top + 12)
    Start-Sleep -Milliseconds 150
    if ([MiniTermUiProbe]::IsWindowVisible($taskWindow)) { throw 'Hide button did not hide the HUD' }
    foreach ($taskShell in $taskShells) { Get-Process -Id $taskShell | Out-Null }
    if ($PhysicalHotkey) { [MiniTermUiProbe]::Hotkey() }
    else { [MiniTermUiProbe]::PostMessage($taskWindow,0x312,[IntPtr]1,[IntPtr]0x200003) | Out-Null }
    Start-Sleep -Milliseconds 250
    if (-not [MiniTermUiProbe]::IsWindowVisible($taskWindow) -or [MiniTermUiProbe]::GetForegroundWindow() -ne $taskWindow) { throw "Global shortcut: visible=$([MiniTermUiProbe]::IsWindowVisible($taskWindow)) foreground=$([MiniTermUiProbe]::GetForegroundWindow()) expected=$taskWindow" }
    foreach ($taskShell in $taskShells) { Get-Process -Id $taskShell | Out-Null }
    $taskResult = [pscustomobject]@{ singleClickFocus = $true; expansionAnchors = $true; hiddenShellPreserved = $true; hotkeyMessageRestored = $true; physicalHotkeyTested = [bool]$PhysicalHotkey; bashProcessIds = $taskShells; note = 'Default probe posts WM_HOTKEY and verifies its activation route. Physical Ctrl+Alt+Space and real Korean IME composition require separate verification.' }
    $taskJson = $taskResult | ConvertTo-Json
    [System.IO.File]::WriteAllText((Join-Path (Split-Path -Parent $PSScriptRoot) 'artifacts/native-ui.json'),$taskJson,[System.Text.UTF8Encoding]::new($false))
    $taskJson
} finally {
    [MiniTermUiProbe]::SetForegroundWindow($taskPreviousFocus) | Out-Null
    [MiniTermUiProbe]::SetCursorPos($taskPreviousPointer.X,$taskPreviousPointer.Y) | Out-Null
}
