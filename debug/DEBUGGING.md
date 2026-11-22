# Debugging the Pear stack

Methods that found real problems, with the commands to repeat them. Paths
assume the default library at `~/Library/Application Support/PearSilicon`.

## Start with the log

```sh
peardebug analyze <bundle-id or log file>       # newest run; exit status 2 on a problem
peardebug list <bundle-id>                      # every rotated run and crash report
pearsilicon analyze <id>                         # the same from PearSilicon's CLI
```

Each launch writes its own log and keeps the previous runs (`<id>.log`,
`<id>.1.log`, …), so the log of a crash survives a relaunch. Findings name the
lines they are based on and what to try next.

## 32-bit iOS games (PearLegacyEngine)

The engine is touchHLE-based; run it directly to add diagnostics. It must start
in its resource folder (`PearSilicon.app/Contents/Resources/PearLegacyEngine`),
or ANGLE is not found:

```sh
cd /Applications/PearSilicon.app/Contents/Resources/PearLegacyEngine
PEAR_LEGACY_SANDBOX_ROOT=/tmp/sandbox <variables> ./touchHLE /path/Game.app --large-window
```

| Variable | Shows | Found |
|---|---|---|
| `PEAR_TRACE_SELECTORS=a:,b:` | every Objective-C send of those selectors, with the receiver class and first arguments | Miku Flick's view was resized only at the root (`setRootViewController:`), its EAGLView kept the nib's 320x460 |
| `PEAR_DUMP_FRAME=/tmp/f/f` | every 20th renderbuffer (bottom-up) | Miku Flick rendered 640x920 instead of 640x960, cutting off the bottom row and shifting touches |
| `PEAR_PRESENT_DUMP_EVERY=15` + `PEAR_PRESENT_DUMP_PREFIX=/tmp/p/p` | what the window shows (top-down), without a frame limit | screen captures when Screen Recording permission is not available |
| `PEAR_TRACE_OPEN=1` | every `open()` path; opening a folder is always noted | Asphalt 6 opens its `data` folder after an asset-index miss |
| `PEAR_TOUCH_SCRIPT=/tmp/t.txt` | replays `<ms> <x> <y> <down\|up>` touches in device points; the file is re-read while the game runs, so taps can be appended after looking at a frame | driving menus to a song without a person at the window |
| `PEAR_PRESENT_DIAG=1` | window, drawable, EGL surface and viewport sizes | window/viewport mismatches |
| `--gdb=127.0.0.1:9001` | a GDB server; a guest memory error stops the game instead of ending the engine | the crash state below |

```sh
peardebug legacy frames /tmp/f/f --flip         # PNGs plus a size summary ("640x920 ×65")
peardebug legacy crash --port 9001 --binary Game.app/Game
```

`legacy crash` prints the stop reason, registers, the call chain from the frame
pointer (r7) and memory at the stack and at every register that points into
memory. For Asphalt 6 it shows the null read at `0x70ee0` and the empty
structure that pointer came from. Symbols are shown only near an exported
symbol; games strip most functions. The GDB server expects a `+` before the
first packet and hexadecimal thread ids; `peardebug` handles both.

`peardebug legacy probe` watches named guest functions and prints their
arguments, return values and callers (used for the CRI Mana movie end-time bug
in Miku Flick).

## Windows programs (Wine)

```sh
pearsilicon wine-status <id>      # every process of the entry's prefix
pearsilicon wine-stop <id>        # force-quit them all (wineserver -k9, then SIGKILL)
peardebug wine ps <entry folder>  # the same scan without PearSilicon
peardebug analyze <entry>/Data/launch.log --source wine
```

Wine rewrites process names (`C:\windows\system32\services.exe`) and ships no
process list. A prefix's processes are found by working directory (inside the
entry folder) and its wineserver by `/tmp/.wine-<uid>/server-<dev>-<inode>`,
named after the prefix folder. A program counts as running when anything other
than Wine's services (services, winedevice, explorer, plugplay, svchost, rpcss…)
is open; `wineserver -k` alone missed an orphaned `Steam.exe` whose parent had
exited.

Launch with diagnostics (`pearsilicon wine-launch <id> --diagnostics`) to get
`WINEDEBUG=err+all`. Wine logs go to the entry's `Data/launch.log`; MetalSharp
writes exception records (`.flight`) under `Data/Library/Logs/Mullion`.

### Steam

```sh
peardebug steam "<entry>/Prefix/drive_c/Program Files (x86)/Steam"
```

Steam keeps its own logs in `<Steam>/logs` with Windows line endings
(`webhelper.txt` for the browser processes, `steamui_html.txt`,
`steamui_login.txt`, `cef_log.txt`, `bootstrap_log.txt`) and saves its install
folder in the prefix's `user.reg` (`SteamPath`). Problems found so far:

- `FATAL:target_services.cc(121)` / `process_state_.InitCalled()` in
  StorageService: MetalSharp's kernelbase rule for `\Steam\bin\cef\` adds flags
  to every Steam CEF child. Starting Steam through `C:\PearSiliconSteam` (a link
  to its folder) avoids it.
- "Steam install folder is not writable… Repair": a drive letter mapped to the
  Steam folder also avoids the rule, but Steam saves the drive root
  (`SteamPath="s:"`) and rejects it on the next start. In the logs Steam starts
  and shuts down its web helper in the same second while the dialog waits.
- Process liveness is not success: in both failures `Steam.exe` kept running.
  Check for the sign-in window (`DesktopLoginWindow … WasHidden 0`).

## Updates and releases

- "PearBuildCommit disagrees with the resolved release tag": the ZIP was built
  before its commit. `scripts/publish-release.sh` builds the tagged commit and
  checks the uploaded app.
- GitHub allows 60 anonymous API requests per hour
  (`curl -s https://api.github.com/rate_limit`). Engine downloads fall back to
  the public release pages, which name releases by title, not tag; the tag is
  read from the release link.
- A quarantined app runs from a read-only translocated copy
  (`/private/var/folders/…/AppTranslocation/…`); the updater replaces the
  original bundle.
