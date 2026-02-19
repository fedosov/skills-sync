import Foundation
import WidgetKit

@MainActor
final class AppViewModel: ObservableObject {
    @Published var state: SyncState = .empty
    @Published var searchText: String = ""
    @Published var scopeFilter: ScopeFilter = .all
    @Published var selectedSkillID: String?
    @Published var alertMessage: String?
    @Published var localBanner: InlineBannerPresentation?

    private let store = SyncStateStore()
    private var timer: Timer?

    var filteredSkills: [SkillRecord] {
        Self.applyFilters(to: state.skills, query: searchText, scopeFilter: scopeFilter)
    }

    nonisolated static func applyFilters(to skills: [SkillRecord], query: String, scopeFilter: ScopeFilter) -> [SkillRecord] {
        let base = skills.sorted { lhs, rhs in
            if lhs.scope != rhs.scope {
                return lhs.scope == "global"
            }
            return lhs.name.localizedCaseInsensitiveCompare(rhs.name) == .orderedAscending
        }

        let scoped = base.filter(scopeFilter.includes)
        let trimmedQuery = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedQuery.isEmpty else {
            return scoped
        }

        return scoped.filter { skill in
            skill.name.localizedCaseInsensitiveContains(trimmedQuery)
                || skill.scope.localizedCaseInsensitiveContains(trimmedQuery)
                || (skill.workspace?.localizedCaseInsensitiveContains(trimmedQuery) ?? false)
                || skill.canonicalSourcePath.localizedCaseInsensitiveContains(trimmedQuery)
        }
    }

    func start() {
        load()
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: 5.0, repeats: true) { [weak self] _ in
            Task { @MainActor in
                self?.load()
            }
        }
    }

    func stop() {
        timer?.invalidate()
        timer = nil
    }

    func load() {
        state = store.loadState()

        if let selectedSkillID, !state.skills.contains(where: { $0.id == selectedSkillID }) {
            self.selectedSkillID = nil
        }
    }

    func refreshSources() {
        load()
        let sourceCount = state.skills.filter { $0.scope == "global" }.count
        localBanner = InlineBannerPresentation(
            title: "Sources refreshed",
            message: "Loaded \(sourceCount) source skills.",
            symbol: "arrow.clockwise.circle.fill",
            role: .info,
            recoveryActionTitle: nil
        )
    }

    func queueSync() {
        queue(type: .syncNow, skill: nil, confirmed: nil)
    }

    func queueOpen(skill: SkillRecord) {
        queue(type: .openInZed, skill: skill, confirmed: nil)
    }

    func queueReveal(skill: SkillRecord) {
        queue(type: .revealInFinder, skill: skill, confirmed: nil)
    }

    func queueDelete(skill: SkillRecord) {
        queue(type: .deleteCanonicalSource, skill: skill, confirmed: true)
    }

    private func queue(type: CommandType, skill: SkillRecord?, confirmed: Bool?) {
        let command = store.makeCommand(
            type: type,
            skill: skill,
            requestedBy: "app",
            confirmed: confirmed
        )

        do {
            try store.appendCommand(command)
            WidgetCenter.shared.reloadAllTimelines()
            localBanner = Self.makeConfirmationBanner(type: type, skill: skill)
        } catch {
            alertMessage = error.localizedDescription
        }
    }

    private nonisolated static func makeConfirmationBanner(type: CommandType, skill: SkillRecord?) -> InlineBannerPresentation {
        switch type {
        case .syncNow:
            return InlineBannerPresentation(
                title: "Sync requested",
                message: "Source skills queued for synchronization to destinations.",
                symbol: "arrow.clockwise.circle.fill",
                role: .success,
                recoveryActionTitle: nil
            )
        case .openInZed:
            return InlineBannerPresentation(
                title: "Open requested",
                message: "\(skill?.name ?? "Source") was queued to open in Zed.",
                symbol: "checkmark.circle.fill",
                role: .success,
                recoveryActionTitle: nil
            )
        case .revealInFinder:
            return InlineBannerPresentation(
                title: "Reveal requested",
                message: "\(skill?.name ?? "Source") was queued to reveal in Finder.",
                symbol: "checkmark.circle.fill",
                role: .success,
                recoveryActionTitle: nil
            )
        case .deleteCanonicalSource:
            return InlineBannerPresentation(
                title: "Delete requested",
                message: "\(skill?.name ?? "Source") was queued to move to Trash.",
                symbol: "checkmark.circle.fill",
                role: .warning,
                recoveryActionTitle: nil
            )
        }
    }
}
