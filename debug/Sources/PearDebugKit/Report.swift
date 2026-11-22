import Foundation

public enum ReportFormat: String, Sendable, CaseIterable {
    case text, markdown, json
}

public enum Report {
    public static func render(_ analysis: Analysis, title: String, format: ReportFormat) -> String {
        switch format {
        case .json:
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            return (try? String(decoding: encoder.encode(analysis), as: UTF8.self)) ?? "{}"
        case .markdown:
            var out = "# \(title)\n\n"
            out += "Source: **\(analysis.source.displayName)** · \(analysis.lineCount) lines\n\n"
            if analysis.findings.isEmpty { out += "No problems found.\n" }
            for finding in analysis.findings {
                out += "## [\(finding.severity.label)] \(finding.title)\n\n```\n\(finding.detail)\n```\n"
                if let suggestion = finding.suggestion { out += "\n\(suggestion)\n" }
                out += "\n"
            }
            return out
        case .text:
            var out = "\(title)\n\(analysis.source.displayName), \(analysis.lineCount) lines\n"
            let levels = analysis.levelCounts.sorted { $0.key < $1.key }.map { "\($0.key)=\($0.value)" }
            out += "levels: \(levels.joined(separator: " "))\n"
            if analysis.findings.isEmpty {
                out += "\nNo problems found.\n"
            }
            for finding in analysis.findings {
                out += "\n[\(finding.severity.label.uppercased())] \(finding.title)\n"
                for line in finding.detail.split(separator: "\n") { out += "    \(line)\n" }
                if let suggestion = finding.suggestion { out += "  → \(suggestion)\n" }
            }
            return out
        }
    }
}

/// macOS crash reports (`.ips`) for a process, newest first.
public enum CrashReports {
    public static var directories: [URL] {
        let home = FileManager.default.homeDirectoryForCurrentUser
        return [home.appendingPathComponent("Library/Logs/DiagnosticReports"),
                URL(fileURLWithPath: "/Library/Logs/DiagnosticReports")]
    }

    public static func reports(matching processName: String, limit: Int = 5) -> [URL] {
        var found: [(URL, Date)] = []
        for directory in directories {
            guard let items = try? FileManager.default.contentsOfDirectory(
                at: directory, includingPropertiesForKeys: [.contentModificationDateKey]) else { continue }
            for url in items where url.lastPathComponent.hasPrefix(processName)
                && ["ips", "crash"].contains(url.pathExtension) {
                let date = (try? url.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate) ?? .distantPast
                found.append((url, date))
            }
        }
        return found.sorted { $0.1 > $1.1 }.prefix(limit).map(\.0)
    }

    /// Short summary of an `.ips` report: exception type and the crashed thread's top frames.
    public static func summarize(_ url: URL) -> String {
        guard let text = try? String(contentsOf: url, encoding: .utf8) else { return url.lastPathComponent }
        // .ips = one JSON header line, then a JSON body.
        let body = text.split(separator: "\n", maxSplits: 1).last.map(String.init) ?? text
        guard let data = body.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return url.lastPathComponent
        }
        var lines = [url.lastPathComponent]
        if let exception = json["exception"] as? [String: Any] {
            lines.append("exception: \(exception["type"] ?? "?") \(exception["signal"] ?? "") \(exception["subtype"] ?? "")")
        }
        if let threads = json["threads"] as? [[String: Any]],
           let crashed = threads.first(where: { ($0["triggered"] as? Bool) == true }),
           let frames = crashed["frames"] as? [[String: Any]] {
            let images = json["usedImages"] as? [[String: Any]] ?? []
            for frame in frames.prefix(8) {
                let index = frame["imageIndex"] as? Int ?? -1
                let image = index >= 0 && index < images.count ? (images[index]["name"] as? String ?? "?") : "?"
                lines.append("  \(image) \(frame["symbol"] as? String ?? String(format: "+%@", "\(frame["imageOffset"] ?? "?")"))")
            }
        }
        return lines.joined(separator: "\n")
    }
}
