# PearDebug

**Logging, log analysis and live probes for the Pear stack.**

PearDebug collects and explains what happens when
[PearSilicon](https://github.com/denizomer10/PearSilicon) launches a game: iOS
apps on [PearRuntime](https://github.com/denizomer10/PearRuntime), 32-bit iOS
games on [PearLegacyEngine](../third_party/legacy-engine),
Windows games on Wine (MetalSharp Wine, WineHQ) and native macOS apps.

It ships as two products:

| Product | Kind | Used by |
| --- | --- | --- |
| `PearDebugKit` | Swift library | PearSilicon, for every launch log and for Wine process tracking |
| `peardebug` | command-line tool | developers, for reading, analyzing and probing |

## Features

| Module | What it does |
| --- | --- |
| `LogStore` | One log session per launch. Earlier runs are rotated (`<id>.log`, `<id>.1.log`, …) rather than truncated, so the log of a crash survives the relaunch. Large logs are trimmed. |
| `LogParser` | Turns PearRuntime (`[PearRuntime][area]`), PearLegacyEngine (`PearLegacyEngine::module:`), Wine (`0124:err:module:`), Rust panic and CRI middleware lines into structured events. |
| `Analyzer` | Rules per component (PearLegacyEngine, PearRuntime, Wine, common). Each finding has a severity, the log lines it is based on and a suggested fix. Rules are plain Swift values, so a component can add its own. |
| `CrashReports` | Finds and summarizes macOS `.ips` crash reports for a process. |
| `LegacyProbe` | GDB remote client for PearLegacyEngine's `--gdb` server: watches guest functions by symbol and reports arguments, return values and callers. |
| `LegacyCrashCapture` | Runs a 32-bit game to its crash under `--gdb` and reads registers, the frame-pointer call chain and the memory every register points to. |
| `LegacyFrameDump` | Reads the engine's frame dumps (`PEAR_DUMP_FRAME`, `PEAR_PRESENT_DUMP_*`), writes PNGs and reports the render size and how much of the frame is drawn. |
| `WineProcesses` | Lists every process of a Wine prefix, whichever engine started it, by working directory and wineserver folder (Wine has no process list and rewrites process names), and can force-stop them. |
| `SteamDiagnostics` | Explains a Windows Steam client's state from its own logs and the prefix registry. |

## Requirements

- macOS 14 or later
- Swift 6 toolchain (Xcode 16 or later)

## Installation

PearDebug is part of PearRuntime (`debug/`). As a local Swift package
dependency (this is how PearSilicon uses it):

```swift
.package(path: "runtime/debug")
// target dependency:
.product(name: "PearDebugKit", package: "debug")
```

## Command line

```sh
swift build -c release
.build/release/peardebug analyze com.example.game              # newest log of an app
.build/release/peardebug analyze ./some.log --format markdown
.build/release/peardebug logs com.example.game --source legacy --follow
.build/release/peardebug list com.example.game
.build/release/peardebug crashes GameProcess
```

`analyze` exits with status 2 when it finds a problem, so it can gate scripts.

### Probing a 32-bit game

Start the engine with a GDB server and a time to stop at, then watch guest
functions:

```sh
PEAR_BREAK_AFTER=215 touchHLE Game.app --gdb=127.0.0.1:9001 --no-error-popup &
peardebug legacy probe Game.app/Game --port 9001 --watch someFunction,otherFunction --count 20
peardebug legacy symbolize Game.app/Game 0x65798 0x77b67
peardebug legacy crash --port 9001 --binary Game.app/Game
peardebug legacy frames /tmp/frames/f --flip                  # after PEAR_DUMP_FRAME=/tmp/frames/f
```

### Wine and Steam

```sh
peardebug wine ps "~/Library/Application Support/PearSilicon/wine-apps/<Game>"
peardebug steam "<entry>/Prefix/drive_c/Program Files (x86)/Steam"
```

[DEBUGGING.md](DEBUGGING.md) describes every method in detail, with the
problems each one has found.

## Development

```sh
swift build
swift test
```

## The Pear stack

| Project | Role |
| --- | --- |
| [PearSilicon](https://github.com/denizomer10/PearSilicon) | macOS app: library, installer, launcher |
| [PearRuntime](https://github.com/denizomer10/PearRuntime) | compatibility layer for arm64 iOS apps |
| [PearANGLE](../third_party/angle) | OpenGL ES on Metal (ANGLE fork), in PearRuntime |
| [PearLegacyEngine](../third_party/legacy-engine) | 32-bit iOS engine (touchHLE fork), in PearRuntime |
| [PearLegacyJIT](../third_party/legacy-jit) | ARMv7 recompiler (Dynarmic fork), in PearRuntime |
| **PearDebug** | logging and diagnostics |

## License

MIT — see [LICENSE](LICENSE).
