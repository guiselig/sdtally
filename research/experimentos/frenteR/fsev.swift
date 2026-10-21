// fsev: histórico do FSEvents para a Frente R (atualização incremental). Só leitura.
// uso: fsev agora                  -> event id atual
//      fsev desde <id> <caminho>   -> "<flags hex> <pasta>" de cada evento depois de <id>, até o fim do histórico
// Compilar: swiftc -O fsev.swift -o fsev
import CoreServices
import Foundation

let a = CommandLine.arguments
if a.count == 2 && a[1] == "agora" {
    print(FSEventsGetCurrentEventId())
    exit(0)
}
guard a.count == 4, a[1] == "desde", let desde = UInt64(a[2]) else {
    FileHandle.standardError.write("uso: fsev agora | fsev desde <id> <caminho>\n".data(using: .utf8)!)
    exit(2)
}

let cb: FSEventStreamCallback = { _, _, n, paths, flags, _ in
    let ps = unsafeBitCast(paths, to: NSArray.self) as! [String]
    for i in 0..<n {
        if flags[i] & UInt32(kFSEventStreamEventFlagHistoryDone) != 0 {
            exit(0)
        }
        print("\(String(flags[i], radix: 16)) \(ps[i])")
    }
}
var ctx = FSEventStreamContext(version: 0, info: nil, retain: nil, release: nil, copyDescription: nil)
let flags = FSEventStreamCreateFlags(kFSEventStreamCreateFlagUseCFTypes | kFSEventStreamCreateFlagNoDefer)
guard let s = FSEventStreamCreate(nil, cb, &ctx, [a[3]] as CFArray, desde, 0.2, flags) else { exit(1) }
FSEventStreamSetDispatchQueue(s, DispatchQueue.main)
FSEventStreamStart(s)
dispatchMain()
