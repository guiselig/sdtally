// G9 (spec-fase6 §7): níveis do O que fazer sem vermelho nem laranja onde o sistema cuida ou
// não se mexe. As outras regras (badge, .red, Toggle) estão no scripts/g9.sh.
import SwiftUI
import Testing
@testable import SDTally

@Test func nivelSemVermelhoNemLaranja() {
    for n in ["system_handles", "do_not_touch"] {
        let cor = Marca.nivel(n).cor
        #expect(cor != .red && cor != .orange, "\(n)")
    }
    #expect(Marca.nivel("do_not_touch") == (Color.secondary, "hand.raised"))
}
