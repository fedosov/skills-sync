import SwiftUI

struct SkillRiskPanel: View {
    let skill: SkillRecord
    @Binding var showArchiveConfirmation: Bool
    @Binding var showDeleteConfirmation: Bool
    @Binding var showRestoreConfirmation: Bool
    @Binding var showMakeGlobalConfirmation: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: AppSpacing.sm) {
            Label("What happens next", systemImage: "exclamationmark.shield")
                .font(.app(.secondary).weight(.semibold))

            if skill.status == .archived {
                Text("Restore returns this archived skill to global scope and re-syncs linked targets.")
                    .font(.app(.secondary))
                    .foregroundStyle(.secondary)

                Button("Restore to Global") {
                    showRestoreConfirmation = true
                }
                .accessibilityLabel("Restore archived skill to global scope")

                Button("Move Source to Trash", role: .destructive) {
                    showDeleteConfirmation = true
                }
                .accessibilityLabel("Permanently delete archived skill source to system Trash")
            } else {
                Text("Archive keeps a recoverable copy in app storage. Trash moves source files to system Trash.")
                    .font(.app(.secondary))
                    .foregroundStyle(.secondary)

                if skill.scope == "project" {
                    Button("Make Global", role: .destructive) {
                        showMakeGlobalConfirmation = true
                    }
                    .accessibilityLabel("Move project skill to global scope")
                }

                Button("Archive Source") {
                    showArchiveConfirmation = true
                }
                .accessibilityLabel("Archive skill source")

                Button("Move Source to Trash", role: .destructive) {
                    showDeleteConfirmation = true
                }
                .accessibilityLabel("Move skill source to system Trash")
            }
        }
        .padding(.vertical, AppSpacing.xs)
    }
}
