import Foundation

/// The component that produced a log. Every PearDebug module is keyed by one.
public enum LogSource: String, Codable, CaseIterable, Sendable {
    /// PearLegacyEngine (32-bit iOS, touchHLE-derived) process output.
    case legacyEngine = "legacy"
    /// PearRuntime inside a modern arm64 iOS app.
    case runtime
    /// Wine (MetalSharp, WineHQ) and its graphics layers (DXVK, DXMT, MoltenVK).
    case wine
    /// A native or managed macOS app launched by PearSilicon.
    case macApp = "mac"
    /// PearSilicon's own launcher/host messages.
    case host

    public var displayName: String {
        switch self {
        case .legacyEngine: "PearLegacyEngine"
        case .runtime: "PearRuntime"
        case .wine: "Wine"
        case .macApp: "macOS app"
        case .host: "PearSilicon host"
        }
    }

    /// File name suffix used by `LogStore` (kept compatible with PearSilicon's
    /// historical names: `<id>.log`, `<id>.legacy.log`, `<id>.native.log`).
    public var fileSuffix: String {
        switch self {
        case .runtime: ""
        case .legacyEngine: ".legacy"
        case .wine: ".wine"
        case .macApp: ".native"
        case .host: ".host"
        }
    }

    /// Guesses the source from log content when the caller does not know it.
    public static func detect(from text: Substring) -> LogSource {
        let sample = text.prefix(64 * 1024)
        if sample.contains("PearLegacyEngine::") || sample.contains("touchHLE") { return .legacyEngine }
        if sample.contains("[PearRuntime]") { return .runtime }
        if sample.contains(":fixme:") || sample.contains(":err:") || sample.contains("wineserver")
            || sample.contains("wine: ") { return .wine }
        if sample.contains("[PearSilicon]") { return .host }
        return .macApp
    }
}

public enum LogLevel: Int, Codable, Comparable, Sendable {
    case trace, info, warning, error, fatal
    public static func < (lhs: LogLevel, rhs: LogLevel) -> Bool { lhs.rawValue < rhs.rawValue }
}

/// One parsed log line.
public struct LogEvent: Codable, Hashable, Sendable {
    public var line: Int
    public var source: LogSource
    public var level: LogLevel
    /// Component inside the source, such as `input` for `[PearRuntime][input]`,
    /// `frameworks::uikit::ui_font` for the legacy engine or `module` for Wine.
    public var area: String
    public var message: String

    public init(line: Int, source: LogSource, level: LogLevel, area: String, message: String) {
        self.line = line
        self.source = source
        self.level = level
        self.area = area
        self.message = message
    }
}
