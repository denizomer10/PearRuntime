import Foundation

public struct Finding: Codable, Hashable, Sendable {
    public enum Severity: Int, Codable, Comparable, Sendable {
        case info, warning, problem, critical
        public static func < (lhs: Severity, rhs: Severity) -> Bool { lhs.rawValue < rhs.rawValue }
        public var label: String {
            switch self { case .info: "info"; case .warning: "warning"; case .problem: "problem"; case .critical: "critical" }
        }
    }

    public var id: String
    public var severity: Severity
    public var title: String
    public var detail: String
    /// What to try next, in plain words.
    public var suggestion: String?
    public var lines: [Int]

    public init(id: String, severity: Severity, title: String, detail: String,
                suggestion: String? = nil, lines: [Int] = []) {
        self.id = id
        self.severity = severity
        self.title = title
        self.detail = detail
        self.suggestion = suggestion
        self.lines = lines
    }
}

/// A rule looks at the parsed events of one log and reports findings. Rules are
/// grouped into modules (`RuleModule`) per component so a component can ship and
/// version its own knowledge.
public protocol DiagnosticRule: Sendable {
    var id: String { get }
    var sources: Set<LogSource> { get }
    func evaluate(_ events: [LogEvent]) -> [Finding]
}

public struct RuleModule: Sendable {
    public var name: String
    public var rules: [any DiagnosticRule]
    public init(name: String, rules: [any DiagnosticRule]) {
        self.name = name
        self.rules = rules
    }
}

public struct Analysis: Codable, Sendable {
    public var source: LogSource
    public var lineCount: Int
    public var levelCounts: [String: Int]
    public var topAreas: [String: Int]
    public var findings: [Finding]

    public init(source: LogSource, lineCount: Int, levelCounts: [String: Int], topAreas: [String: Int], findings: [Finding]) {
        self.source = source
        self.lineCount = lineCount
        self.levelCounts = levelCounts
        self.topAreas = topAreas
        self.findings = findings
    }
}

public struct Analyzer: Sendable {
    public var modules: [RuleModule]

    public init(modules: [RuleModule] = Analyzer.builtInModules) {
        self.modules = modules
    }

    public static let builtInModules: [RuleModule] = [
        LegacyEngineRules.module, RuntimeRules.module, WineRules.module, CommonRules.module,
    ]

    public func analyze(_ text: String, source: LogSource? = nil) -> Analysis {
        let resolved = source ?? LogSource.detect(from: Substring(text))
        let events = LogParser(source: resolved).parse(text)
        var levels: [String: Int] = [:]
        var areas: [String: Int] = [:]
        for event in events {
            levels[String(describing: event.level), default: 0] += 1
            if !event.area.isEmpty { areas["\(event.source.rawValue):\(event.area)", default: 0] += 1 }
        }
        let topAreas = Dictionary(uniqueKeysWithValues: areas.sorted { $0.value > $1.value }.prefix(12).map { ($0.key, $0.value) })
        var findings: [Finding] = []
        for module in modules {
            for rule in module.rules where rule.sources.contains(resolved) || rule.sources.isEmpty {
                findings.append(contentsOf: rule.evaluate(events))
            }
        }
        findings.sort { ($0.severity, $1.id) > ($1.severity, $0.id) }
        return Analysis(source: resolved, lineCount: events.count, levelCounts: levels,
                        topAreas: topAreas, findings: findings)
    }
}

/// A rule defined by a predicate over single events, aggregated into one finding.
public struct PatternRule: DiagnosticRule {
    public var id: String
    public var sources: Set<LogSource>
    public var severity: Finding.Severity
    public var title: String
    public var suggestion: String?
    public var minimumHits: Int
    public var matches: @Sendable (LogEvent) -> Bool

    public init(id: String, sources: Set<LogSource>, severity: Finding.Severity, title: String,
                suggestion: String? = nil, minimumHits: Int = 1,
                matches: @escaping @Sendable (LogEvent) -> Bool) {
        self.id = id
        self.sources = sources
        self.severity = severity
        self.title = title
        self.suggestion = suggestion
        self.minimumHits = minimumHits
        self.matches = matches
    }

    public func evaluate(_ events: [LogEvent]) -> [Finding] {
        let hits = events.filter(matches)
        guard hits.count >= minimumHits else { return [] }
        let sample = hits.prefix(3).map { "line \($0.line): \($0.message.prefix(160))" }.joined(separator: "\n")
        return [Finding(id: id, severity: severity, title: title,
                        detail: "\(hits.count) occurrence(s).\n\(sample)",
                        suggestion: suggestion, lines: hits.prefix(20).map(\.line))]
    }
}

/// Counts unimplemented/TODO calls and lists the most frequent ones.
struct UnimplementedRule: DiagnosticRule {
    let id = "legacy.unimplemented"
    let sources: Set<LogSource> = [.legacyEngine]

    func evaluate(_ events: [LogEvent]) -> [Finding] {
        var counts: [String: (Int, Int)] = [:]
        for event in events where event.message.hasPrefix("TODO") || event.message.contains("unimplemented") {
            let key = String(event.message.prefix(80))
            let current = counts[key] ?? (0, event.line)
            counts[key] = (current.0 + 1, current.1)
        }
        guard !counts.isEmpty else { return [] }
        let top = counts.sorted { $0.value.0 > $1.value.0 }.prefix(8)
        let detail = top.map { "\($0.value.0)× \($0.key)" }.joined(separator: "\n")
        return [Finding(id: id, severity: .info, title: "Unimplemented engine functions were called",
                        detail: detail,
                        suggestion: "Calls that repeat thousands of times are the first candidates when a game misbehaves.",
                        lines: top.map(\.value.1))]
    }
}

/// Detects frame presentation stopping while the guest keeps running.
struct PresentStallRule: DiagnosticRule {
    let id = "legacy.present-stall"
    let sources: Set<LogSource> = [.legacyEngine]

    func evaluate(_ events: [LogEvent]) -> [Finding] {
        guard let lastFPS = events.lastIndex(where: { $0.message.contains("FPS:") }) else { return [] }
        let after = events.count - lastFPS - 1
        guard after > 200 else { return [] }
        return [Finding(id: id, severity: .problem,
                        title: "Frame output stopped while the app kept running",
                        detail: "\(after) log lines after the last FPS report (line \(events[lastFPS].line)).",
                        suggestion: "Attach `peardebug legacy probe` when the screen goes black to see which guest functions are being polled.",
                        lines: [events[lastFPS].line])]
    }
}

enum LegacyEngineRules {
    static let module = RuleModule(name: "PearLegacyEngine", rules: [
        PatternRule(id: "legacy.panic", sources: [.legacyEngine], severity: .critical,
                    title: "The engine panicked",
                    suggestion: "The panic message names the source file and line in PearLegacyEngine.") {
            $0.area == "panic" || $0.message.contains("panicked at")
        },
        PatternRule(id: "legacy.cri-mana", sources: [.legacyEngine], severity: .problem,
                    title: "CRI Mana movie player error",
                    suggestion: "The game's movie player did not reach STOP/PLAYEND. Check audio unit pulls and file EOF handling.") {
            $0.area == "cri" || $0.message.contains("CriMvPly") || $0.message.contains("Decoder core error")
        },
        PatternRule(id: "legacy.missing-font", sources: [.legacyEngine], severity: .info,
                    title: "Fonts were substituted", minimumHits: 1) {
            $0.message.contains("No replacement found for font")
        },
        PatternRule(id: "legacy.missing-file", sources: [.legacyEngine], severity: .warning,
                    title: "Guest file opens failed", minimumHits: 1) {
            $0.message.contains("open(") && $0.message.contains("failed")
        },
        PatternRule(id: "legacy.null-page", sources: [.legacyEngine], severity: .critical,
                    title: "The game read or wrote the null page",
                    suggestion: "A pointer the game expected to be set was empty, usually because an earlier load failed. Capture registers and the call chain with `peardebug legacy crash` (engine started with --gdb), then look at what that pointer should have come from.") {
            $0.message.contains("Attempted null-page access")
        },
        PatternRule(id: "legacy.open-directory", sources: [.legacyEngine], severity: .warning,
                    title: "The game opened a folder where it expected a file",
                    suggestion: "Usually an asset lookup that produced an empty file name (for example an archive index miss). Run with PEAR_TRACE_OPEN=1 to see every path the game opens.") {
            $0.message.contains("open() of directory") || $0.message.contains("Seeking a directory as a guest file")
        },
        UnimplementedRule(),
        PresentStallRule(),
    ])
}

enum RuntimeRules {
    static let module = RuleModule(name: "PearRuntime", rules: [
        PatternRule(id: "runtime.controller-broadcast", sources: [.runtime], severity: .problem,
                    title: "Controller connects were re-posted to every observer",
                    suggestion: "Unreal and Unity games then move the pad to player 2. Update PearRuntime to 0.8.0 or later (targeted delivery).") {
            $0.area == "input" && ($0.message.hasPrefix("replaying controller connect") || $0.message.hasPrefix("broadcast replay"))
        },
        PatternRule(id: "runtime.controller-wait", sources: [.runtime], severity: .warning,
                    title: "Controller queries blocked the main thread",
                    suggestion: "Each 'controller query waited' froze the app for up to 1.2 s. PearRuntime 0.8.0 only waits once, during launch.",
                    minimumHits: 2) {
            $0.area == "input" && $0.message.hasPrefix("controller query waited")
        },
        PatternRule(id: "runtime.no-controller", sources: [.runtime], severity: .info,
                    title: "No game controller was connected at launch") {
            $0.area == "input" && $0.message == "game controllers at bootstrap: 0"
        },
        PatternRule(id: "runtime.native-controller", sources: [.runtime], severity: .info,
                    title: "The game drives controllers itself") {
            $0.area == "input" && $0.message.hasPrefix("app drives controllers natively")
        },
        PatternRule(id: "runtime.virtual-pad-failed", sources: [.runtime], severity: .problem,
                    title: "The keyboard-to-gamepad controller could not connect") {
            $0.area == "virtual-pad" && ($0.message.contains("failed") || $0.message.contains("unavailable"))
        },
        PatternRule(id: "runtime.crash-signal", sources: [.runtime], severity: .critical,
                    title: "The app crashed (signal)") {
            $0.message.contains("fatal signal") || $0.message.contains("crash signal")
        },
    ])
}

enum WineRules {
    static let module = RuleModule(name: "Wine", rules: [
        PatternRule(id: "wine.missing-dll", sources: [.wine], severity: .problem,
                    title: "A Windows DLL could not be loaded",
                    suggestion: "Install the matching redistributable in the prefix (for example vcrun or d3dx), or switch engine.") {
            $0.source == .wine && $0.area == "module" && $0.level == .error
        },
        PatternRule(id: "wine.graphics", sources: [.wine], severity: .problem,
                    title: "Graphics layer errors (DXVK / DXMT / MoltenVK / D3DMetal)",
                    suggestion: "Try another graphics backend for this game, or the fallback engine.") {
            let message = $0.message.lowercased()
            return $0.level >= .error && (message.contains("dxvk") || message.contains("vulkan")
                || message.contains("moltenvk") || message.contains("metal") || $0.area == "d3d")
        },
        PatternRule(id: "wine.page-fault", sources: [.wine], severity: .critical,
                    title: "The Windows program crashed (page fault)",
                    suggestion: "If this repeats, the engine fallback list in PearSilicon tries the next Wine build automatically.") {
            $0.message.contains("Unhandled page fault") || $0.message.contains("unhandled exception")
        },
        PatternRule(id: "wine.rosetta", sources: [.wine], severity: .problem,
                    title: "Rosetta 2 is missing or failed",
                    suggestion: "Install Rosetta (`softwareupdate --install-rosetta`) or use the native arm64 MetalSharp Wine engine.") {
            $0.message.contains("Bad CPU type") || $0.message.lowercased().contains("rosetta")
        },
        PatternRule(id: "wine.steam-cef-storage", sources: [.wine], severity: .critical,
                    title: "Steam's browser (CEF StorageService) failed to start",
                    suggestion: "MetalSharp's launcher rule for \\Steam\\bin\\cef\\ adds flags to every Steam CEF child. Start Steam from PearSilicon 0.8.6 or later, which runs it through C:\\PearSiliconSteam. `peardebug steam <Steam folder>` checks the client's state.") {
            $0.message.contains("target_services.cc") || $0.message.contains("process_state_.InitCalled")
        },
        PatternRule(id: "wine.breakpoint", sources: [.wine], severity: .info,
                    title: "Programs hit breakpoint exceptions (0x80000003)",
                    suggestion: "Chromium/CEF and some launchers raise these on purpose; they matter only if the program then closes. Check whether its window stays open.",
                    minimumHits: 3) {
            $0.message.contains("Unhandled exception 0x80000003")
        },
        PatternRule(id: "wine.mullion-flight", sources: [.wine], severity: .warning,
                    title: "MetalSharp recorded an exception (Mullion flight record)",
                    suggestion: "The .flight file named in the line holds the exception context; it is under the entry's Data/Library/Logs/Mullion.") {
            $0.message.contains("mullion: flight record")
        },
    ])
}

enum CommonRules {
    static let module = RuleModule(name: "Common", rules: [
        PatternRule(id: "common.fatal", sources: [], severity: .critical, title: "Fatal errors") {
            $0.level == .fatal && $0.area != "panic"
        },
        PatternRule(id: "host.update-commit-mismatch", sources: [], severity: .problem,
                    title: "An update was rejected: its app was built from another commit than its release tag",
                    suggestion: "The release ZIP was built before the commit it was tagged with. Publish with scripts/publish-release.sh, which builds from the tagged commit and verifies the uploaded app.") {
            $0.message.contains("PearBuildCommit disagrees")
        },
        PatternRule(id: "host.github-rate-limit", sources: [], severity: .warning,
                    title: "GitHub's API rate limit was reached",
                    suggestion: "Anonymous API use is limited to 60 requests per hour. Engine downloads fall back to the public release pages; wait an hour if something still fails.") {
            let message = $0.message
            return message.contains("rate limit") || message.contains("rate-limiting requests")
        },
    ])
}
