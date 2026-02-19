import Foundation
import SwiftUI

struct SyncStatusPresentation {
    let title: String
    let subtitle: String
    let symbol: String
    let tint: Color
    let accessibilityLabel: String
}

enum InlineBannerRole: String, Equatable {
    case info
    case success
    case warning
    case error

    var tint: Color {
        switch self {
        case .info:
            return .blue
        case .success:
            return .green
        case .warning:
            return .orange
        case .error:
            return .red
        }
    }
}

struct InlineBannerPresentation: Equatable {
    let title: String
    let message: String
    let symbol: String
    let role: InlineBannerRole
    let recoveryActionTitle: String?

    static func syncFailure(errorDetails: String?) -> InlineBannerPresentation {
        let recovery = "Try Sync now. If this persists, open the app for details."
        let detailText = errorDetails?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let message = detailText.isEmpty ? recovery : "\(detailText) \(recovery)"

        return InlineBannerPresentation(
            title: "Sync couldn't complete.",
            message: message,
            symbol: "exclamationmark.triangle.fill",
            role: .error,
            recoveryActionTitle: "Sync now"
        )
    }

    static func commandResult(_ result: CommandResult) -> InlineBannerPresentation {
        let normalized = result.status.lowercased()
        if normalized.contains("ok") || normalized.contains("success") {
            return InlineBannerPresentation(
                title: "Last action succeeded",
                message: result.message,
                symbol: "checkmark.circle.fill",
                role: .success,
                recoveryActionTitle: nil
            )
        }

        if normalized.contains("fail") || normalized.contains("error") {
            return InlineBannerPresentation(
                title: "Last action needs attention",
                message: result.message,
                symbol: "exclamationmark.triangle.fill",
                role: .error,
                recoveryActionTitle: nil
            )
        }

        return InlineBannerPresentation(
            title: "Last action update",
            message: result.message,
            symbol: "info.circle.fill",
            role: .info,
            recoveryActionTitle: nil
        )
    }
}

enum ScopeFilter: String, CaseIterable, Identifiable {
    case all
    case global
    case project

    var id: String { rawValue }

    var title: String {
        switch self {
        case .all:
            return "All"
        case .global:
            return "Global"
        case .project:
            return "Project"
        }
    }

    func includes(_ skill: SkillRecord) -> Bool {
        switch self {
        case .all:
            return true
        case .global:
            return skill.scope == "global"
        case .project:
            return skill.scope == "project"
        }
    }
}

enum SyncFormatting {
    static func relativeTime(_ iso: String?, relativeTo referenceDate: Date = Date()) -> String {
        guard let iso else {
            return "Never synced"
        }

        let parser = ISO8601DateFormatter()
        guard let date = parser.date(from: iso) else {
            return "Time unavailable"
        }

        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: date, relativeTo: referenceDate)
    }

    static func updatedLine(_ iso: String?, relativeTo referenceDate: Date = Date()) -> String {
        "Updated \(relativeTime(iso, relativeTo: referenceDate))"
    }
}

extension SyncHealthStatus {
    var presentation: SyncStatusPresentation {
        switch self {
        case .ok:
            return SyncStatusPresentation(
                title: "Healthy",
                subtitle: "Everything is syncing normally.",
                symbol: "checkmark.circle.fill",
                tint: .green,
                accessibilityLabel: "Healthy sync status"
            )
        case .failed:
            return SyncStatusPresentation(
                title: "Needs attention",
                subtitle: "A sync run failed and may need retry.",
                symbol: "exclamationmark.triangle.fill",
                tint: .red,
                accessibilityLabel: "Sync needs attention"
            )
        case .syncing:
            return SyncStatusPresentation(
                title: "Sync in progress",
                subtitle: "A sync run is currently active.",
                symbol: "arrow.triangle.2.circlepath",
                tint: .orange,
                accessibilityLabel: "Sync in progress"
            )
        case .unknown:
            return SyncStatusPresentation(
                title: "Waiting for first sync",
                subtitle: "Run Sync now to establish current state.",
                symbol: "clock.badge.questionmark",
                tint: .gray,
                accessibilityLabel: "Waiting for first sync"
            )
        }
    }
}
