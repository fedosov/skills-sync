import Foundation

struct SyncAppSettings: Codable, Equatable {
    let version: Int
    let autoMigrateToCanonicalSource: Bool
    let workspaceDiscoveryRoots: [String]
    let windowState: AppWindowState?
    let uiState: AppUIState?

    enum CodingKeys: String, CodingKey {
        case version
        case autoMigrateToCanonicalSource = "auto_migrate_to_canonical_source"
        case workspaceDiscoveryRoots = "workspace_discovery_roots"
        case windowState = "window_state"
        case uiState = "ui_state"
    }

    static let `default` = SyncAppSettings(
        version: 2,
        autoMigrateToCanonicalSource: false,
        workspaceDiscoveryRoots: [],
        windowState: nil,
        uiState: nil
    )

    init(
        version: Int,
        autoMigrateToCanonicalSource: Bool,
        workspaceDiscoveryRoots: [String],
        windowState: AppWindowState?,
        uiState: AppUIState?
    ) {
        self.version = version
        self.autoMigrateToCanonicalSource = autoMigrateToCanonicalSource
        self.workspaceDiscoveryRoots = workspaceDiscoveryRoots
        self.windowState = windowState
        self.uiState = uiState
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        version = try container.decodeIfPresent(Int.self, forKey: .version) ?? 1
        autoMigrateToCanonicalSource = try container.decodeIfPresent(Bool.self, forKey: .autoMigrateToCanonicalSource) ?? false
        workspaceDiscoveryRoots = try container.decodeIfPresent([String].self, forKey: .workspaceDiscoveryRoots) ?? []
        windowState = try container.decodeIfPresent(AppWindowState.self, forKey: .windowState)
        uiState = try container.decodeIfPresent(AppUIState.self, forKey: .uiState)
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(version, forKey: .version)
        try container.encode(autoMigrateToCanonicalSource, forKey: .autoMigrateToCanonicalSource)
        try container.encode(workspaceDiscoveryRoots, forKey: .workspaceDiscoveryRoots)
        try container.encodeIfPresent(windowState, forKey: .windowState)
        try container.encodeIfPresent(uiState, forKey: .uiState)
    }
}

struct AppWindowState: Codable, Equatable {
    let x: Double
    let y: Double
    let width: Double
    let height: Double
    let isMaximized: Bool

    enum CodingKeys: String, CodingKey {
        case x
        case y
        case width
        case height
        case isMaximized = "is_maximized"
    }
}

struct AppUIState: Codable, Equatable {
    let sidebarWidth: Double?
    let scopeFilter: String
    let searchText: String
    let selectedSkillIDs: [String]

    enum CodingKeys: String, CodingKey {
        case sidebarWidth = "sidebar_width"
        case scopeFilter = "scope_filter"
        case searchText = "search_text"
        case selectedSkillIDs = "selected_skill_ids"
    }
}

struct SyncPreferencesStore {
    private let decoder = JSONDecoder()
    private let encoder = JSONEncoder()

    init() {
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
    }

    func loadSettings() -> SyncAppSettings {
        let url = SyncPaths.appSettingsURL
        guard let data = try? Data(contentsOf: url),
              let settings = try? decoder.decode(SyncAppSettings.self, from: data) else {
            return .default
        }
        return settings
    }

    func saveSettings(_ settings: SyncAppSettings) {
        do {
            try FileManager.default.createDirectory(at: SyncPaths.runtimeDirectoryURL, withIntermediateDirectories: true)
            let normalized = SyncAppSettings(
                version: 2,
                autoMigrateToCanonicalSource: settings.autoMigrateToCanonicalSource,
                workspaceDiscoveryRoots: settings.workspaceDiscoveryRoots,
                windowState: settings.windowState,
                uiState: settings.uiState
            )
            let data = try encoder.encode(normalized)
            try data.write(to: SyncPaths.appSettingsURL, options: [.atomic])
        } catch {
            // Preferences persistence should never crash sync/UI flows.
        }
    }
}
