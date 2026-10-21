// Rola a lista do Ajustes > Armazenamento para a linha "Dados do Sistema" aparecer na captura
// (CI, research/mapa-de-fontes.md §7.1). Só eventos de mouse sintéticos.
import CoreGraphics
import Foundation

let p = CGPoint(x: 620, y: 560)
CGEvent(mouseEventSource: nil, mouseType: .mouseMoved, mouseCursorPosition: p, mouseButton: .left)?.post(tap: .cghidEventTap)
usleep(300_000)
for _ in 0..<15 {
    let e = CGEvent(scrollWheelEvent2Source: nil, units: .line, wheelCount: 1, wheel1: -10, wheel2: 0, wheel3: 0)
    e?.location = p
    e?.post(tap: .cghidEventTap)
    usleep(100_000)
}
