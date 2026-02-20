import SwiftUI
import AppKit

@main
struct SkillsSyncApp: App {
    @StateObject private var viewModel = AppViewModel()

    var body: some Scene {
        Window("Skills Sync", id: "main") {
            ContentView(viewModel: viewModel)
                .frame(minWidth: 980, minHeight: 620)
                .background {
                    WindowStateCoordinator(viewModel: viewModel)
                        .frame(width: 0, height: 0)
                }
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
                selectedSkillIDs: $viewModel.selectedSkillIDs,
                displayTitle: viewModel.displayTitle,
                warmupTitles: viewModel.warmupTitles,
                validationProvider: viewModel.validation,
                warmupValidation: viewModel.warmupValidation
            )
            .navigationSplitViewColumnWidth(min: 300, ideal: 340, max: 420)
        } detail: {
            DetailPaneView(
                selectedSkills: viewModel.selectedSkills,
                singleSelectedSkill: viewModel.singleSelectedSkill,
                onOpen: viewModel.open,
                onReveal: viewModel.reveal,
                onMoveToTrash: viewModel.moveToTrash,
                onArchive: viewModel.archive,
                onRestoreToGlobal: viewModel.restoreToGlobal,
                onMakeGlobal: viewModel.makeGlobal,
                onRename: viewModel.rename,
                onRepairCodexFrontmatter: viewModel.repairCodexFrontmatter,
                onArchiveSelected: viewModel.archiveSelectedSkills,
                onTrashSelected: viewModel.deleteSelectedSkills,
                previewProvider: viewModel.preview,
                validationProvider: viewModel.validation
            )
        }
        .toolbar {
            ToolbarItemGroup {
                SyncHealthToolbarControl(
                    state: viewModel.state,
                    feedbackMessages: feedbackMessages,
                    autoMigrateToCanonicalSource: $viewModel.autoMigrateToCanonicalSource,
                    workspaceDiscoveryRoots: $viewModel.workspaceDiscoveryRoots,
                    onAddWorkspaceRoot: viewModel.addWorkspaceDiscoveryRoot,
                    onRemoveWorkspaceRoot: viewModel.removeWorkspaceDiscoveryRoot
                )
            }
        }
    }
}

private struct SidebarView: View {
    let skills: [SkillRecord]
    @Binding var searchText: String
    @Binding var scopeFilter: ScopeFilter
    @Binding var selectedSkillIDs: Set<String>
    let displayTitle: (SkillRecord) -> String
    let warmupTitles: ([SkillRecord]) async -> Void
    let validationProvider: (SkillRecord) -> SkillValidationResult
    let warmupValidation: ([SkillRecord]) async -> Void

    private var groups: [SidebarSkillGroup] {
        AppViewModel.sidebarGroups(from: skills)
    }

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
                    Text("Skills are discovered automatically. Select one to inspect once available.")
                }
            } else {
                List(selection: $selectedSkillIDs) {
                    ForEach(groups, id: \.id) { group in
                        Section(group.title) {
                            ForEach(group.skills, id: \.id) { skill in
                                SkillRowView(
                                    skill: skill,
                                    title: displayTitle(skill),
                                    validation: validationProvider(skill)
                                )
                                    .tag(skill.id)
                            }
                        }
                    }
                }
                .listStyle(.sidebar)
                .searchable(text: $searchText, placement: .sidebar, prompt: "Search skills and paths")
                .task(id: skills.map(\.id).joined(separator: "|")) {
                    await warmupTitles(skills)
                    await warmupValidation(skills)
                }
            }
        }
    }
}

private struct SkillRowView: View {
    let skill: SkillRecord
    let title: String
    let validation: SkillValidationResult

    private var shouldShowSkillName: Bool {
        normalized(title) != normalized(skill.name)
    }

    private var archivedStatusLine: String? {
        guard skill.status == .archived else {
            return nil
        }
        var problems: [String] = []
        if !skill.exists {
            problems.append("missing source")
        }
        if validation.hasWarnings {
            problems.append("\(validation.issues.count) issue(s)")
        }
        if problems.isEmpty {
            return "Archived"
        }
        return "Archived • \(problems.joined(separator: ", "))"
    }

    var body: some View {
        VStack(alignment: .leading, spacing: AppSpacing.xs) {
            Text(title)
                .font(.app(.body).weight(.semibold))
                .lineLimit(1)

            if shouldShowSkillName {
                Text(skill.name)
                    .font(.app(.meta))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }

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

            if validation.hasWarnings {
                Label("\(validation.issues.count) validation issue(s)", systemImage: "exclamationmark.triangle.fill")
                    .font(.app(.meta))
                    .foregroundStyle(.orange)
            }

            if let archivedStatusLine {
                Text(archivedStatusLine)
                    .font(.app(.meta))
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.vertical, AppSpacing.xs)
        .opacity(skill.status == .archived ? 0.72 : 1.0)
        .accessibilityElement(children: .combine)
        .accessibilityLabel(skill.accessibilitySummary)
    }

    private func normalized(_ value: String) -> String {
        value.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
    }
}

private struct DetailPaneView: View {
    let selectedSkills: [SkillRecord]
    let singleSelectedSkill: SkillRecord?
    let onOpen: (SkillRecord) -> Void
    let onReveal: (SkillRecord) -> Void
    let onMoveToTrash: (SkillRecord) -> Void
    let onArchive: (SkillRecord) -> Void
    let onRestoreToGlobal: (SkillRecord) -> Void
    let onMakeGlobal: (SkillRecord) -> Void
    let onRename: (SkillRecord, String) -> Void
    let onRepairCodexFrontmatter: (SkillRecord) -> Void
    let onArchiveSelected: () -> Void
    let onTrashSelected: () -> Void
    let previewProvider: (SkillRecord) async -> SkillPreviewData
    let validationProvider: (SkillRecord) -> SkillValidationResult

    var body: some View {
        if let singleSelectedSkill {
            SkillDetailView(
                skill: singleSelectedSkill,
                onOpen: onOpen,
                onReveal: onReveal,
                onMoveToTrash: onMoveToTrash,
                onArchive: onArchive,
                onRestoreToGlobal: onRestoreToGlobal,
                onMakeGlobal: onMakeGlobal,
                onRename: onRename,
                onRepairCodexFrontmatter: onRepairCodexFrontmatter,
                previewProvider: previewProvider,
                validationProvider: validationProvider
            )
        } else if selectedSkills.count > 1 {
            MultiSelectionDetailView(
                selectedCount: selectedSkills.count,
                onArchiveSelected: onArchiveSelected,
                onTrashSelected: onTrashSelected
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
    @Binding var autoMigrateToCanonicalSource: Bool
    @Binding var workspaceDiscoveryRoots: [String]
    let onAddWorkspaceRoot: (String) -> Void
    let onRemoveWorkspaceRoot: (String) -> Void
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
                autoMigrateToCanonicalSource: $autoMigrateToCanonicalSource,
                workspaceDiscoveryRoots: $workspaceDiscoveryRoots,
                onAddWorkspaceRoot: onAddWorkspaceRoot,
                onRemoveWorkspaceRoot: onRemoveWorkspaceRoot
            )
            .frame(minWidth: 320, idealWidth: 360)
            .padding(AppSpacing.lg)
        }
    }
}

private struct MultiSelectionDetailView: View {
    let selectedCount: Int
    let onArchiveSelected: () -> Void
    let onTrashSelected: () -> Void
    @State private var showArchiveConfirmation = false
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
                Button("Archive Selected") {
                    showArchiveConfirmation = true
                }
                .accessibilityLabel("Archive \(selectedCount) selected skills")

                Button("Move Selected Sources to Trash", role: .destructive) {
                    showDeleteConfirmation = true
                }
                .accessibilityLabel("Move \(selectedCount) selected sources to Trash")
            } header: {
                Text("Danger Zone")
            } footer: {
                Text("You can archive selected active skills or move them to Trash. Archived items are skipped for batch archive.")
                    .font(.app(.secondary))
            }
        }
        .confirmationDialog(
            "Archive \(selectedCount) selected skills?",
            isPresented: $showArchiveConfirmation,
            titleVisibility: .visible
        ) {
            Button("Archive") {
                onArchiveSelected()
            }
            Button("Cancel", role: .cancel) { }
        } message: {
            Text("Selected active skills will be moved into the app archive.")
        }
        .confirmationDialog(
            "Move \(selectedCount) sources to Trash?",
            isPresented: $showDeleteConfirmation,
            titleVisibility: .visible
        ) {
            Button("Move to Trash", role: .destructive) {
                onTrashSelected()
            }
            Button("Cancel", role: .cancel) { }
        } message: {
            Text("Selected canonical sources will be moved to Trash in a batch operation.")
        }
        .formStyle(.grouped)
    }
}

private struct SyncHealthPopoverContent: View {
    let state: SyncState
    let feedbackMessages: [InlineBannerPresentation]
    @Binding var autoMigrateToCanonicalSource: Bool
    @Binding var workspaceDiscoveryRoots: [String]
    let onAddWorkspaceRoot: (String) -> Void
    let onRemoveWorkspaceRoot: (String) -> Void
    @State private var newWorkspaceRoot: String = ""
    @State private var workspaceRootValidationMessage: String?

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

            VStack(alignment: .leading, spacing: AppSpacing.sm) {
                Text("Workspace search roots")
                    .font(.app(.secondary).weight(.semibold))

                if workspaceDiscoveryRoots.isEmpty {
                    Text("No custom roots configured.")
                        .font(.app(.meta))
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(workspaceDiscoveryRoots, id: \.self) { root in
                        HStack(spacing: AppSpacing.sm) {
                            Text(root)
                                .font(.app(.pathMono))
                                .lineLimit(1)
                                .truncationMode(.middle)
                            Spacer()
                            Button("Remove Root") {
                                onRemoveWorkspaceRoot(root)
                            }
                        }
                    }
                }

                HStack(spacing: AppSpacing.sm) {
                    TextField("/absolute/path/to/workspaces", text: $newWorkspaceRoot)
                        .textFieldStyle(.roundedBorder)
                    Button("Add Root") {
                        guard let valid = validatedWorkspaceRoot(newWorkspaceRoot) else { return }
                        onAddWorkspaceRoot(valid)
                        newWorkspaceRoot = ""
                        workspaceRootValidationMessage = nil
                    }
                }

                if let workspaceRootValidationMessage {
                    Text(workspaceRootValidationMessage)
                        .font(.app(.meta))
                        .foregroundStyle(.orange)
                }
            }

            Divider()

            HStack(spacing: AppSpacing.md) {
                Toggle("Auto-migrate source", isOn: $autoMigrateToCanonicalSource)
                    .toggleStyle(.checkbox)
                    .font(.app(.secondary))
            }
        }
    }

    private func validatedWorkspaceRoot(_ candidate: String) -> String? {
        let trimmed = candidate.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            workspaceRootValidationMessage = "Path cannot be empty."
            return nil
        }
        guard trimmed.hasPrefix("/") else {
            workspaceRootValidationMessage = "Path must be absolute."
            return nil
        }
        let normalized = URL(fileURLWithPath: trimmed, isDirectory: true).standardizedFileURL.path
        guard !workspaceDiscoveryRoots.contains(normalized) else {
            workspaceRootValidationMessage = "Path is already in the list."
            return nil
        }
        return normalized
    }
}
