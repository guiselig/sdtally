// Frente F: estado de sync por arquivo via NSURL ubiquitous keys, sem baixar nada.
// Uso: swift ubiq_keys.swift <raiz>   (ou compilar: swiftc -O ubiq_keys.swift -o ubiq_keys)
// Saída TSV em stdout: status  uploaded  uploading  downloading  conflicts  excluded  alloc  path
// - setiopolicy_np(MATERIALIZE_DATALESS_FILES_OFF) no processo.
// - Não desce em pasta com SF_DATALESS (listar pede materialização ao File Provider).
// - Resource values são metadados; nenhuma chamada abre o conteúdo do arquivo.
import Foundation
import Darwin

let IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES: Int32 = 3
let IOPOL_SCOPE_PROCESS: Int32 = 0
let IOPOL_MATERIALIZE_DATALESS_FILES_OFF: Int32 = 1
precondition(setiopolicy_np(IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES, IOPOL_SCOPE_PROCESS, IOPOL_MATERIALIZE_DATALESS_FILES_OFF) == 0)
let SF_DATALESS: UInt32 = 0x4000_0000

let root = URL(fileURLWithPath: CommandLine.arguments[1])
let keys: [URLResourceKey] = [
    .isDirectoryKey, .isRegularFileKey, .totalFileAllocatedSizeKey,
    .ubiquitousItemDownloadingStatusKey, .ubiquitousItemIsUploadedKey, .ubiquitousItemIsUploadingKey,
    .ubiquitousItemIsDownloadingKey, .ubiquitousItemHasUnresolvedConflictsKey, .ubiquitousItemIsExcludedFromSyncKey,
]
let en = FileManager.default.enumerator(at: root, includingPropertiesForKeys: keys, options: [], errorHandler: { url, err in
    FileHandle.standardError.write("erro \(url.path): \(err.localizedDescription)\n".data(using: .utf8)!)
    return true
})!
func b(_ v: Bool?) -> String { v.map { $0 ? "1" : "0" } ?? "-" }
for case let url as URL in en {
    var st = stat()
    if lstat(url.path, &st) == 0, (st.st_mode & S_IFMT) == S_IFDIR, st.st_flags & SF_DATALESS != 0 {
        en.skipDescendants()
        print("DATALESS_DIR\t-\t-\t-\t-\t-\t0\t\(url.path)")
        continue
    }
    guard let v = try? url.resourceValues(forKeys: Set(keys)), v.isRegularFile == true else { continue }
    let status = v.ubiquitousItemDownloadingStatus?.rawValue ?? "-"
    print("\(status)\t\(b(v.ubiquitousItemIsUploaded))\t\(b(v.ubiquitousItemIsUploading))\t\(b(v.ubiquitousItemIsDownloading))\t\(b(v.ubiquitousItemHasUnresolvedConflicts))\t\(b(v.ubiquitousItemIsExcludedFromSync))\t\(v.totalFileAllocatedSize ?? 0)\t\(url.path)")
}
