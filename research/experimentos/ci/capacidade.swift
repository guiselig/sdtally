// Capacidade do volume "/" pelas chaves do Foundation, para achar de onde vem o "usado" do
// cabeçalho do Ajustes > Armazenamento (CI, research/mapa-de-fontes.md §1). Uma linha por chamada.
import Foundation

let k: Set<URLResourceKey> = [.volumeTotalCapacityKey, .volumeAvailableCapacityKey,
                              .volumeAvailableCapacityForImportantUsageKey, .volumeAvailableCapacityForOpportunisticUsageKey]
let v = try URL(fileURLWithPath: "/").resourceValues(forKeys: k)
print(ISO8601DateFormatter().string(from: Date()), "total", v.volumeTotalCapacity ?? -1,
      "livre", v.volumeAvailableCapacity ?? -1,
      "importante", v.volumeAvailableCapacityForImportantUsage ?? -1,
      "oportunista", v.volumeAvailableCapacityForOpportunisticUsage ?? -1)
