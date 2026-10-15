# Drives a real ping gesture (default Alt trigger) over a target window and checks that the
# wheel and ping reach the screen and the click is swallowed. -TopMost puts an always-on-top
# window under the gesture, which must not hide the overlay.
param([string]$Exe, [string]$Dir, [switch]$TopMost)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System; using System.Collections.Generic; using System.Drawing; using System.Drawing.Imaging;
using System.Runtime.InteropServices;
public static class G {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  [StructLayout(LayoutKind.Sequential)] struct MOUSEINPUT { public int dx, dy; public uint data, flags, time; public IntPtr extra; }
  [StructLayout(LayoutKind.Sequential)] struct KEYBDINPUT { public ushort vk, scan; public uint flags, time; public IntPtr extra; }
  [StructLayout(LayoutKind.Explicit)] struct UNION { [FieldOffset(0)] public MOUSEINPUT m; [FieldOffset(0)] public KEYBDINPUT k; }
  [StructLayout(LayoutKind.Sequential)] struct INPUT { public int type; public UNION u; }
  [DllImport("user32.dll")] static extern uint SendInput(uint n, INPUT[] i, int size);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern IntPtr FindWindowExW(IntPtr p, IntPtr a, string c, string t);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] static extern int GetSystemMetrics(int i);
  [DllImport("user32.dll")] static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint m, UIntPtr w, IntPtr l);
  [DllImport("user32.dll")] static extern bool GetCursorPos(out POINT p);
  [DllImport("user32.dll")] static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] static extern IntPtr GetDC(IntPtr h);
  [DllImport("user32.dll")] static extern int ReleaseDC(IntPtr h, IntPtr dc);
  [DllImport("gdi32.dll")] static extern bool BitBlt(IntPtr d, int x, int y, int w, int h, IntPtr s, int sx, int sy, int rop);
  public static int Diff(string a, string b) {
    int n = 0;
    using (var x = new Bitmap(a)) using (var y = new Bitmap(b))
      for (int i = 0; i < x.Width; i++) for (int j = 0; j < x.Height; j++) if (x.GetPixel(i, j) != y.GetPixel(i, j)) n++;
    return n;
  }
  static void Aware() { SetThreadDpiAwarenessContext((IntPtr)(-4)); }
  public static IntPtr Find(string cls, string title, uint pid) {
    IntPtr h = IntPtr.Zero;
    while ((h = FindWindowExW(IntPtr.Zero, h, cls, title)) != IntPtr.Zero) { uint p; GetWindowThreadProcessId(h, out p); if (p == pid) return h; }
    return IntPtr.Zero;
  }
  public static int[] Center(IntPtr h) { Aware(); RECT r; GetWindowRect(h, out r); return new int[] { (r.L + r.R) / 2, (r.T + r.B) / 2 }; }
  public static int[] Cursor() { Aware(); POINT p; GetCursorPos(out p); return new int[] { p.X, p.Y }; }
  public static void Restore(int x, int y) { Aware(); SetCursorPos(x, y); }
  static void Send(INPUT i) { if (SendInput(1, new INPUT[] { i }, Marshal.SizeOf(typeof(INPUT))) != 1) throw new Exception("SendInput failed"); }
  public static void Key(ushort vk, bool up) { var i = new INPUT(); i.type = 1; i.u.k.vk = vk; i.u.k.flags = up ? 2u : 0u; Send(i); }
  // MOUSEEVENTF_MOVE | ABSOLUTE | VIRTUALDESK, plus optional button flags.
  public static void Mouse(int x, int y, uint buttons) {
    Aware();
    int vl = GetSystemMetrics(76), vt = GetSystemMetrics(77), vw = GetSystemMetrics(78), vh = GetSystemMetrics(79);
    var i = new INPUT(); i.type = 0;
    i.u.m.dx = (int)Math.Round((x - vl) * 65535.0 / (vw - 1)); i.u.m.dy = (int)Math.Round((y - vt) * 65535.0 / (vh - 1));
    i.u.m.flags = 0x0001u | 0x8000u | 0x4000u | buttons; Send(i);
  }
  public static void Shot(int cx, int cy, int size, string path) {
    Aware();
    using (var b = new Bitmap(size, size)) {
      using (var g = Graphics.FromImage(b)) {
        // CAPTUREBLT so the layered overlay window is included.
        IntPtr screen = GetDC(IntPtr.Zero), dc = g.GetHdc();
        BitBlt(dc, 0, 0, size, size, screen, cx - size / 2, cy - size / 2, 0x00CC0020 | 0x40000000);
        g.ReleaseHdc(dc); ReleaseDC(IntPtr.Zero, screen);
      }
      b.Save(path, ImageFormat.Png);
    }
  }
}
'@
function Check($ok, $what) { if (-not $ok) { throw "FAIL: $what" }; Write-Output "ok   $what" }
function Pump($ms) { $end = [DateTime]::Now.AddMilliseconds($ms); while ([DateTime]::Now -lt $end) { [System.Windows.Forms.Application]::DoEvents(); Start-Sleep -Milliseconds 10 } }

$settings = Join-Path $env:APPDATA 'lolPing\settings.json'
function Hash { if (Test-Path $settings) { (Get-FileHash $settings).Hash } }
$before = Hash
$cursor = [G]::Cursor()
$p = Start-Process $Exe -PassThru
# A plain, non-fullscreen window under the gesture catches any click Pingloom fails to swallow.
$form = New-Object System.Windows.Forms.Form
$form.Text = 'Pingloom gesture target'; $form.StartPosition = 'CenterScreen'; $form.Width = 700; $form.Height = 500; $form.TopMost = $TopMost
$script:leaked = 0
$form.Add_MouseDown({ $script:leaked++ })
try {
  Start-Sleep 8
  Check (-not $p.HasExited) "running after 8 s (pid $($p.Id))"
  $overlay = [G]::Find('pingloom-overlay', [NullString]::Value, $p.Id)
  Check ($overlay -ne [IntPtr]::Zero) 'overlay window exists'
  Check (-not [G]::IsWindowVisible($overlay)) 'overlay hidden while idle'
  $form.Show(); $form.Activate(); Pump 800
  $c = [G]::Center($form.Handle); $x = $c[0]; $y = $c[1]
  [G]::Shot($x, $y, 420, (Join-Path $Dir 'idle.png'))

  # Ping gesture: hold Alt, press left button, drag right past the threshold, release.
  [G]::Mouse($x, $y, 0); Pump 100
  [G]::Key(0x12, $false); Pump 80
  [G]::Mouse($x, $y, 0x0002); Pump 80
  foreach ($i in 1..12) { [G]::Mouse($x + 10 * $i, $y, 0); Pump 25 }
  Pump 350
  Check ([G]::IsWindowVisible($overlay)) 'overlay shown with the radial wheel during the drag'
  [G]::Shot($x, $y, 420, (Join-Path $Dir 'wheel.png'))
  $changed = [G]::Diff((Join-Path $Dir 'idle.png'), (Join-Path $Dir 'wheel.png'))
  Check ($changed -gt 50000) "wheel visible on screen ($changed pixels changed, target TopMost=$TopMost)"
  [G]::Mouse($x + 120, $y, 0x0004); Pump 50
  [G]::Key(0x12, $true); Pump 250
  Check ([G]::IsWindowVisible($overlay)) 'overlay shows the ping after release'
  [G]::Shot($x, $y, 420, (Join-Path $Dir 'ping.png'))
  $changed = [G]::Diff((Join-Path $Dir 'idle.png'), (Join-Path $Dir 'ping.png'))
  Check ($changed -gt 2000) "ping visible on screen ($changed pixels changed)"
  Pump 300
  Check ($script:leaked -eq 0) 'gesture click swallowed (target window saw no mouse-down)'
  Check (-not $p.HasExited) 'still running after the gesture'

  # Control: without Alt, a click must reach the target window.
  [G]::Mouse($x, $y, 0x0002); Pump 50; [G]::Mouse($x, $y, 0x0004); Pump 300
  Check ($script:leaked -eq 1) 'plain click passes through to the target window'

  $main = [G]::Find('pingloom-main', 'Pingloom', $p.Id)
  [void][G]::PostMessageW($main, 0x10, [UIntPtr]::Zero, [IntPtr]::Zero)
  Check ($p.WaitForExit(10000)) 'exits within 10 s of WM_CLOSE'
  Check ($p.ExitCode -eq 0) "exit code $($p.ExitCode)"
} finally {
  [G]::Key(0x12, $true)
  $form.Close()
  [G]::Restore($cursor[0], $cursor[1])
  if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force }
  Check ((Hash) -eq $before) 'settings.json unchanged'
}
