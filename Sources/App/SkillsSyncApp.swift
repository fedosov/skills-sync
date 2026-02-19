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
                        viewModel.selectedSkillIDs = Set([skillID])
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

    private var syncErrorBanner: InlineBannerPresentation? {
        let hasError = !(viewModel.state.sync.error?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ?? true)
        guard viewModel.state.sync.status == .failed || hasError else {
            return nil
        }
        return .syncFailure(errorDetails: viewModel.state.sync.error)
    }

    private var feedbackMessages: [InlineBannerPresentation] {
        [syncErrorBanner, viewModel.localBanner].compactMap { $0 }
    }

    var body: some View {
        NavigationSplitView {
            SidebarView(
                skills: viewModel.filteredSkills,
                searchText: $viewModel.searchText,
                scopeFilter: $viewModel.scopeFilter,
                selectedSkillIDs: $viewModel.selectedSkillIDs
            )
            .navigationSplitViewColumnWidth(min: 300, ideal: 340, max: 420)
        } detail: {
            DetailPaneView(
                selectedSkills: viewModel.selectedSkills,
                singleSelectedSkill: viewModel.singleSelectedSkill,
                onOpen: viewModel.open,
                onReveal: viewModel.reveal,
                onDelete: viewModel.delete,
                onDeleteSelected: viewModel.deleteSelectedSkills
            )
        }
        .toolbar {
            ToolbarItemGroup {
                SyncHealthToolbarControl(
                    state: viewModel.state,
                    feedbackMessages: feedbackMessages,
                    onSyncNow: viewModel.syncNow
                )

                Button("Refresh") {
                    viewModel.refreshSources()
                }
                Button("Sync Now") {
                    viewModel.syncNow()
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
    @Binding var selectedSkillIDs: Set<String>

    var body: some View {
        VStack(spacing: AppSpacing.sm) {
            Picker("Scope", selection: $scopeFilter) {
                ForEach(ScopeFilter.allCases) { scope in
                    Text(scope.title).tag(scope)
                }
            }
            .pickerStyle(.segmented)
            .padding(.horizontal, AppSpacing.md)
            .padding(.top, AppSpacing.md)

            if skills.isEmpty {
                ContentUnavailableView {
                    Label("No Skills Found", systemImage: "tray")
                } description: {
                    Text("Run Sync Now to discover skills, then select one to inspect.")
                }
            } else {
                List(selection: $selectedSkillIDs) {
                    Section("Source Skills (\(skills.count))") {
                        ForEach(skills, id: \.id) { skill in
                            SkillRowView(skill: skill)
                                .tag(skill.id)
                        }
                    }
                }
                .listStyle(.sidebar)
                .searchable(text: $searchText, placement: .sidebar, prompt: "Search skills and paths")
            }
        }
    }
}

private struct SkillRowView: View {
    let skill: SkillRecord

    var body: some View {
        VStack(alignment: .leading, spacing: AppSpacing.xs) {
            Text(skill.name)
                .font(.app(.body).weight(.semibold))
                .lineLimit(1)

            Text(skill.canonicalSourcePath)
                .font(.app(.pathMono))
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .truncationMode(.middle)

            if !skill.exists {
                Label("Missing source", systemImage: "exclamationmark.triangle.fill")
                    .font(.app(.meta))
                    .foregroundStyle(.red)
            }
        }
        .padding(.vertical, AppSpacing.xs)
        .accessibilityElement(children: .combine)
        .accessibilityLabel(skill.accessibilitySummary)
    }
}

private struct DetailPaneView: View {
    let selectedSkills: [SkillRecord]
    let singleSelectedSkill: SkillRecord?
    let onOpen: (SkillRecord) -> Void
    let onReveal: (SkillRecord) -> Void
    let onDelete: (SkillRecord) -> Void
    let onDeleteSelected: () -> Void

    var body: some View {
        if let singleSelectedSkill {
            SkillDetailView(
                skill: singleSelectedSkill,
                onOpen: onOpen,
                onReveal: onReveal,
                onDelete: onDelete
            )
        } else if selectedSkills.count > 1 {
            MultiSelectionDetailView(
                selectedCount: selectedSkills.count,
                onDeleteSelected: onDeleteSelected
            )
        } else {
            ContentUnavailableView {
                Label("Choose a Skill", systemImage: "sidebar.right")
            } description: {
                Text("Select a skill from the sidebar to inspect details and run actions.")
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }
}

private struct SyncHealthToolbarControl: View {
    let state: SyncState
    let feedbackMessages: [InlineBannerPresentation]
    let onSyncNow: () -> Void
    @State private var showDetails = false

    private var status: SyncStatusPresentation {
        state.sync.status.presentation
    }

    var body: some View {
        Button {
            showDetails = true
        } label: {
            Label(status.title, systemImage: status.symbol)
                .font(.app(.secondary))
                .foregroundStyle(status.tint)
        }
        .buttonStyle(.bordered)
        .help("Show sync health details")
        .popover(isPresented: $showDetails, arrowEdge: .top) {
            SyncHealthPopoverContent(
                state: state,
                feedbackMessages: feedbackMessages,
                onSyncNow: onSyncNow
            )
            .frame(minWidth: 320, idealWidth: 360)
            .padding(AppSpacing.lg)
        }
    }
}

private struct MultiSelectionDetailView: View {
    let selectedCount: Int
    let onDeleteSelected: () -> Void
    @State private var showDeleteConfirmation = false

    var body: some View {
        Form {
            Section("Selection") {
                LabeledContent("Selected", value: "\(selectedCount)")
                Text("Selected: \(selectedCount)")
                    .font(.app(.secondary))
                    .foregroundStyle(.secondary)
            }

            Section {
                Button("Move Selected Sources to Trash", role: .destructive) {
                    showDeleteConfirmation = true
                }
                .accessibilityLabel("Move \(selectedCount) selected sources to Trash")
            } header: {
                Text("Danger Zone")
            } footer: {
                Text("This moves canonical sources to Trash. Some items may fail; successful deletions will still be applied.")
                    .font(.app(.secondary))
            }
        }
        .confirmationDialog(
            "Move \(selectedCount) sources to Trash?",
            isPresented: $showDeleteConfirmation,
            titleVisibility: .visible
        ) {
            Button("Move to Trash", role: .destructive) {
                onDeleteSelected()
            }
            Button("Cancel", role: .cancel) { }
        } message: {
            Text("Selected canonical sources will be moved to Trash in a batch operation.")
        }
        .formStyle(.grouped)
    }
}

private struct SkillDetailView: View {
    let skill: SkillRecord
    let onOpen: (SkillRecord) -> Void
    let onReveal: (SkillRecord) -> Void
    let onDelete: (SkillRecord) -> Void
    @State private var showDeleteConfirmation = false

    var body: some View {
        Form {
            Section("Overview") {
                LabeledContent("Name", value: skill.name)
                LabeledContent("Source status", value: skill.exists ? "Available" : "Missing")
                LabeledContent("Package type", value: skill.packageType)
                LabeledContent("Scope", value: skill.scopeTitle)
                if let workspace = skill.workspace {
                    LabeledContent("Workspace") {
                        Text(workspace)
                            .font(.app(.pathMono))
                            .textSelection(.enabled)
                            .multilineTextAlignment(.leading)
                            .lineLimit(nil)
                    }
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
                    .font(.app(.secondary))
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
        .formStyle(.grouped)
    }
}

private struct SyncHealthPopoverContent: View {
    let state: SyncState
    let feedbackMessages: [InlineBannerPresentation]
    let onSyncNow: () -> Void

    private var status: SyncStatusPresentation {
        state.sync.status.presentation
    }

    var body: some View {
        VStack(alignment: .leading, spacing: AppSpacing.md) {
            Text("Sync Health")
                .font(.app(.sectionHeader))

            LabeledContent("Status") {
                Label(status.title, systemImage: status.symbol)
                    .font(.app(.body))
                    .foregroundStyle(status.tint)
            }
            LabeledContent("Last update", value: SyncFormatting.updatedLine(state.sync.lastFinishedAt))
            LabeledContent("Global", value: "\(state.summary.globalCount)")
            LabeledContent("Project", value: "\(state.summary.projectCount)")
            LabeledContent("Conflicts", value: "\(state.summary.conflictCount)")

            if !status.subtitle.isEmpty {
                Text(status.subtitle)
                    .font(.app(.secondary))
                    .foregroundStyle(.secondary)
            }

            ForEach(Array(feedbackMessages.enumerated()), id: \.offset) { _, message in
                VStack(alignment: .leading, spacing: AppSpacing.xs) {
                    Label(message.title, systemImage: message.symbol)
                        .font(.app(.secondary))
                        .foregroundStyle(message.role.tint)
                    Text(message.message)
                        .font(.app(.meta))
                        .foregroundStyle(.secondary)
                }
            }

            Divider()

            Button("Sync Now") {
                onSyncNow()
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
                .font(.app(.pathMono))
                .lineLimit(nil)
                .multilineTextAlignment(.leading)
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
