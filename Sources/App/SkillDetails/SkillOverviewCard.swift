import SwiftUI

struct SkillOverviewCard: View {
    private struct StatusChip {
        let label: String
        let symbol: String
        let tint: Color
        let helpText: String?
    }

    let skill: SkillRecord
    let previewData: SkillPreviewData?
    let validationIssuesCount: Int
    let hasCodexVisibilityIssues: Bool
    @Binding var editableTitle: String
    let canApplyTitle: Bool
    let onTitleChange: () -> Void
    let onApplyRename: () -> Void

    private var statusItems: [StatusChip] {
        var items: [StatusChip] = [
            StatusChip(
                label: skill.exists ? "Source available" : "Source missing",
                symbol: skill.exists ? "checkmark.circle.fill" : "xmark.octagon.fill",
                tint: skill.exists ? .green : .red,
                helpText: nil
            )
        ]
        if skill.status == .archived {
            items.append(
                StatusChip(
                    label: "Archived",
                    symbol: "archivebox.fill",
                    tint: .secondary,
                    helpText: "Gray means informational status. Archived is not an error."
                )
            )
        }
        if validationIssuesCount > 0 {
            items.append(
                StatusChip(
                    label: "\(validationIssuesCount) validation issue(s)",
                    symbol: "exclamationmark.triangle.fill",
                    tint: .orange,
                    helpText: nil
                )
            )
        }
        items.append(
            StatusChip(
                label: hasCodexVisibilityIssues ? "Codex hidden" : "Codex visible",
                symbol: hasCodexVisibilityIssues ? "eye.slash.fill" : "eye.fill",
                tint: hasCodexVisibilityIssues ? .orange : .green,
                helpText: hasCodexVisibilityIssues
                    ? "Codex visibility checks reported warnings."
                    : "Codex visibility checks passed."
            )
        )
        return items
    }

    var body: some View {
        VStack(alignment: .leading, spacing: AppSpacing.md) {
            HStack(alignment: .firstTextBaseline, spacing: AppSpacing.md) {
                Text("Title")
                    .font(.app(.sectionHeader))
                    .frame(width: 80, alignment: .leading)

                TextField("", text: $editableTitle, prompt: Text("Title"))
                    .textFieldStyle(.roundedBorder)
                    .onChange(of: editableTitle) { _, _ in
                        onTitleChange()
                    }
                    .onSubmit {
                        onApplyRename()
                    }

                Button("Apply") {
                    onApplyRename()
                }
                .disabled(!canApplyTitle)
                .accessibilityLabel("Apply skill title rename")
            }

            HStack(spacing: AppSpacing.sm) {
                ForEach(statusItems, id: \.label) { item in
                    Label(item.label, systemImage: item.symbol)
                        .font(.app(.secondary))
                        .foregroundStyle(item.tint)
                        .padding(.horizontal, AppSpacing.sm)
                        .padding(.vertical, 2)
                        .background(.quaternary.opacity(0.4))
                        .clipShape(Capsule())
                        .help(item.helpText ?? "")
                }
            }

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

            if skill.status == .archived {
                if let scope = skill.archivedOriginalScope {
                    LabeledContent("Archived from scope", value: scope)
                }
                if let workspace = skill.archivedOriginalWorkspace {
                    LabeledContent("Archived from workspace", value: workspace)
                }
                if let archivedAt = skill.archivedAt {
                    LabeledContent("Archived at", value: archivedAt)
                }
            }

            Divider()

            if let previewData {
                VStack(alignment: .leading, spacing: AppSpacing.xs) {
                    Text("Preview")
                        .font(.app(.title))
                    Text(previewData.displayTitle)
                        .font(.app(.title).weight(.semibold))
                    if let description = previewData.header?.description {
                        Text(description)
                            .font(.app(.body))
                            .foregroundStyle(.secondary)
                    }
                    if let reason = previewData.previewUnavailableReason {
                        Text(reason)
                            .font(.app(.body))
                            .foregroundStyle(.secondary)
                    }
                }
            } else {
                Text("Loading preview...")
                    .font(.app(.body))
                    .foregroundStyle(.secondary)
            }
        }
    }
}
