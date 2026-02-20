import SwiftUI

struct SkillDeepDivePanel: View {
    let skill: SkillRecord
    let previewData: SkillPreviewData?

    private var contentRelations: [SkillRelation] {
        previewData?.relations.filter { $0.kind == .content } ?? []
    }

    private var symlinkRelations: [SkillRelation] {
        previewData?.relations.filter { $0.kind == .symlink } ?? []
    }

    var body: some View {
        VStack(alignment: .leading, spacing: AppSpacing.md) {
            VStack(alignment: .leading, spacing: AppSpacing.sm) {
                Text("Paths")
                    .font(.app(.secondary).weight(.semibold))
                PathLine(label: "Source path", value: skill.canonicalSourcePath)
                ForEach(Array(skill.targetPaths.enumerated()), id: \.offset) { index, path in
                    PathLine(label: "Target \(index + 1)", value: path)
                }
            }

            if let tree = previewData?.tree {
                VStack(alignment: .leading, spacing: AppSpacing.xs) {
                    Text("Files")
                        .font(.app(.secondary).weight(.semibold))
                    ForEach(tree.children, id: \.id) { node in
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

            if let bodyPreview = previewData?.mainFileBodyPreview {
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
                    if previewData?.isMainFileBodyPreviewTruncated == true {
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
        LabeledContent {
            Text(value)
                .font(.app(.pathMono))
                .lineLimit(nil)
                .multilineTextAlignment(.leading)
                .textSelection(.enabled)
        } label: {
            Text(label)
        }
    }
}
