import AppIntents
import SwiftUI
import WidgetKit

struct SkillsWidgetConfigurationIntent: WidgetConfigurationIntent {
    static let title: LocalizedStringResource = "Skills sync"
    static let description = IntentDescription("Shows skill sync health and top discovered skills.")
}

struct SkillsWidgetEntry: TimelineEntry {
    let date: Date
    let state: SyncState
    let topSkills: [SkillRecord]
}

struct SkillsWidgetProvider: AppIntentTimelineProvider {
    typealias Entry = SkillsWidgetEntry
    typealias Intent = SkillsWidgetConfigurationIntent

    func recommendations() -> [AppIntentRecommendation<Intent>] {
        [AppIntentRecommendation(intent: .init(), description: "Default")]
    }

    func placeholder(in context: Context) -> SkillsWidgetEntry {
        SkillsWidgetEntry(date: .now, state: .empty, topSkills: [])
    }

    func snapshot(for configuration: Intent, in context: Context) async -> SkillsWidgetEntry {
        makeEntry()
    }

    func timeline(for configuration: Intent, in context: Context) async -> Timeline<SkillsWidgetEntry> {
        let entry = makeEntry()
        let next = Date().addingTimeInterval(300)
        return Timeline(entries: [entry], policy: .after(next))
    }

    private func makeEntry() -> SkillsWidgetEntry {
        let store = SyncStateStore()
        let state = store.loadState()
        let topSkills = store.topSkills(from: state)
        return SkillsWidgetEntry(date: .now, state: state, topSkills: topSkills)
    }
}

struct SkillsSyncWidget: Widget {
    let kind = "SkillsSyncWidget"

    var body: some WidgetConfiguration {
        AppIntentConfiguration(kind: kind, intent: SkillsWidgetConfigurationIntent.self, provider: SkillsWidgetProvider()) { entry in
            SkillsWidgetView(entry: entry)
                .containerBackground(.background, for: .widget)
        }
        .configurationDisplayName("Skills Sync")
        .description("Sync status, conflicts and top skills")
        .supportedFamilies([.systemLarge])
    }
}

struct SkillsWidgetView: View {
    let entry: SkillsWidgetEntry

    private var status: SyncStatusPresentation {
        entry.state.sync.status.presentation
    }

    private var syncErrorBanner: InlineBannerPresentation? {
        let hasError = !(entry.state.sync.error?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ?? true)
        guard entry.state.sync.status == .failed || hasError else {
            return nil
        }
        return .syncFailure(errorDetails: entry.state.sync.error)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            WidgetStatusHeader(status: status, lastFinishedAt: entry.state.sync.lastFinishedAt)
            WidgetMetricsRow(summary: entry.state.summary)
            TopSkillsSection(skills: Array(entry.topSkills.prefix(5)))

            if let syncErrorBanner {
                WidgetStatusMessage(banner: syncErrorBanner)
            }

            Spacer(minLength: 0)

            WidgetActionRow()
        }
        .padding(12)
    }
}

private struct WidgetStatusHeader: View {
    let status: SyncStatusPresentation
    let lastFinishedAt: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Label(status.title, systemImage: status.symbol)
                .font(.headline)
                .foregroundStyle(status.tint)

            Text(status.subtitle)
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(1)

            Text(SyncFormatting.updatedLine(lastFinishedAt))
                .font(.caption2)
                .foregroundStyle(.secondary)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(status.accessibilityLabel). \(SyncFormatting.updatedLine(lastFinishedAt))")
    }
}

private struct WidgetMetricsRow: View {
    let summary: SyncSummary

    var body: some View {
        HStack {
            LabeledMetric(title: "Global", value: summary.globalCount)
            LabeledMetric(title: "Project", value: summary.projectCount)
            LabeledMetric(title: "Conflicts", value: summary.conflictCount)
        }
    }
}

private struct LabeledMetric: View {
    let title: String
    let value: Int

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title)
                .font(.caption2)
                .foregroundStyle(.secondary)
            Text("\(value)")
                .font(.subheadline.weight(.semibold))
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(title) \(value)")
    }
}

private struct TopSkillsSection: View {
    let skills: [SkillRecord]

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Top skills")
                .font(.caption)
                .foregroundStyle(.secondary)

            if skills.isEmpty {
                Label("No skills discovered yet", systemImage: "tray")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(skills, id: \.id) { skill in
                    Link(destination: skill.url) {
                        HStack(spacing: 6) {
                            Text(skill.name)
                                .font(.caption)
                                .lineLimit(1)
                            Spacer(minLength: 6)
                            Text(skill.scopeTitle)
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                    }
                    .accessibilityLabel("\(skill.name), \(skill.scopeTitle) skill")
                    .accessibilityHint("Open skill details in the app")
                }
            }
        }
    }
}

private struct WidgetStatusMessage: View {
    let banner: InlineBannerPresentation

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Label(banner.title, systemImage: banner.symbol)
                .font(.caption.weight(.semibold))
                .foregroundStyle(banner.role.tint)
            Text(banner.message)
                .font(.caption2)
                .foregroundStyle(.secondary)
                .lineLimit(2)
        }
        .accessibilityElement(children: .combine)
    }
}

private struct WidgetActionRow: View {
    var body: some View {
        HStack(spacing: 8) {
            Button(intent: SyncNowIntent()) {
                Label("Sync now", systemImage: "arrow.triangle.2.circlepath")
                    .font(.caption)
                    .frame(maxWidth: .infinity, minHeight: 36)
            }
            .buttonStyle(.borderedProminent)
            .accessibilityHint("Queue an immediate sync run")

            Link(destination: URL(string: "skillssync://open")!) {
                Label("Open app", systemImage: "arrow.up.right.square")
                    .font(.caption)
                    .frame(maxWidth: .infinity, minHeight: 36)
            }
            .buttonStyle(.bordered)
            .accessibilityHint("Open Skills Sync app")
        }
    }
}

private extension SkillRecord {
    var url: URL {
        URL(string: "skillssync://skill?id=\(id)")!
    }

    var scopeTitle: String {
        scope.capitalized
    }
}
