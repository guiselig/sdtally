// Logo do GitHub: path do ícone `mark-github-16` do Primer Octicons 19.40.0 (GitHub Inc., MIT), arcos do SVG convertidos em curvas; segue a cor do texto.
import SwiftUI

nonisolated struct LogoGithub: Shape {
    func path(in rect: CGRect) -> Path {
        let k = min(rect.width, rect.height) / 16
        let (dx, dy) = (rect.midX - 8 * k, rect.midY - 8 * k)
        func P(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: dx + x * k, y: dy + y * k) }
        var p = Path()
        p.move(to: P(6.766, 11.328))
        p.addCurve(to: P(3.25, 7.672), control1: P(4.703, 11.078), control2: P(3.25, 9.594))
        p.addCurve(to: P(4, 5.484), control1: P(3.25, 6.891), control2: P(3.531, 6.047))
        p.addCurve(to: P(4.063, 3.422), control1: P(3.797, 4.969), control2: P(3.828, 3.875))
        p.addCurve(to: P(6.031, 4.125), control1: P(4.688, 3.344), control2: P(5.531, 3.672))
        p.addCurve(to: P(8.016, 3.844), control1: P(6.625, 3.938), control2: P(7.25, 3.844))
        p.addCurve(to: P(9.969, 4.109), control1: P(8.781, 3.844), control2: P(9.406, 3.938))
        p.addCurve(to: P(11.938, 3.422), control1: P(10.453, 3.672), control2: P(11.313, 3.344))
        p.addCurve(to: P(11.984, 5.469), control1: P(12.156, 3.844), control2: P(12.188, 4.937))
        p.addCurve(to: P(12.75, 7.672), control1: P(12.484, 6.062), control2: P(12.75, 6.859))
        p.addCurve(to: P(9.203, 11.312), control1: P(12.75, 9.594), control2: P(11.297, 11.047))
        p.addCurve(to: P(10.093, 13.266), control1: P(9.734, 11.656), control2: P(10.093, 12.406))
        p.addLine(to: P(10.093, 14.891))
        p.addCurve(to: P(10.953, 15.438), control1: P(10.093, 15.359), control2: P(10.484, 15.625))
        p.addCurve(to: P(16, 8.03), control1: P(13.781, 14.359), control2: P(16, 11.53))
        p.addCurve(to: P(7.984, 0), control1: P(16, 3.61), control2: P(12.406, 0))
        p.addCurve(to: P(0, 8.031), control1: P(3.563, 0), control2: P(0, 3.61))
        p.addCurve(to: P(5.172, 15.453), control1: P(-0.009, 11.347), control2: P(2.058, 14.314))
        p.addCurve(to: P(6, 14.906), control1: P(5.594, 15.609), control2: P(6, 15.328))
        p.addLine(to: P(6, 13.656))
        p.addCurve(to: P(5.25, 13.812), control1: P(5.781, 13.75), control2: P(5.5, 13.812))
        p.addCurve(to: P(3.172, 12.203), control1: P(4.219, 13.812), control2: P(3.61, 13.25))
        p.addCurve(to: P(2.453, 11.484), control1: P(3, 11.781), control2: P(2.812, 11.531))
        p.addCurve(to: P(2.203, 11.297), control1: P(2.266, 11.469), control2: P(2.203, 11.391))
        p.addCurve(to: P(2.828, 10.969), control1: P(2.203, 11.109), control2: P(2.516, 10.969))
        p.addCurve(to: P(4.078, 11.829), control1: P(3.281, 10.969), control2: P(3.672, 11.25))
        p.addCurve(to: P(5.109, 12.484), control1: P(4.391, 12.281), control2: P(4.718, 12.484))
        p.addCurve(to: P(6.109, 11.984), control1: P(5.5, 12.484), control2: P(5.75, 12.344))
        p.addCurve(to: P(6.766, 11.328), control1: P(6.375, 11.719), control2: P(6.579, 11.484))
        p.closeSubpath()
        return p
    }
}
