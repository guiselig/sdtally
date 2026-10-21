import Foundation
let u = URL(fileURLWithPath: "/System/Volumes/Data")
let v = try u.resourceValues(forKeys: [.volumeTotalCapacityKey, .volumeAvailableCapacityKey, .volumeAvailableCapacityForImportantUsageKey, .volumeAvailableCapacityForOpportunisticUsageKey])
let g = { (x: Int64) in String(format: "%.2f GiB", Double(x) / 1073741824) }
print("total", g(Int64(v.volumeTotalCapacity!)))
print("livre (statfs)", g(Int64(v.volumeAvailableCapacity!)))
print("livre p/ uso importante", g(v.volumeAvailableCapacityForImportantUsage!))
print("livre p/ oportunista", g(v.volumeAvailableCapacityForOpportunisticUsage!))
print("purgeable ≈", g(v.volumeAvailableCapacityForImportantUsage! - Int64(v.volumeAvailableCapacity!)))
