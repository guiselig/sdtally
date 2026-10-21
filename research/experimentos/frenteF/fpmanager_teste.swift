// Testa o que NSFileProviderManager responde a um CLI comum (sem ser app provedor). Uso: swiftc fpmanager_teste.swift -o fpm && ./fpm <arquivo do iCloud>
import Foundation
import FileProvider
let sem = DispatchSemaphore(value: 0)
NSFileProviderManager.getDomainsWithCompletionHandler { d, e in print("getDomains:", d.map { $0.identifier.rawValue }, e.map { "\($0)" } ?? "nil"); sem.signal() }
_ = sem.wait(timeout: .now() + 10)
// Item de um arquivo real do iCloud: getIdentifierForUserVisibleFile não baixa nada.
let url = URL(fileURLWithPath: CommandLine.arguments[1])
NSFileProviderManager.getIdentifierForUserVisibleFile(at: url) { id, dom, e in print("getIdentifier:", id?.rawValue ?? "nil", dom?.rawValue ?? "nil", e.map { "\($0)" } ?? "nil"); sem.signal() }
_ = sem.wait(timeout: .now() + 10)
