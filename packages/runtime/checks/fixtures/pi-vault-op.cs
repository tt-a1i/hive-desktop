using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Text;
using System.Threading;
using System.Web.Script.Serialization;

public static class PiVaultOpFixture
{
    public static int Main(string[] args)
    {
        string root = Directory.GetParent(AppDomain.CurrentDomain.BaseDirectory.TrimEnd(Path.DirectorySeparatorChar)).FullName;
        var json = new JavaScriptSerializer();
        var state = json.Deserialize<Dictionary<string, string>>(File.ReadAllText(Path.Combine(root, "command.json")));
        File.AppendAllText(Path.Combine(root, "command.jsonl"),
            json.Serialize(new { pid = Process.GetCurrentProcess().Id, args = args }) + "\n");
        Console.OutputEncoding = new UTF8Encoding(false);
        switch (state["mode"])
        {
            case "hold": Thread.Sleep(Timeout.Infinite); return 0;
            case "deferred":
                while (!File.Exists(Path.Combine(root, "release"))) Thread.Sleep(20);
                Console.Write(state["key"]);
                return 0;
            case "fail": Console.Error.Write(state["key"]); return 1;
            default: Console.Write(state["key"]); return 0;
        }
    }
}
