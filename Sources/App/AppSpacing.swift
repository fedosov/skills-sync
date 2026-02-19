import SwiftUI

enum AppSpacing {
    static let xs: CGFloat = 4
    static let sm: CGFloat = 8
    static let md: CGFloat = 12
    static let lg: CGFloat = 16
    static let xl: CGFloat = 24
}

extension View {
    func appSectionSpacing() -> some View {
        padding(AppSpacing.lg)
    }
}
