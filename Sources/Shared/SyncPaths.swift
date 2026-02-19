import Foundation

enum SyncPaths {
    static let groupIdentifier = "group.dev.fedosov.skillssync"

    static var fallbackContainerURL: URL {
        let appSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSHomeDirectory())
            .appendingPathComponent("Library")
            .appendingPathComponent("Application Support")
        return appSupport.appendingPathComponent("SkillsSync", isDirectory: true)
    }

    static var groupContainerURL: URL {
        if let override = ProcessInfo.processInfo.environment["SKILLS_SYNC_GROUP_DIR"], !override.isEmpty {
            return URL(fileURLWithPath: override)
        }

        if let container = FileManager.default.containerURL(
            forSecurityApplicationGroupIdentifier: groupIdentifier
        ) {
            return container
        }

        // Keep fallback inside user Application Support to avoid prompting for external folder access.
        return fallbackContainerURL
    }

    static var stateURL: URL {
        groupContainerURL.appendingPathComponent("state.json")
    }

    static var commandQueueURL: URL {
        groupContainerURL.appendingPathComponent("commands.jsonl")
    }
}
