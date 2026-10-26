// swift-tools-version: 6.2
// App SDTally: SwiftUI que chama o binário `sdtally` embutido e lê o JSON dele.
// Montar o .app: app/montar.sh (compila o sdtally, o app e o helper root, e assina).
import PackageDescription

let package = Package(
    name: "SDTally",
    // Os textos da interface estão em pt-BR (a chave é o próprio texto); as outras línguas vêm do
    // Sources/SDTally/Localizable.xcstrings. O montar.sh copia os .lproj para o bundle principal.
    defaultLocalization: "pt-BR",
    platforms: [.macOS(.v15)],
    targets: [
        .target(name: "SDTallyXPC", path: "Sources/SDTallyXPC"),
        // Instalar uma versão nova, sem rede: versão, assinatura, hdiutil, a troca do bundle e os
        // argumentos do executor. Usada pelo app e pelo SDTallyUpdater.
        .target(name: "SDTallyInstalacao", path: "Sources/SDTallyInstalacao"),
        .executableTarget(
            name: "SDTally",
            dependencies: ["SDTallyXPC", "SDTallyInstalacao"],
            path: "Sources/SDTally",
            resources: [.process("Localizable.xcstrings")],
            swiftSettings: [.defaultIsolation(MainActor.self)]
        ),
        // Helper root (LaunchDaemon via SMAppService): vai em Contents/MacOS/SDTallyHelper.
        .executableTarget(
            name: "SDTallyHelper",
            dependencies: ["SDTallyXPC"],
            path: "Sources/SDTallyHelper"
        ),
        // Executor da atualização, sem rede: vai em Contents/Helpers/SDTallyUpdater. O app o copia
        // para a pasta temporária e o abre só depois do clique em Instalar. A rede do app fica só em
        // Sources/SDTally/Atualizacao.
        .executableTarget(
            name: "SDTallyUpdater",
            dependencies: ["SDTallyInstalacao"],
            path: "Sources/SDTallyUpdater"
        ),
        .testTarget(
            name: "SDTallyTests",
            dependencies: ["SDTally", "SDTallyXPC"],
            resources: [.copy("Fixtures")]
        ),
        .testTarget(
            name: "SDTallyUpdaterTests",
            dependencies: ["SDTallyInstalacao"]
        ),
    ]
)
