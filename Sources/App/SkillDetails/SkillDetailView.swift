import SwiftUI
import AppKit

struct SkillDetailPresentation {
    struct ValidationSummary: Equatable {
        let text: String
        let symbol: String
        let tint: Color
    }

    static let defaultDeepDiveExpanded = false

    static func defaultValidationExpanded(issuesCount: Int) -> Bool {
        issuesCount > 0
    }

    static func validationSummary(issuesCount: Int) -> ValidationSummary {
        if issuesCount > 0 {
            return ValidationSummary(
                text: "\(issuesCount) validation issue(s)",
                symbol: "exclamationmark.triangle.fill",
                tint: .orange
            )
        }
        return ValidationSummary(
            text: "No validation warnings",
            symbol: "checkmark.circle.fill",
            tint: .green
        )
    }
}

struct SkillDetailView: View {
    let skill: SkillRecord
    let onOpen: (SkillRecord) -> Void
    let onReveal: (SkillRecord) -> Void
    let onMoveToTrash: (SkillRecord) -> Void
    let onArchive: (SkillRecord) -> Void
    let onRestoreToGlobal: (SkillRecord) -> Void
    let onMakeGlobal: (SkillRecord) -> Void
    let onRename: (SkillRecord, String) -> Void
    let onApplyValidationFix: (SkillRecord, SkillValidationIssue) -> Void
    let previewProvider: (SkillRecord) async -> SkillPreviewData
    let validationProvider: (SkillRecord) -> SkillValidationResult

    @State private var showDeleteConfirmation = false
    @State private var showArchiveConfirmation = false
    @State private var showRestoreConfirmation = false
    @State private var showMakeGlobalConfirmation = false
    @State private var showMakeGlobalSecondConfirmation = false
    @State private var previewData: SkillPreviewData?
    @State private var copiedIssueID: String?
    @State private var editableTitle: String = ""
    @State private var isTitleDirty = false
    @State private var isValidationExpanded = false
    @State private var isDeepDiveExpanded = SkillDetailPresentation.defaultDeepDiveExpanded

    private var currentDisplayTitle: String {
        previewData?.displayTitle ?? skill.name
    }

    private var canApplyTitle: Bool {
        let trimmed = editableTitle.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return false
        }
        return normalized(trimmed) != normalized(currentDisplayTitle)
    }

    var body: some View {
        let validation = validationProvider(skill)
        Form {
            Section("Overview") {
                SkillOverviewCard(
                    skill: skill,
                    previewData: previewData,
                    validationIssuesCount: validation.issues.count,
                    hasCodexVisibilityIssues: validation.issues.contains(where: { $0.code.hasPrefix("codex_") }),
                    editableTitle: $editableTitle,
                    canApplyTitle: canApplyTitle,
                    onTitleChange: {
                        isTitleDirty = true
                    },
                    onApplyRename: applyRename
                )
            }

            Section("Validation") {
                SkillValidationPanel(
                    validation: validation,
                    copiedIssueID: $copiedIssueID,
                    isExpanded: $isValidationExpanded,
                    onCopyIssue: copyRepairPrompt,
                    onFixIssue: applyValidationFix
                )
            }

            Section {
                Button {
                    withAnimation(.easeInOut(duration: 0.18)) {
                        isDeepDiveExpanded.toggle()
                    }
                } label: {
                    HStack(spacing: AppSpacing.sm) {
                        Image(systemName: isDeepDiveExpanded ? "chevron.down" : "chevron.right")
                            .foregroundStyle(.secondary)
                        Label("Deep Dive", systemImage: "magnifyingglass.circle")
                            .font(.app(.secondary).weight(.semibold))
                        Spacer(minLength: 0)
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Toggle Deep Dive section")
                .accessibilityValue(isDeepDiveExpanded ? "Expanded" : "Collapsed")

                if isDeepDiveExpanded {
                    SkillDeepDivePanel(skill: skill, previewData: previewData)
                        .padding(.top, AppSpacing.sm)
                }
            } footer: {
                Text("Paths, file tree, relations, and raw preview are available on demand.")
                    .font(.app(.secondary))
            }

            Section("Primary Actions") {
                ControlGroup {
                    Button("Open in Zed") {
                        onOpen(skill)
                    }
                    .accessibilityLabel("Open skill source in Zed")

                    Button("Reveal in Finder") {
                        onReveal(skill)
                    }
                    .accessibilityLabel("Reveal skill source in Finder")
                }
            }

            Section("Risk Actions") {
                SkillRiskPanel(
                    skill: skill,
                    showArchiveConfirmation: $showArchiveConfirmation,
                    showDeleteConfirmation: $showDeleteConfirmation,
                    showRestoreConfirmation: $showRestoreConfirmation,
                    showMakeGlobalConfirmation: $showMakeGlobalConfirmation
                )
            }
        }
        .confirmationDialog(
            "Restore archived skill to global?",
            isPresented: $showRestoreConfirmation,
            titleVisibility: .visible
        ) {
            Button("Restore") {
                onRestoreToGlobal(skill)
            }
            Button("Cancel", role: .cancel) { }
        } message: {
            Text("The archived skill source will be moved back into global skills and synced to targets.")
        }
        .confirmationDialog(
            "Archive source?",
            isPresented: $showArchiveConfirmation,
            titleVisibility: .visible
        ) {
            Button("Archive") {
                onArchive(skill)
            }
            Button("Cancel", role: .cancel) { }
        } message: {
            Text("The canonical source and existing symlinks will be moved to app archive storage.")
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
            Text("This moves the canonical source from the project workspace to global skills.")
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
                onMoveToTrash(skill)
            }
            Button("Cancel", role: .cancel) { }
        } message: {
            Text("The canonical source will be moved to system Trash.")
        }
        .formStyle(.grouped)
        .task(id: "\(skill.id)|\(skill.canonicalSourcePath)") {
            let validation = validationProvider(skill)
            isValidationExpanded = SkillDetailPresentation.defaultValidationExpanded(issuesCount: validation.issues.count)
            isDeepDiveExpanded = SkillDetailPresentation.defaultDeepDiveExpanded
            editableTitle = skill.name
            isTitleDirty = false
            previewData = await previewProvider(skill)
            if !isTitleDirty {
                editableTitle = previewData?.displayTitle ?? skill.name
            }
        }
    }

    private func copyRepairPrompt(for issue: SkillValidationIssue) {
        let prompt = SkillRepairPromptBuilder.prompt(for: skill, issue: issue)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(prompt, forType: .string)
        copiedIssueID = issue.id
    }

    private func applyRename() {
        guard canApplyTitle else {
            return
        }
        let title = editableTitle.trimmingCharacters(in: .whitespacesAndNewlines)
        onRename(skill, title)
        isTitleDirty = false
    }

    private func applyValidationFix(_ issue: SkillValidationIssue) {
        guard issue.isAutoFixable else {
            return
        }
        onApplyValidationFix(skill, issue)
    }

    private func normalized(_ value: String) -> String {
        value.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
    }
}

extension SkillRecord {
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
