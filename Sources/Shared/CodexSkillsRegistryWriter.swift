import Foundation

struct CodexSkillsRegistryWriter {
    struct RegistryEntry: Hashable {
        let path: String
        let enabled: Bool
    }

    enum WriterError: LocalizedError {
        case invalidHomeDirectory(String)
        case writeFailed(String)

        var errorDescription: String? {
            switch self {
            case let .invalidHomeDirectory(path):
                return "Invalid home directory for Codex config: \(path)"
            case let .writeFailed(reason):
                return "Failed to write Codex registry: \(reason)"
            }
        }
    }

    private let fileManager: FileManager
    private let homeDirectory: URL
    private let beginMarker = "# skills-sync:begin"
    private let endMarker = "# skills-sync:end"

    init(homeDirectory: URL, fileManager: FileManager = .default) {
        self.homeDirectory = homeDirectory
        self.fileManager = fileManager
    }

    func writeManagedRegistry(skills: [SkillRecord]) throws {
        guard homeDirectory.path.hasPrefix("/") else {
            throw WriterError.invalidHomeDirectory(homeDirectory.path)
        }

        let entries = buildEntries(from: skills)
        let configURL = homeDirectory
            .appendingPathComponent(".codex", isDirectory: true)
            .appendingPathComponent("config.toml")

        do {
            try fileManager.createDirectory(at: configURL.deletingLastPathComponent(), withIntermediateDirectories: true)
            let existing = (try? String(contentsOf: configURL, encoding: .utf8)) ?? ""
            let updated = upsertManagedBlock(in: existing, entries: entries)
            try updated.write(to: configURL, atomically: true, encoding: .utf8)
        } catch {
            throw WriterError.writeFailed(error.localizedDescription)
        }
    }

    private func buildEntries(from skills: [SkillRecord]) -> [RegistryEntry] {
        var unique: Set<String> = []
        var ordered: [RegistryEntry] = []

        for skill in skills {
            guard skill.status == .active else {
                continue
            }
            if let agentsTarget = preferredAgentsTarget(for: skill) {
                let standardized = URL(fileURLWithPath: agentsTarget).standardizedFileURL.path
                if unique.insert(standardized).inserted {
                    ordered.append(RegistryEntry(path: standardized, enabled: true))
                }
                continue
            }

            let canonical = URL(fileURLWithPath: skill.canonicalSourcePath).standardizedFileURL.path
            if unique.insert(canonical).inserted {
                ordered.append(RegistryEntry(path: canonical, enabled: true))
            }
        }

        return ordered.sorted { $0.path < $1.path }
    }

    private func preferredAgentsTarget(for skill: SkillRecord) -> String? {
        let globalNeedle = "/.agents/skills/\(skill.skillKey)"
        let projectNeedle = "/.agents/skills/\(skill.skillKey)"
        for targetPath in skill.targetPaths {
            let standardized = URL(fileURLWithPath: targetPath).standardizedFileURL.path
            if standardized.hasSuffix(globalNeedle) || standardized.hasSuffix(projectNeedle) {
                return standardized
            }
        }
        return nil
    }

    private func upsertManagedBlock(in current: String, entries: [RegistryEntry]) -> String {
        let block = managedBlock(entries: entries)
        if current.isEmpty {
            return block + "\n"
        }

        let normalized = current.replacingOccurrences(of: "\r\n", with: "\n")
        if let beginRange = normalized.range(of: beginMarker),
           let endRange = normalized.range(of: endMarker, range: beginRange.upperBound..<normalized.endIndex) {
            let prefix = String(normalized[..<beginRange.lowerBound]).trimmingCharacters(in: .newlines)
            let suffix = String(normalized[endRange.upperBound...]).trimmingCharacters(in: .newlines)

            if prefix.isEmpty && suffix.isEmpty {
                return block + "\n"
            }
            if prefix.isEmpty {
                return block + "\n\n" + suffix + "\n"
            }
            if suffix.isEmpty {
                return prefix + "\n\n" + block + "\n"
            }
            return prefix + "\n\n" + block + "\n\n" + suffix + "\n"
        }

        let trimmed = normalized.trimmingCharacters(in: .newlines)
        if trimmed.isEmpty {
            return block + "\n"
        }
        return trimmed + "\n\n" + block + "\n"
    }

    private func managedBlock(entries: [RegistryEntry]) -> String {
        var lines: [String] = [beginMarker]
        if entries.isEmpty {
            lines.append("# No managed skill entries")
        } else {
            for entry in entries {
                lines.append("[[skills.config]]")
                lines.append("path = \"\(tomlEscape(entry.path))\"")
                lines.append("enabled = \(entry.enabled ? "true" : "false")")
                lines.append("")
            }
            if lines.last?.isEmpty == true {
                lines.removeLast()
            }
        }
        lines.append(endMarker)
        return lines.joined(separator: "\n")
    }

    private func tomlEscape(_ value: String) -> String {
        value
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
    }
}
