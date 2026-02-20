import SwiftUI

struct SkillValidationPanel: View {
    let validation: SkillValidationResult
    @Binding var copiedIssueID: String?
    @Binding var isExpanded: Bool
    let onCopyIssue: (SkillValidationIssue) -> Void
    let onFixIssue: (SkillValidationIssue) -> Void

    private var summary: SkillDetailPresentation.ValidationSummary {
        SkillDetailPresentation.validationSummary(issuesCount: validation.issues.count)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: AppSpacing.sm) {
            Label(summary.text, systemImage: summary.symbol)
                .font(.app(.secondary))
                .foregroundStyle(summary.tint)
                .accessibilityLabel("Validation summary: \(summary.text)")

            if validation.hasWarnings {
                DisclosureGroup(isExpanded: $isExpanded) {
                    VStack(alignment: .leading, spacing: AppSpacing.sm) {
                        Text("Select an issue to copy a repair prompt.")
                            .font(.app(.meta))
                            .foregroundStyle(.secondary)

                        ForEach(validation.issues, id: \.id) { issue in
                            VStack(alignment: .leading, spacing: AppSpacing.xs) {
                                Button {
                                    onCopyIssue(issue)
                                } label: {
                                    HStack(alignment: .top, spacing: AppSpacing.sm) {
                                        Image(systemName: copiedIssueID == issue.id ? "checkmark.circle.fill" : "doc.on.doc")
                                            .foregroundStyle(copiedIssueID == issue.id ? .green : .secondary)
                                        VStack(alignment: .leading, spacing: AppSpacing.xs) {
                                            if issue.code.hasPrefix("codex_") || issue.code == "archived_skill_not_visible_in_codex" {
                                                Text("Codex visibility")
                                                    .font(.app(.meta))
                                                    .foregroundStyle(.secondary)
                                            }
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

                                if issue.isAutoFixable {
                                    Button("Fix") {
                                        onFixIssue(issue)
                                    }
                                    .buttonStyle(.borderedProminent)
                                }
                            }
                        }
                    }
                    .padding(.top, AppSpacing.xs)
                } label: {
                    Label("Review validation details", systemImage: "list.bullet.clipboard")
                        .font(.app(.secondary).weight(.semibold))
                }
            }
        }
    }
}
