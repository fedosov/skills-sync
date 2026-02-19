import SwiftUI

@main
struct SkillsSyncApp: App {
    @StateObject private var viewModel = AppViewModel()

    var body: some Scene {
        WindowGroup {
            ContentView(viewModel: viewModel)
                .frame(minWidth: 980, minHeight: 620)
                .onAppear {
                    viewModel.start()
                }
                .onDisappear {
                    viewModel.stop()
                }
                .onOpenURL { url in
                    guard let route = DeepLinkParser.parse(url) else { return }
                    if case let .skill(id: skillID) = route {
                        viewModel.selectedSkillID = skillID
                    }
                }
                .alert("Operation Failed", isPresented: Binding(
                    get: { viewModel.alertMessage != nil },
                    set: { if !$0 { viewModel.alertMessage = nil } }
                )) {
                    Button("OK", role: .cancel) { }
                } message: {
                    Text(viewModel.alertMessage ?? "Unknown error")
                }
        }
    }
}

private struct ContentView: View {
    @ObservedObject var viewModel: AppViewModel

    private var selectedSkill: SkillRecord? {
        viewModel.state.skills.first(where: { $0.id == viewModel.selectedSkillID })
    }

    private var syncErrorBanner: InlineBannerPresentation? {
        let hasError = !(viewModel.state.sync.error?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ?? true)
        guard viewModel.state.sync.status == .failed || hasError else {
            return nil
        }
        return .syncFailure(errorDetails: viewModel.state.sync.error)
    }

    private var commandBanner: InlineBannerPresentation? {
        guard let result = viewModel.state.lastCommandResult else {
            return nil
        }
        return .commandResult(result)
    }

    private var feedbackMessages: [InlineBannerPresentation] {
        [syncErrorBanner, commandBanner, viewModel.localBanner].compactMap { $0 }
    }

    var body: some View {
        NavigationSplitView {
            SidebarView(
                skills: viewModel.filteredSkills,
                searchText: $viewModel.searchText,
                scopeFilter: $viewModel.scopeFilter,
                selectedSkillID: $viewModel.selectedSkillID
            )
            .navigationSplitViewColumnWidth(min: 300, ideal: 340, max: 420)
        } detail: {
            DetailPaneView(
                state: viewModel.state,
                selectedSkill: selectedSkill,
                feedbackMessages: feedbackMessages,
                onSyncNow: viewModel.queueSync,
                onOpen: viewModel.queueOpen,
                onReveal: viewModel.queueReveal,
                onDelete: viewModel.queueDelete
            )
        }
        .toolbar {
            ToolbarItemGroup {
                Button("Refresh") {
                    viewModel.refreshSources()
                }
                Button("Sync Now") {
                    viewModel.queueSync()
                }
                .keyboardShortcut("r", modifiers: [.command, .shift])
            }
        }
    }
}

private struct SidebarView: View {
    let skills: [SkillRecord]
    @Binding var searchText: String
    @Binding var scopeFilter: ScopeFilter
    @Binding var selectedSkillID: String?

    var body: some View {
        VStack(spacing: 8) {
            Picker("Scope", selection: $scopeFilter) {
                ForEach(ScopeFilter.allCases) { scope in
                    Text(scope.title).tag(scope)
                }
            }
            .pickerStyle(.segmented)
            .padding(.horizontal, 8)
            .padding(.top, 8)

            if skills.isEmpty {
                ContentUnavailableView {
                    Label("No Skills Found", systemImage: "tray")
                } description: {
                    Text("Run Sync Now to discover skills, then select one to inspect.")
                }
            } else {
                List(selection: $selectedSkillID) {
                    Section("Source Skills (\(skills.count))") {
                        ForEach(skills, id: \.id) { skill in
                            SkillRowView(skill: skill)
                                .tag(skill.id)
                        }
                    }
                }
                .listStyle(.sidebar)
                .searchable(text: $searchText, prompt: "Search skills and paths")
            }
        }
    }
}

private struct SkillRowView: View {
    let skill: SkillRecord

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(skill.name)
                .font(.headline)
                .lineLimit(1)

            Text(skill.canonicalSourcePath)
                .font(.caption.monospaced())
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .truncationMode(.middle)

            if !skill.exists {
                Label("Missing source", systemImage: "exclamationmark.triangle.fill")
                    .font(.caption)
                    .foregroundStyle(.red)
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(skill.accessibilitySummary)
    }
}

private struct DetailPaneView: View {
    let state: SyncState
    let selectedSkill: SkillRecord?
    let feedbackMessages: [InlineBannerPresentation]
    let onSyncNow: () -> Void
    let onOpen: (SkillRecord) -> Void
    let onReveal: (SkillRecord) -> Void
    let onDelete: (SkillRecord) -> Void

    var body: some View {
        if let selectedSkill {
            SkillDetailView(
                state: state,
                skill: selectedSkill,
                feedbackMessages: feedbackMessages,
                onSyncNow: onSyncNow,
                onOpen: onOpen,
                onReveal: onReveal,
                onDelete: onDelete
            )
        } else {
            VStack(spacing: 0) {
                Form {
                    SyncStatusSection(state: state, feedbackMessages: feedbackMessages, onSyncNow: onSyncNow)
                }
                ContentUnavailableView {
                    Label("Choose a Skill", systemImage: "sidebar.right")
                } description: {
                    Text("Select a skill from the sidebar to inspect details and run actions.")
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
    }
}

private struct SkillDetailView: View {
    let state: SyncState
    let skill: SkillRecord
    let feedbackMessages: [InlineBannerPresentation]
    let onSyncNow: () -> Void
    let onOpen: (SkillRecord) -> Void
    let onReveal: (SkillRecord) -> Void
    let onDelete: (SkillRecord) -> Void
    @State private var showDeleteConfirmation = false

    var body: some View {
        Form {
            SyncStatusSection(state: state, feedbackMessages: feedbackMessages, onSyncNow: onSyncNow)

            Section("Overview") {
                LabeledContent("Name", value: skill.name)
                LabeledContent("Source status", value: skill.exists ? "Available" : "Missing")
                LabeledContent("Package type", value: skill.packageType)
                LabeledContent("Scope", value: skill.scopeTitle)
                if let workspace = skill.workspace {
                    LabeledContent("Workspace", value: workspace)
                }
            }

            Section("Paths") {
                PathLine(label: "Source path", value: skill.canonicalSourcePath)
                ForEach(Array(skill.targetPaths.enumerated()), id: \.offset) { index, path in
                    PathLine(label: "Target \(index + 1)", value: path)
                }
            }

            Section("Integrity") {
                LabeledContent("Source file", value: skill.exists ? "Available" : "Missing from disk")
                LabeledContent("Canonical symlink", value: skill.isSymlinkCanonical ? "Yes" : "No")
            }

            Section("Actions") {
                ControlGroup {
                    Button("Open in Zed") {
                        onOpen(skill)
                    }
                    Button("Reveal in Finder") {
                        onReveal(skill)
                    }
                }
            }

            Section {
                Button("Move Source to Trash", role: .destructive) {
                    showDeleteConfirmation = true
                }
            } header: {
                Text("Danger Zone")
            } footer: {
                Text("This moves the canonical source to Trash. You can restore it or run sync again to recreate it.")
            }
        }
        .confirmationDialog(
            "Move Source to Trash?",
            isPresented: $showDeleteConfirmation,
            titleVisibility: .visible
        ) {
            Button("Move to Trash", role: .destructive) {
                onDelete(skill)
            }
            Button("Cancel", role: .cancel) { }
        } message: {
            Text("The canonical source will be moved to Trash.")
        }
    }
}

private struct SyncStatusSection: View {
    let state: SyncState
    let feedbackMessages: [InlineBannerPresentation]
    let onSyncNow: () -> Void

    private var status: SyncStatusPresentation {
        state.sync.status.presentation
    }

    var body: some View {
        Section("Sync Health") {
            LabeledContent("Status") {
                Label(status.title, systemImage: status.symbol)
                    .foregroundStyle(status.tint)
            }
            LabeledContent("Last update", value: SyncFormatting.updatedLine(state.sync.lastFinishedAt))
            LabeledContent("Global", value: "\(state.summary.globalCount)")
            LabeledContent("Project", value: "\(state.summary.projectCount)")
            LabeledContent("Conflicts", value: "\(state.summary.conflictCount)")

            if !status.subtitle.isEmpty {
                Text(status.subtitle)
                    .foregroundStyle(.secondary)
            }

            ForEach(Array(feedbackMessages.enumerated()), id: \.offset) { _, message in
                VStack(alignment: .leading, spacing: 2) {
                    Label(message.title, systemImage: message.symbol)
                        .foregroundStyle(message.role.tint)
                    Text(message.message)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            if feedbackMessages.contains(where: { $0.recoveryActionTitle != nil }) {
                Button("Sync Now") {
                    onSyncNow()
                }
            }
        }
    }
}

private struct PathLine: View {
    let label: String
    let value: String

    var body: some View {
        LabeledContent(label) {
            Text(value)
                .font(.footnote.monospaced())
                .lineLimit(1)
                .truncationMode(.middle)
                .textSelection(.enabled)
        }
    }
}

private extension SkillRecord {
    var scopeTitle: String {
        scope.capitalized
    }

    var accessibilitySummary: String {
        var summary = "\(name)."
        if !exists {
            summary += " Source missing."
        }
        return summary
    }
}
