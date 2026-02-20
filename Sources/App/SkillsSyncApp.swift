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
                onDelete: viewModel.delete,
                onMakeGlobal: viewModel.makeGlobal,
                onDeleteSelected: viewModel.deleteSelectedSkills,
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
                    onRemoveWorkspaceRoot: viewModel.removeWorkspaceDiscoveryRoot,
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
                    Text("Run Sync Now to discover skills, then select one to inspect.")
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
        }
        .padding(.vertical, AppSpacing.xs)
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
    let onDelete: (SkillRecord) -> Void
    let onMakeGlobal: (SkillRecord) -> Void
    let onDeleteSelected: () -> Void
    let previewProvider: (SkillRecord) async -> SkillPreviewData
    let validationProvider: (SkillRecord) -> SkillValidationResult

    var body: some View {
        if let singleSelectedSkill {
            SkillDetailView(
                skill: singleSelectedSkill,
                onOpen: onOpen,
                onReveal: onReveal,
                onDelete: onDelete,
                onMakeGlobal: onMakeGlobal,
                previewProvider: previewProvider,
                validationProvider: validationProvider
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
    @Binding var autoMigrateToCanonicalSource: Bool
    @Binding var workspaceDiscoveryRoots: [String]
    let onAddWorkspaceRoot: (String) -> Void
    let onRemoveWorkspaceRoot: (String) -> Void
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
                autoMigrateToCanonicalSource: $autoMigrateToCanonicalSource,
                workspaceDiscoveryRoots: $workspaceDiscoveryRoots,
                onAddWorkspaceRoot: onAddWorkspaceRoot,
                onRemoveWorkspaceRoot: onRemoveWorkspaceRoot,
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
    let onMakeGlobal: (SkillRecord) -> Void
    let previewProvider: (SkillRecord) async -> SkillPreviewData
    let validationProvider: (SkillRecord) -> SkillValidationResult
    @State private var showDeleteConfirmation = false
    @State private var showMakeGlobalConfirmation = false
    @State private var showMakeGlobalSecondConfirmation = false
    @State private var previewData: SkillPreviewData?
    @State private var copiedIssueID: String?

    var body: some View {
        Form {
            Section("Overview & Preview") {
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
                Divider()
                Text("Skill Preview")
                    .font(.app(.sectionHeader))
                if let previewData {
                    SkillPreviewSection(previewData: previewData)
                } else {
                    Text("Loading preview...")
                        .font(.app(.secondary))
                        .foregroundStyle(.secondary)
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

            Section("Validation") {
                let validation = validationProvider(skill)
                if validation.hasWarnings {
                    Label(validation.summaryText, systemImage: "exclamationmark.triangle.fill")
                        .font(.app(.secondary))
                        .foregroundStyle(.orange)
                    Text("You can click issues and the repair prompt will be copied.")
                        .font(.app(.meta))
                        .foregroundStyle(.secondary)
                    ForEach(validation.issues, id: \.id) { issue in
                        Button {
                            copyRepairPrompt(for: issue)
                        } label: {
                            HStack(alignment: .top, spacing: AppSpacing.sm) {
                                Image(systemName: copiedIssueID == issue.id ? "checkmark.circle.fill" : "doc.on.doc")
                                    .foregroundStyle(copiedIssueID == issue.id ? .green : .secondary)
                                VStack(alignment: .leading, spacing: AppSpacing.xs) {
                                    Text(issue.message)
                                        .font(.app(.secondary))
                                        .foregroundStyle(.secondary)
                                        .multilineTextAlignment(.leading)
                                    if let source = issue.sourceLocationText {
                                        Text("Source: \(source)")
                                            .font(.app(.meta))
                                            .foregroundStyle(.secondary)
                                            .multilineTextAlignment(.leading)
                                            .textSelection(.enabled)
                                    }
                                    if !issue.details.isEmpty {
                                        Text("Details: \(issue.details)")
                                            .font(.app(.meta))
                                            .foregroundStyle(.secondary)
                                            .multilineTextAlignment(.leading)
                                    }
                                }
                                Spacer(minLength: 0)
                            }
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .accessibilityLabel("Copy repair prompt for issue: \(issue.message)")
                    }
                } else {
                    Text("No validation warnings")
                        .font(.app(.secondary))
                        .foregroundStyle(.secondary)
                }
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
                if skill.scope == "project" {
                    Button("Make Global", role: .destructive) {
                        showMakeGlobalConfirmation = true
                    }
                }

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
            "Make skill global?",
            isPresented: $showMakeGlobalConfirmation,
            titleVisibility: .visible
        ) {
            Button("Continue", role: .destructive) {
                showMakeGlobalSecondConfirmation = true
            }
            Button("Cancel", role: .cancel) { }
        } message: {
            Text("This will move the canonical source from the project workspace to global skills.")
        }
        .confirmationDialog(
            "Are you sure?",
            isPresented: $showMakeGlobalSecondConfirmation,
            titleVisibility: .visible
        ) {
            Button("Make Global", role: .destructive) {
                onMakeGlobal(skill)
            }
            Button("Cancel", role: .cancel) { }
        } message: {
            Text("This action changes scope and file location. Continue?")
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
        .task(id: "\(skill.id)|\(skill.canonicalSourcePath)") {
            previewData = await previewProvider(skill)
        }
    }

    private func copyRepairPrompt(for issue: SkillValidationIssue) {
        let prompt = SkillRepairPromptBuilder.prompt(for: skill, issue: issue)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(prompt, forType: .string)
        copiedIssueID = issue.id
    }
}

private struct SyncHealthPopoverContent: View {
    let state: SyncState
    let feedbackMessages: [InlineBannerPresentation]
    @Binding var autoMigrateToCanonicalSource: Bool
    @Binding var workspaceDiscoveryRoots: [String]
    let onAddWorkspaceRoot: (String) -> Void
    let onRemoveWorkspaceRoot: (String) -> Void
    let onSyncNow: () -> Void
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
                            .buttonStyle(.borderless)
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
                Spacer()
                Button("Sync Now") {
                    onSyncNow()
                }
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

private struct SkillPreviewSection: View {
    let previewData: SkillPreviewData

    private var contentRelations: [SkillRelation] {
        previewData.relations.filter { $0.kind == .content }
    }

    private var symlinkRelations: [SkillRelation] {
        previewData.relations.filter { $0.kind == .symlink }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: AppSpacing.md) {
            if let header = previewData.header {
                VStack(alignment: .leading, spacing: AppSpacing.xs) {
                    Text(header.title)
                        .font(.app(.sectionHeader))
                    if let description = header.description {
                        Text(description)
                            .font(.app(.secondary))
                            .foregroundStyle(.secondary)
                    }
                    if !header.metadata.isEmpty {
                        ScrollView(.horizontal) {
                            HStack(spacing: AppSpacing.sm) {
                                ForEach(header.metadata, id: \.self) { item in
                                    Text("\(item.key): \(item.value)")
                                        .font(.app(.meta))
                                        .padding(.horizontal, AppSpacing.sm)
                                        .padding(.vertical, 2)
                                        .background(.quaternary.opacity(0.5))
                                        .clipShape(Capsule())
                                }
                            }
                        }
                        .scrollIndicators(.never)
                    }
                    if let intro = header.intro {
                        Text(intro)
                            .font(.app(.secondary))
                    }
                }
            } else if let reason = previewData.previewUnavailableReason {
                Text(reason)
                    .font(.app(.secondary))
                    .foregroundStyle(.secondary)
            }

            if let root = previewData.tree {
                VStack(alignment: .leading, spacing: AppSpacing.xs) {
                    Text("Files")
                        .font(.app(.secondary).weight(.semibold))
                    ForEach(root.children, id: \.id) { node in
                        SkillTreeNodeView(node: node, depth: 0)
                    }
                }
            }

            if !contentRelations.isEmpty || !symlinkRelations.isEmpty {
                VStack(alignment: .leading, spacing: AppSpacing.xs) {
                    Text("Relations")
                        .font(.app(.secondary).weight(.semibold))
                    ForEach(contentRelations, id: \.id) { relation in
                        RelationRow(relation: relation)
                    }
                    ForEach(symlinkRelations, id: \.id) { relation in
                        RelationRow(relation: relation)
                    }
                }
            }

            if let bodyPreview = previewData.mainFileBodyPreview {
                VStack(alignment: .leading, spacing: AppSpacing.xs) {
                    Text("SKILL.md Preview")
                        .font(.app(.secondary).weight(.semibold))
                    Text(bodyPreview)
                        .font(.app(.pathMono))
                        .textSelection(.enabled)
                        .lineLimit(nil)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(AppSpacing.sm)
                        .background(.quaternary.opacity(0.35))
                        .clipShape(RoundedRectangle(cornerRadius: 8))
                    if previewData.isMainFileBodyPreviewTruncated {
                        Text("Preview truncated")
                            .font(.app(.meta))
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
    }
}

private struct SkillTreeNodeView: View {
    let node: SkillTreeNode
    let depth: Int

    var body: some View {
        VStack(alignment: .leading, spacing: AppSpacing.xs) {
            HStack(spacing: AppSpacing.sm) {
                Image(systemName: node.isDirectory ? "folder" : "doc.text")
                    .foregroundStyle(.secondary)
                Text(node.name)
                    .font(node.isDirectory ? .app(.secondary) : .app(.pathMono))
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            .padding(.leading, CGFloat(depth) * 14)

            ForEach(node.children, id: \.id) { child in
                SkillTreeNodeView(node: child, depth: depth + 1)
            }
        }
    }
}

private struct RelationRow: View {
    let relation: SkillRelation

    var body: some View {
        HStack(alignment: .top, spacing: AppSpacing.sm) {
            Image(systemName: relation.kind == .content ? "doc.text" : "link")
                .foregroundStyle(.secondary)
            VStack(alignment: .leading, spacing: 2) {
                Text(relation.from)
                    .font(.app(.meta))
                    .foregroundStyle(.secondary)
                Text("-> \(relation.to)")
                    .font(.app(.pathMono))
                    .lineLimit(1)
                    .truncationMode(.middle)
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
