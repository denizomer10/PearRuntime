import Foundation

/// Turns raw log text from any Pear component into `LogEvent`s. Parsing is
/// line-based and allocation-light so multi-hundred-megabyte logs stay usable.
public struct LogParser: Sendable {
    public var source: LogSource?

    public init(source: LogSource? = nil) {
        self.source = source
    }

    public func parse(_ text: String) -> [LogEvent] {
        let resolved = source ?? LogSource.detect(from: Substring(text))
        var events: [LogEvent] = []
        var number = 0
        text.enumerateLines { line, _ in
            number += 1
            guard !line.isEmpty else { return }
            events.append(Self.parseLine(line, number: number, source: resolved))
        }
        return events
    }

    public static func parseLine(_ line: String, number: Int, source: LogSource) -> LogEvent {
        // [PearRuntime][area] message   (also accepted inside any source)
        if let range = line.range(of: "[PearRuntime]") {
            let rest = line[range.upperBound...]
            var area = "core"
            var message = Substring(rest)
            if rest.hasPrefix("["), let close = rest.firstIndex(of: "]") {
                area = String(rest[rest.index(after: rest.startIndex)..<close])
                message = rest[rest.index(after: close)...]
            }
            let text = message.trimmingCharacters(in: .whitespaces)
            return LogEvent(line: number, source: .runtime, level: level(for: text), area: area, message: text)
        }

        // PearLegacyEngine::frameworks::uikit::ui_font: message
        if let range = line.range(of: "PearLegacyEngine::") {
            let rest = line[range.upperBound...]
            if let colon = rest.range(of: ": ") {
                let area = String(rest[..<colon.lowerBound])
                let text = String(rest[colon.upperBound...])
                return LogEvent(line: number, source: .legacyEngine, level: level(for: text), area: area, message: text)
            }
        }

        // Rust panics from the legacy engine.
        if line.hasPrefix("thread '") && line.contains("panicked at") {
            return LogEvent(line: number, source: .legacyEngine, level: .fatal, area: "panic", message: line)
        }

        // Wine: 0124:err:module:import_dll Library X not found
        if source == .wine || line.contains(":fixme:") || line.contains(":err:") || line.contains(":warn:") {
            let parts = line.split(separator: ":", maxSplits: 3, omittingEmptySubsequences: false)
            if parts.count == 4, parts[0].count <= 5, ["fixme", "err", "warn", "trace"].contains(parts[1]) {
                let level: LogLevel = switch parts[1] {
                case "err": .error
                case "warn": .warning
                case "fixme": .info
                default: .trace
                }
                return LogEvent(line: number, source: .wine, level: level, area: String(parts[2]),
                                message: String(parts[3]))
            }
        }

        // CRI middleware error codes (E05121602M : ...), seen in SEGA titles.
        if line.count > 10, line.first == "E", line.dropFirst().prefix(8).allSatisfy(\.isNumber) {
            return LogEvent(line: number, source: source, level: .error, area: "cri", message: line)
        }

        return LogEvent(line: number, source: source, level: level(for: line), area: "", message: line)
    }

    static func level(for text: String) -> LogLevel {
        let lower = text.lowercased()
        if lower.contains("panicked") || lower.contains("fatal") || lower.contains("terminating app") {
            return .fatal
        }
        if lower.hasPrefix("error") || lower.contains(" error") || lower.contains("failed") || lower.contains("crash") {
            return .error
        }
        if lower.hasPrefix("warning") || lower.contains("todo") || lower.contains("unimplemented")
            || lower.contains("not found") {
            return .warning
        }
        return .info
    }
}
