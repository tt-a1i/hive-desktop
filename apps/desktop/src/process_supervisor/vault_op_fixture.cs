using System;
using System.Diagnostics;
using System.IO;
using System.Threading;
using System.Runtime.InteropServices;
class VaultFixture {
  [DllImport("kernel32.dll")] static extern IntPtr GetStdHandle(int id);
  [DllImport("kernel32.dll")] static extern uint GetFileType(IntPtr handle);
  static int Main(string[] args) {
    string dir = Environment.GetEnvironmentVariable("HIVE_TEST_VAULT_DIR");
    if (args.Length == 1 && args[0] == "helper") {
      File.WriteAllText(Path.Combine(dir, "helper-ready"), Process.GetCurrentProcess().Id + " " + GetFileType(GetStdHandle(-11)));
      Thread.Sleep(Timeout.Infinite); return 0;
    }
    if (args.Length != 3 || args[0] != "read" || args[1] != "--no-newline" || args[2] != "op://fixture/item/field") return 3;
    var helper = Process.Start(new ProcessStartInfo { FileName = Process.GetCurrentProcess().MainModule.FileName, Arguments = "helper", UseShellExecute = false });
    File.WriteAllText(Path.Combine(dir, "op-pids"), Process.GetCurrentProcess().Id + " " + helper.Id + " " + GetFileType(GetStdHandle(-11)));
    Thread.Sleep(Timeout.Infinite); return 0;
  }
}
