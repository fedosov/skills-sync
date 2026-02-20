import SwiftUI

struct AppTextSpec: Equatable {
    let textStyle: Font.TextStyle
    let weight: Font.Weight?
    let monospaced: Bool
}

enum AppTextRole {
    case title
    case sectionHeader
    case body
    case secondary
    case meta
    case pathMono

    var spec: AppTextSpec {
        switch self {
        case .title:
            AppTextSpec(textStyle: .title2, weight: .semibold, monospaced: false)
        case .sectionHeader:
            AppTextSpec(textStyle: .title3, weight: .semibold, monospaced: false)
        case .body:
            AppTextSpec(textStyle: .body, weight: nil, monospaced: false)
        case .secondary:
            AppTextSpec(textStyle: .body, weight: nil, monospaced: false)
        case .meta:
            AppTextSpec(textStyle: .footnote, weight: nil, monospaced: false)
        case .pathMono:
            AppTextSpec(textStyle: .callout, weight: nil, monospaced: true)
        }
    }
}

extension Font {
    static func app(_ role: AppTextRole) -> Font {
        let spec = role.spec
        var font = Font.system(spec.textStyle)
        if let weight = spec.weight {
            font = font.weight(weight)
        }
        if spec.monospaced {
            font = font.monospaced()
        }
        return font
    }
}
