import Foundation

struct SkillValidationIssue: Identifiable, Hashable {
    let code: String
    let message: String
    let source: String?
    let line: Int?
    let details: String

    init(
        code: String,
        message: String,
        source: String? = nil,
        line: Int? = nil,
        details: String = ""
    ) {
        self.code = code
        self.message = message
        self.source = source
        self.line = line
        self.details = details
    }

    var id: String {
        "\(code)|\(message)|\(source ?? "-")|\(line ?? -1)|\(details)"
    }

    var sourceLocationText: String? {
        guard let source else { return nil }
        if let line {
            return "\(source):\(line)"
        }
        return source
    }

    var isAutoFixable: Bool {
        switch code {
        case "codex_frontmatter_invalid_yaml",
            "missing_frontmatter_name",
            "missing_frontmatter_description",
            "frontmatter_name_mismatch_skill_key":
            return true
        default:
            return false
        }
    }
}

struct SkillValidationResult: Hashable {
    let issues: [SkillValidationIssue]

    var hasWarnings: Bool {
        !issues.isEmpty
    }

    var summaryText: String {
        let count = issues.count
        let noun = count == 1 ? "issue" : "issues"
        return "\(count) \(noun) found"
    }
}

struct SkillValidator {
    private let fileManager: FileManager
    private struct BrokenReferenceHit: Hashable {
        let path: String
        let line: Int
    }
    private struct CandidateHit: Hashable {
        let candidate: String
        let line: Int
    }
    private struct CodeContext: Hashable {
        let text: String
        let line: Int
    }
    private struct FrontmatterBlock: Hashable {
        let raw: String
        let startLine: Int
    }

    init(fileManager: FileManager = .default) {
        self.fileManager = fileManager
    }

    func validate(skill: SkillRecord) -> SkillValidationResult {
        let mainFile = resolveMainSkillFile(skill: skill)
        let root = skillRootURL(skill: skill, mainSkillFile: mainFile)
        var issues: [SkillValidationIssue] = []

        if isSymbolicLink(mainFile) {
            if let target = symlinkDestination(of: mainFile) {
                if fileManager.fileExists(atPath: target.path) {
                    issues.append(
                        SkillValidationIssue(
                            code: "skill_md_is_symlink",
                            message: "SKILL.md is a symlink",
                            source: mainFile.path,
                            line: 1,
                            details: "This skill uses a symlinked SKILL.md target: \(target.path)."
                        )
                    )
                } else {
                    issues.append(
                        SkillValidationIssue(
                            code: "broken_skill_md_symlink",
                            message: "SKILL.md symlink is broken",
                            source: mainFile.path,
                            line: 1,
                            details: "Symlink target does not exist: \(target.path)."
                        )
                    )
                    return SkillValidationResult(issues: issues)
                }
            } else {
                issues.append(
                    SkillValidationIssue(
                        code: "broken_skill_md_symlink",
                        message: "SKILL.md symlink is broken",
                        source: mainFile.path,
                        line: 1,
                        details: "Symlink destination cannot be resolved."
                    )
                )
                return SkillValidationResult(issues: issues)
            }
        }

        guard fileManager.fileExists(atPath: mainFile.path), !isDirectory(mainFile) else {
            let issue = SkillValidationIssue(
                code: skill.packageType == "dir" ? "missing_skill_md" : "missing_main_file",
                message: skill.packageType == "dir"
                    ? "SKILL.md not found at \(mainFile.path)"
                    : "Main file not found at \(mainFile.path)",
                source: mainFile.path,
                line: 1,
                details: "The expected main skill file is missing on disk."
            )
            return SkillValidationResult(issues: [issue])
        }

        guard let raw = try? String(contentsOf: mainFile, encoding: .utf8) else {
            let issue = SkillValidationIssue(
                code: "unreadable_utf8_main_file",
                message: "Main file cannot be read as UTF-8",
                source: mainFile.path,
                line: 1,
                details: "Check encoding or file permissions."
            )
            return SkillValidationResult(issues: issues + [issue])
        }

        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty {
            issues.append(
                SkillValidationIssue(
                    code: "empty_main_file",
                    message: "Main file is empty",
                    source: mainFile.path,
                    line: 1,
                    details: "SKILL.md has no meaningful content."
                )
            )
            return SkillValidationResult(issues: issues)
        }

        let parsed = parseFrontmatterAndBody(raw)
        if !hasTitle(frontmatter: parsed.frontmatter, body: parsed.body) {
            issues.append(
                SkillValidationIssue(
                    code: "missing_title",
                    message: "No title found",
                    source: mainFile.path,
                    line: 1,
                    details: "Add frontmatter `title`/`name` or a top-level `#` heading."
                )
            )
        }

        let broken = brokenLocalReferences(in: raw, root: root)
        issues.append(
            contentsOf: broken.map {
                SkillValidationIssue(
                    code: "broken_reference",
                    message: "Broken reference: \($0.path)",
                    source: mainFile.path,
                    line: $0.line,
                    details: "Referenced path does not exist in this skill package."
                )
            }
        )

        issues.append(contentsOf: validateCodexVisibility(skill: skill))

        return SkillValidationResult(issues: issues)
    }

    private func resolveMainSkillFile(skill: SkillRecord) -> URL {
        let source = URL(fileURLWithPath: skill.canonicalSourcePath)
        if skill.packageType == "dir" {
            return source.appendingPathComponent("SKILL.md")
        }
        return source
    }

    private func skillRootURL(skill: SkillRecord, mainSkillFile: URL) -> URL {
        if skill.packageType == "dir" {
            return URL(fileURLWithPath: skill.canonicalSourcePath, isDirectory: true)
        }
        return mainSkillFile.deletingLastPathComponent()
    }

    private func parseFrontmatterAndBody(_ text: String) -> (frontmatter: [String: String], body: String) {
        let normalized = text.replacingOccurrences(of: "\r\n", with: "\n")
        guard normalized.hasPrefix("---\n") else {
            return ([:], normalized)
        }

        let start = normalized.index(normalized.startIndex, offsetBy: 4)
        guard let endRange = normalized.range(of: "\n---", range: start..<normalized.endIndex) else {
            return ([:], normalized)
        }
        let fmRaw = String(normalized[start..<endRange.lowerBound])
        let bodyStart = endRange.upperBound
        let body = String(normalized[bodyStart...]).trimmingCharacters(in: .whitespacesAndNewlines)

        var map: [String: String] = [:]
        for line in fmRaw.split(separator: "\n", omittingEmptySubsequences: false) {
            guard let colon = line.firstIndex(of: ":") else { continue }
            let key = line[..<colon].trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            guard !key.isEmpty else { continue }
            let value = line[line.index(after: colon)...]
                .trimmingCharacters(in: .whitespacesAndNewlines)
                .trimmingCharacters(in: CharacterSet(charactersIn: "\"'"))
            map[key] = value
        }

        return (map, body)
    }

    private func hasTitle(frontmatter: [String: String], body: String) -> Bool {
        if let title = cleaned(frontmatter["title"]), !title.isEmpty {
            return true
        }
        if let name = cleaned(frontmatter["name"]), !name.isEmpty {
            return true
        }

        for line in body.split(separator: "\n", omittingEmptySubsequences: false) {
            let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed.hasPrefix("# "), trimmed.count > 2 {
                return true
            }
        }
        return false
    }

    private func brokenLocalReferences(in text: String, root: URL) -> [BrokenReferenceHit] {
        let lines = text.components(separatedBy: .newlines)
        var missingByPath: [String: Int] = [:]

        let allCandidates = extractMarkdownLinks(from: lines)
            + extractBacktickPaths(from: lines)
            + extractReferencesFromCodeContext(extractCodeContexts(from: lines))

        for hit in allCandidates {
            guard let normalized = normalizeRelativePath(hit.candidate) else {
                continue
            }
            let fullPath = root.appendingPathComponent(normalized).path
            if !fileManager.fileExists(atPath: fullPath) {
                let current = missingByPath[normalized]
                if current == nil || hit.line < current! {
                    missingByPath[normalized] = hit.line
                }
            }
        }

        return missingByPath
            .map { BrokenReferenceHit(path: $0.key, line: $0.value) }
            .sorted { lhs, rhs in lhs.path < rhs.path }
    }

    private func extractMarkdownLinks(from lines: [String]) -> [CandidateHit] {
        extractCandidates(
            from: lines,
            pattern: "\\[[^\\]]+\\]\\(([^)]+)\\)",
            group: 1
        )
    }

    private func extractBacktickPaths(from lines: [String]) -> [CandidateHit] {
        extractCandidates(
            from: lines,
            pattern: "`((?:resources|references|scripts|assets)/[^`]+)`",
            group: 1
        )
    }

    private func extractCodeContexts(from lines: [String]) -> [CodeContext] {
        var contexts: [CodeContext] = []
        var inFence = false
        var fenceMarker: Character?

        for (index, lineText) in lines.enumerated() {
            let line = index + 1
            let trimmed = lineText.trimmingCharacters(in: .whitespaces)

            if let marker = fenceStartMarker(for: trimmed) {
                if !inFence {
                    inFence = true
                    fenceMarker = marker
                } else if fenceMarker == marker {
                    inFence = false
                    fenceMarker = nil
                }
                continue
            }

            if inFence {
                contexts.append(CodeContext(text: lineText, line: line))
                continue
            }

            let inlineRegex = try? NSRegularExpression(pattern: "`([^`]+)`")
            let range = NSRange(lineText.startIndex..<lineText.endIndex, in: lineText)
            inlineRegex?.enumerateMatches(in: lineText, range: range) { match, _, _ in
                guard let match else { return }
                guard let captureRange = Range(match.range(at: 1), in: lineText) else { return }
                let snippet = String(lineText[captureRange])
                contexts.append(CodeContext(text: snippet, line: line))
            }
        }

        return contexts
    }

    private func extractReferencesFromCodeContext(_ contexts: [CodeContext]) -> [CandidateHit] {
        let regex = try? NSRegularExpression(pattern: "\\bopen\\s+([A-Za-z0-9_./-]+)")
        var hits: [CandidateHit] = []

        for context in contexts {
            let range = NSRange(context.text.startIndex..<context.text.endIndex, in: context.text)
            regex?.enumerateMatches(in: context.text, range: range) { match, _, _ in
                guard let match else { return }
                guard let captureRange = Range(match.range(at: 1), in: context.text) else { return }
                let candidate = String(context.text[captureRange])
                guard isPathLikeCandidate(candidate) else {
                    return
                }
                hits.append(CandidateHit(candidate: candidate, line: context.line))
            }
        }

        return hits
    }

    private func extractCandidates(from lines: [String], pattern: String, group: Int) -> [CandidateHit] {
        let regex = try? NSRegularExpression(pattern: pattern)
        var hits: [CandidateHit] = []

        for (index, lineText) in lines.enumerated() {
            let line = index + 1
            let range = NSRange(lineText.startIndex..<lineText.endIndex, in: lineText)
            regex?.enumerateMatches(in: lineText, range: range) { match, _, _ in
                guard let match else { return }
                guard let captureRange = Range(match.range(at: group), in: lineText) else { return }
                hits.append(CandidateHit(candidate: String(lineText[captureRange]), line: line))
            }
        }

        return hits
    }

    private func fenceStartMarker(for trimmedLine: String) -> Character? {
        guard let first = trimmedLine.first, first == "`" || first == "~" else {
            return nil
        }
        let count = trimmedLine.prefix { $0 == first }.count
        return count >= 3 ? first : nil
    }

    private func isPathLikeCandidate(_ candidate: String) -> Bool {
        if candidate.hasPrefix("./") || candidate.hasPrefix("../") {
            return true
        }
        if candidate.contains("/") {
            return true
        }
        let extRegex = try? NSRegularExpression(pattern: "\\.[A-Za-z0-9]+$")
        let range = NSRange(candidate.startIndex..<candidate.endIndex, in: candidate)
        return extRegex?.firstMatch(in: candidate, range: range) != nil
    }

    private func normalizeRelativePath(_ value: String) -> String? {
        var candidate = value.trimmingCharacters(in: .whitespacesAndNewlines)
        candidate = candidate.trimmingCharacters(in: CharacterSet(charactersIn: "\"'`<>"))
        while let last = candidate.last, ".,;:".contains(last) {
            candidate.removeLast()
        }
        guard !candidate.isEmpty else { return nil }
        guard !candidate.hasPrefix("/") else { return nil }
        guard !candidate.contains("://") else { return nil }
        if candidate.hasPrefix("./") {
            candidate.removeFirst(2)
        }
        guard !candidate.isEmpty else { return nil }
        return candidate
    }

    private func isDirectory(_ url: URL) -> Bool {
        var isDir: ObjCBool = false
        return fileManager.fileExists(atPath: url.path, isDirectory: &isDir) && isDir.boolValue
    }

    private func isSymbolicLink(_ url: URL) -> Bool {
        (try? url.resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink) == true
    }

    private func symlinkDestination(of url: URL) -> URL? {
        guard let raw = try? fileManager.destinationOfSymbolicLink(atPath: url.path) else {
            return nil
        }
        if raw.hasPrefix("/") {
            return URL(fileURLWithPath: raw)
        }
        return url.deletingLastPathComponent().appendingPathComponent(raw).standardizedFileURL
    }

    private func cleaned(_ value: String?) -> String? {
        guard let value else { return nil }
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    private func validateCodexVisibility(skill: SkillRecord) -> [SkillValidationIssue] {
        var issues: [SkillValidationIssue] = []

        if skill.status == .archived {
            issues.append(
                SkillValidationIssue(
                    code: "archived_skill_not_visible_in_codex",
                    message: "Archived skill is hidden from Codex",
                    source: skill.canonicalSourcePath,
                    line: 1,
                    details: "Archived skills are not shown in active Codex skill lists."
                )
            )
        }

        guard let codexTargetPath = codexTargetPath(for: skill) else {
            issues.append(
                SkillValidationIssue(
                    code: "codex_target_not_declared",
                    message: "Codex target path is missing",
                    source: skill.canonicalSourcePath,
                    line: 1,
                    details: "No path like .../.codex/skills/\(skill.skillKey) was found in target_paths."
                )
            )
            return issues
        }

        let codexTargetURL = URL(fileURLWithPath: codexTargetPath, isDirectory: true)
        let targetExists = fileManager.fileExists(atPath: codexTargetURL.path) || isSymbolicLink(codexTargetURL)
        guard targetExists else {
            issues.append(
                SkillValidationIssue(
                    code: "codex_target_missing_on_disk",
                    message: "Codex target path does not exist",
                    source: codexTargetURL.path,
                    line: 1,
                    details: "Codex target path is declared but missing on disk: \(codexTargetURL.path)."
                )
            )
            return issues
        }

        if isSymbolicLink(codexTargetURL) {
            guard let destination = symlinkDestination(of: codexTargetURL), fileManager.fileExists(atPath: destination.path) else {
                issues.append(
                    SkillValidationIssue(
                        code: "codex_target_broken_symlink",
                        message: "Codex target symlink is broken",
                        source: codexTargetURL.path,
                        line: 1,
                        details: "Codex target points to a missing destination."
                    )
                )
                return issues
            }
        }

        let codexSkillFile = codexTargetURL.appendingPathComponent("SKILL.md")
        guard fileManager.fileExists(atPath: codexSkillFile.path), !isDirectory(codexSkillFile) else {
            issues.append(
                SkillValidationIssue(
                    code: "codex_target_missing_skill_md",
                    message: "Codex target misses SKILL.md",
                    source: codexTargetURL.path,
                    line: 1,
                    details: "Codex can discover this package only if SKILL.md exists at \(codexSkillFile.path)."
                )
            )
            return issues
        }

        guard let codexRaw = try? String(contentsOf: codexSkillFile, encoding: .utf8) else {
            issues.append(
                SkillValidationIssue(
                    code: "codex_target_missing_skill_md",
                    message: "Codex target SKILL.md is unreadable",
                    source: codexSkillFile.path,
                    line: 1,
                    details: "SKILL.md exists but cannot be read as UTF-8."
                )
            )
            return issues
        }

        if let invalidYAMLIssue = codexFrontmatterInvalidYAMLIssue(skillFile: codexSkillFile, raw: codexRaw) {
            issues.append(invalidYAMLIssue)
            return issues
        }

        let parsed = parseFrontmatterAndBody(codexRaw)
        if cleaned(parsed.frontmatter["name"]) == nil {
            issues.append(
                SkillValidationIssue(
                    code: "missing_frontmatter_name",
                    message: "Frontmatter `name` is required",
                    source: codexSkillFile.path,
                    line: 1,
                    details: "Codex visibility metadata requires frontmatter `name` in SKILL.md."
                )
            )
        }

        if cleaned(parsed.frontmatter["description"]) == nil {
            issues.append(
                SkillValidationIssue(
                    code: "missing_frontmatter_description",
                    message: "Frontmatter `description` is required",
                    source: codexSkillFile.path,
                    line: 1,
                    details: "Codex visibility metadata requires frontmatter `description` in SKILL.md."
                )
            )
        }

        if let rawName = cleaned(parsed.frontmatter["name"]),
           normalizedSkillKey(rawName) != normalizedSkillKey(skill.skillKey) {
            issues.append(
                SkillValidationIssue(
                    code: "frontmatter_name_mismatch_skill_key",
                    message: "Frontmatter `name` does not match skill key",
                    source: codexSkillFile.path,
                    line: 1,
                    details: "Found name '\(rawName)', expected key '\(skill.skillKey)'. Codex may not match this skill consistently."
                )
            )
        }

        return issues
    }

    private func codexTargetPath(for skill: SkillRecord) -> String? {
        for path in skill.targetPaths {
            let standardized = URL(fileURLWithPath: path).standardizedFileURL.path
            if standardized.contains("/.codex/skills/"), standardized.hasSuffix("/\(skill.skillKey)") {
                return standardized
            }
        }
        return nil
    }

    private func normalizedSkillKey(_ value: String) -> String {
        let lowered = value.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        var normalized = lowered.replacingOccurrences(of: " ", with: "-").replacingOccurrences(of: "_", with: "-")
        while normalized.contains("--") {
            normalized = normalized.replacingOccurrences(of: "--", with: "-")
        }
        return normalized
    }

    private func codexFrontmatterInvalidYAMLIssue(skillFile: URL, raw: String) -> SkillValidationIssue? {
        guard let frontmatter = extractFrontmatterBlock(from: raw) else {
            return nil
        }

        let lines = frontmatter.raw.components(separatedBy: .newlines)
        for (index, line) in lines.enumerated() {
            let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed.isEmpty || trimmed.hasPrefix("#") {
                continue
            }
            guard let colonIndex = line.firstIndex(of: ":") else {
                return SkillValidationIssue(
                    code: "codex_frontmatter_invalid_yaml",
                    message: "Frontmatter is not valid YAML for Codex",
                    source: skillFile.path,
                    line: frontmatter.startLine + index,
                    details: "Line '\(trimmed)' is missing a key/value separator (:)."
                )
            }

            let key = line[..<colonIndex].trimmingCharacters(in: .whitespacesAndNewlines)
            if key.isEmpty {
                return SkillValidationIssue(
                    code: "codex_frontmatter_invalid_yaml",
                    message: "Frontmatter is not valid YAML for Codex",
                    source: skillFile.path,
                    line: frontmatter.startLine + index,
                    details: "A frontmatter key is empty before ':'."
                )
            }

            let valueStart = line.index(after: colonIndex)
            let value = line[valueStart...].trimmingCharacters(in: .whitespacesAndNewlines)
            if value.isEmpty {
                continue
            }
            if isUnquotedScalar(value), isCodexBreakingPlainScalar(value) {
                return SkillValidationIssue(
                    code: "codex_frontmatter_invalid_yaml",
                    message: "Frontmatter is not valid YAML for Codex",
                    source: skillFile.path,
                    line: frontmatter.startLine + index,
                    details: "Field '\(key)' uses unquoted value '\(value)' which breaks Codex YAML parsing. Wrap the value in quotes."
                )
            }
        }

        return nil
    }

    private func extractFrontmatterBlock(from text: String) -> FrontmatterBlock? {
        let normalized = text.replacingOccurrences(of: "\r\n", with: "\n")
        guard normalized.hasPrefix("---\n") else {
            return nil
        }

        let afterStart = normalized.index(normalized.startIndex, offsetBy: 4)
        guard let endRange = normalized.range(of: "\n---", range: afterStart..<normalized.endIndex) else {
            return nil
        }

        let fmRaw = String(normalized[afterStart..<endRange.lowerBound])
        return FrontmatterBlock(raw: fmRaw, startLine: 2)
    }

    private func isUnquotedScalar(_ value: String) -> Bool {
        guard let first = value.first else {
            return false
        }
        return first != "\"" && first != "'"
    }

    private func isCodexBreakingPlainScalar(_ value: String) -> Bool {
        if value.contains("] [") {
            return true
        }
        if value.hasPrefix("[") && !isLikelyYAMLFlowSequence(value) {
            return true
        }
        return false
    }

    private func isLikelyYAMLFlowSequence(_ value: String) -> Bool {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix("["), trimmed.hasSuffix("]") else {
            return false
        }
        var balance = 0
        for char in trimmed {
            if char == "[" { balance += 1 }
            if char == "]" { balance -= 1 }
            if balance < 0 { return false }
        }
        return balance == 0
    }
}
