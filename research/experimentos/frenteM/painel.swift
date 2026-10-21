// painel: reproduz, sem root, dois números do painel Ajustes > Geral > Armazenamento (Storage.appex):
//   1. "macOS"               = Σ CapacityInUse de todos os volumes APFS do disco interno, menos o volume Data
//   2. "Apple Intelligence"  = STMSize(GenerativeModels/) + STMSize(Visual/), que no disco é
//                              dir-stats(GenerativeModels) + dir-stats(Visual) + Σ dir-stats(<asset>/.AssetData)
// Só leitura. dir-stats = fsctl(path, 0xC1104A71, buf zerado): buf+4 != 0 ESCREVE no volume, nunca mexer.
// Não abre nem lista pasta nenhuma; os IDs dos assets vêm do log do mobileassetd ("AssetGC determine").
// uso: swiftc -O painel.swift -o painel && ./painel [horas_de_log=72]
import Foundation

@_silgen_name("fsctl") func fsctl(_ path: UnsafePointer<CChar>, _ req: UInt, _ data: UnsafeMutableRawPointer, _ opt: UInt32) -> Int32

func dirstats(_ p: String) -> UInt64? {
    var b = [UInt8](repeating: 0, count: 0x110)          // get; b[4] fica 0 sempre
    let r = b.withUnsafeMutableBytes { fsctl(p, 0xC1104A71, $0.baseAddress!, 0) }
    if r != 0 { return nil }
    return b.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: 0x40, as: UInt64.self) }
}

func run(_ cmd: String, _ args: [String]) -> Data {
    let p = Process(); p.executableURL = URL(fileURLWithPath: cmd); p.arguments = args
    let o = Pipe(); p.standardOutput = o; p.standardError = FileHandle.nullDevice
    try! p.run(); let d = o.fileHandleForReading.readDataToEndOfFile(); p.waitUntilExit(); return d
}

// 1. macOS
let apfs = try! PropertyListSerialization.propertyList(from: run("/usr/sbin/diskutil", ["apfs", "list", "-plist"]), format: nil) as! [String: Any]
let containers = apfs["Containers"] as! [[String: Any]]
func whole(_ s: String) -> String { s.replacingOccurrences(of: #"s\d+$"#, with: "", options: .regularExpression) }
// disco do container que tem o volume Data (disco interno)
let dataCont = containers.first { ($0["Volumes"] as! [[String: Any]]).contains { ($0["Roles"] as? [String] ?? []).contains("Data") } }!
let disk = whole(dataCont["DesignatedPhysicalStore"] as! String)
var macos: UInt64 = 0
for c in containers where whole(c["DesignatedPhysicalStore"] as! String) == disk {
    var sub: UInt64 = 0
    for v in c["Volumes"] as! [[String: Any]] {
        let roles = v["Roles"] as? [String] ?? [], used = (v["CapacityInUse"] as! NSNumber).uint64Value
        let conta = !roles.contains("Data")
        if conta { sub += used }
        print(String(format: "  %@ %-10@ %15llu %@", v["DeviceIdentifier"] as! String, roles.joined(separator: ",") as NSString, used, conta ? "" : "(fora: Data)"))
    }
    print("  container \(c["ContainerReference"]!): \(sub)")
    macos += sub
}
print("macOS (painel) = \(macos) B = \(String(format: "%.2f", Double(macos) / 1e9)) GB\n")

// 2. Apple Intelligence
let horas = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "72"
let base = "/System/Library/AssetsV2/com_apple_MobileAsset_UAF_FM_"
let pastas = ["GenerativeModels", "Visual"].map { base + $0 }
let log = String(decoding: run("/usr/bin/log", ["show", "--last", "\(horas)h", "--style", "compact", "--predicate",
    "process == \"mobileassetd\" AND eventMessage CONTAINS \"AssetGC determine\" AND eventMessage CONTAINS \"UAF_FM_\""]), as: UTF8.self)
let re = try! NSRegularExpression(pattern: #"/System/Library/AssetsV2/com_apple_MobileAsset_UAF_FM_(GenerativeModels|Visual)/[A-Za-z_]+/[0-9a-f]{40}\.asset"#)
let assets = Set(re.matches(in: log, range: NSRange(log.startIndex..., in: log)).map { String(log[Range($0.range, in: log)!]) }).sorted()
var ai: UInt64 = 0
for p in pastas { let v = dirstats(p) ?? 0; print("  dir-stats \(p): \(v)"); ai += v }
var enx: UInt64 = 0, montados = 0, vazios = 0, ausentes = 0
for a in assets {
    guard FileManager.default.fileExists(atPath: a) else { ausentes += 1; continue }
    guard let v = dirstats(a + "/.AssetData") else { continue }   // sem .AssetData: asset sem cryptex
    if v == 0 { vazios += 1 } else { montados += 1; enx += v }
}
print("  assets no log: \(assets.count) (ausentes no disco: \(ausentes)); .AssetData com conteúdo: \(montados), vazias: \(vazios)")
print("  Σ dir-stats(.AssetData) = \(enx)")
ai += enx
print("Apple Intelligence (painel) = \(ai) B = \(String(format: "%.2f", Double(ai) / 1e9)) GB")
