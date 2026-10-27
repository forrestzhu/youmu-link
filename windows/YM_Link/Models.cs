using System.Text.Json;

namespace YM_Link;

internal static class WireJson
{
    public static readonly JsonSerializerOptions Options = new() { PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower };
    public static string Command(string action, params (string Key, object Value)[] fields)
    {
        var values = fields.ToDictionary(field => field.Key, field => field.Value);
        values["action"] = action;
        return JsonSerializer.Serialize(values);
    }
}

internal sealed record Snapshot
{
    public string DeviceName { get; init; } = "";
    public string Status { get; init; } = "starting";
    public ushort ListenPort { get; init; }
    public ushort DiscoveryPort { get; init; }
    public List<Peer> Peers { get; init; } = [];
    public Pairing? Pairing { get; init; }
    public List<Display> Displays { get; init; } = [];
    public Mirror Mirror { get; init; } = new();
    public List<Share> Shares { get; init; } = [];
    public List<Transfer> Transfers { get; init; } = [];
    public ulong ErrorId { get; init; }
    public string? Error { get; init; }
}

internal sealed record Peer
{
    public string Id { get; init; } = "";
    public string Address { get; init; } = "";
    public string Name { get; init; } = "";
    public string Platform { get; init; } = "";
    public string Status { get; init; } = "";
    public bool Trusted { get; init; }
}
internal sealed record Pairing
{
    public string PeerName { get; init; } = "";
    public string? Code { get; init; }
    public ulong ExpiresIn { get; init; }
    public string Mode { get; init; } = "";
    public static bool IsValidCode(string code) => code.Length == 6 && code.All(c => c is >= '0' and <= '9');
}
internal sealed record Display
{
    public uint Id { get; init; }
    public string Name { get; init; } = "";
    public uint Width { get; init; }
    public uint Height { get; init; }
}
internal sealed record Mirror
{
    public string Status { get; init; } = "stopped";
    public string Permission { get; init; } = "unknown";
    public uint? DisplayId { get; init; }
    public bool Audio { get; init; } = true;
    public string Quality { get; init; } = "balanced";
    public double? Fps { get; init; }
    public double? BitrateMbps { get; init; }
    public double? LatencyMs { get; init; }
    public string? Error { get; init; }
    public bool IsActive => Status is "starting" or "running" or "paused";
}
internal sealed record Share
{
    public string Id { get; init; } = "";
    public string Name { get; init; } = "";
    public string Path { get; init; } = "";
}
internal sealed record Transfer
{
    public string Id { get; init; } = "";
    public string Name { get; init; } = "";
    public ulong Transferred { get; init; }
    public ulong Total { get; init; }
    public string Status { get; init; } = "";
    public double Fraction => Total == 0 ? 0 : Math.Min(1, (double)Transferred / Total);
}
internal sealed record CommandResult(bool Ok, string? Error);
internal sealed class ErrorCursor
{
    private ulong last;
    public string? Consume(Snapshot snapshot)
    {
        if (snapshot.ErrorId == last) return null;
        last = snapshot.ErrorId;
        return snapshot.Error;
    }
}
