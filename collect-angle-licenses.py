#!/usr/bin/env python3
"""Collects the license texts of the third-party code inside an ANGLE build.

ANGLE's own LICENSE covers ANGLE only. A non-component ANGLE build also links
abseil, the Chromium copy of zlib, xxHash, astc-encoder, Perfetto and others
straight into libEGL/libGLESv2, and a component build ships some of them as
separate dylibs. Their licenses (Apache-2.0, Zlib, BSD-2/3-Clause, MIT) all
require that the license text travels with the binary, so this script writes
one text file holding every one of them.

Components are found in two ways:

  --ninja-dir/--target  read the link inputs of the given targets from the
                        build's ninja files, so a new dependency is noticed
                        automatically (an unknown input is an error, not a
                        silent omission);
  --component NAME      add a component by name (for prebuilt dylibs whose
                        build directory is not available).

Usage:
  collect-angle-licenses.py --angle-root DIR --out FILE
      [--ninja-dir OUT_DIR --target libGLESv2.dylib ...]
      [--component NAME ...] [--license NAME=PATH ...]
"""

import argparse
import pathlib
import re
import subprocess
import sys

# name -> (directory in the ANGLE checkout, fallback license file, SPDX)
# The directory's README.chromium is preferred when it names a license file.
COMPONENTS = {
    "abseil-cpp": ("third_party/abseil-cpp", "LICENSE", "Apache-2.0"),
    "astc-encoder": ("third_party/astc-encoder", "src/LICENSE.txt", "Apache-2.0"),
    "perfetto": ("third_party/perfetto", "LICENSE", "Apache-2.0"),
    "zlib": ("third_party/zlib", "LICENSE", "Zlib"),
    "xxhash": ("src/common/third_party/xxhash", "src/LICENSE", "BSD-2-Clause"),
    "volk": ("src/third_party/volk", "src/LICENSE.md", "MIT"),
    "spirv-headers": ("third_party/spirv-headers", "LICENSE", "Apache-2.0"),
    "spirv-tools": ("third_party/spirv-tools", "src/LICENSE", "Apache-2.0"),
    "glslang": ("third_party/glslang", "src/LICENSE.txt", "BSD-3-Clause AND others"),
    "vulkan-headers": ("third_party/vulkan-headers", "LICENSE.txt", "Apache-2.0"),
    "vulkan_memory_allocator": ("third_party/vulkan_memory_allocator", "LICENSE.txt", "MIT"),
    "libc++": ("third_party/libc++", "src/LICENSE.TXT", "Apache-2.0 WITH LLVM-exception"),
    "libc++abi": ("third_party/libc++abi", "src/LICENSE.TXT", "Apache-2.0 WITH LLVM-exception"),
    "dawn": ("third_party/dawn", "LICENSE", "BSD-3-Clause"),
}

# Link inputs that are ANGLE itself or toolchain runtime objects. compiler-rt
# is Apache-2.0 WITH LLVM-exception, whose exception waives the notice
# requirement for object code produced by the compiler.
ANGLE_OWN = re.compile(
    r"^obj/(src/(?!third_party/)|lib[A-Za-z_]+|angle_[A-Za-z_]+|libANGLE|"
    r"libGLESv2|libEGL|libEGL_egl_loader|common|preprocessor|translator)"
)
TOOLCHAIN = re.compile(r"/llvm-build/.*libclang_rt")
INPUT_MAP = [
    (re.compile(r"^obj/xxhash/"), "xxhash"),
    (re.compile(r"^obj/src/third_party/([^/]+)/"), None),
    (re.compile(r"^obj/third_party/([^/]+)/"), None),
]


def components_from_ninja(ninja_dir: pathlib.Path, targets):
    found = set()
    ninja_files = list(ninja_dir.glob("*.ninja")) + list(ninja_dir.glob("obj/**/*.ninja"))
    for target in targets:
        rule = re.compile(r"^build \./" + re.escape(target) + r"\b")
        inputs = None
        for path in ninja_files:
            lines = path.read_text(errors="replace").splitlines()
            for line in lines:
                if rule.match(line):
                    inputs = line
                    break
            if inputs:
                break
        if inputs is None:
            sys.exit(f"error: no link rule for {target} in {ninja_dir}")
        for token in inputs.split():
            if not token.endswith((".o", ".a")):
                continue
            if TOOLCHAIN.search(token):
                continue
            name = None
            for pattern, fixed in INPUT_MAP:
                match = pattern.match(token)
                if match:
                    name = fixed or match.group(1)
                    break
            if name is None:
                if ANGLE_OWN.match(token):
                    continue
                sys.exit(f"error: {target} links {token}, which is not a known ANGLE or "
                         f"third-party input; add it to collect-angle-licenses.py")
            found.add(name)
    return found


def license_file(root: pathlib.Path, name: str, overrides):
    if name in overrides:
        return overrides[name], COMPONENTS.get(name, (None, None, "see text"))[2]
    if name not in COMPONENTS:
        sys.exit(f"error: unknown component {name}; add it to COMPONENTS")
    directory, fallback, spdx = COMPONENTS[name]
    base = root / directory
    readme = base / "README.chromium"
    if readme.is_file():
        text = readme.read_text(errors="replace")
        match = re.search(r"^License File:\s*(\S+)", text, re.M)
        if match and (base / match.group(1)).is_file():
            fallback = match.group(1)
        match = re.search(r"^License:\s*(.+)$", text, re.M)
        if match:
            spdx = match.group(1).strip()
    path = base / fallback
    if not path.is_file():
        sys.exit(f"error: license file for {name} not found at {path}; "
                 f"check out the dependency or pass --license {name}=PATH")
    return path, spdx


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--angle-root", required=True, type=pathlib.Path)
    parser.add_argument("--out", required=True, type=pathlib.Path)
    parser.add_argument("--ninja-dir", type=pathlib.Path)
    parser.add_argument("--target", action="append", default=[])
    parser.add_argument("--component", action="append", default=[])
    parser.add_argument("--license", action="append", default=[], metavar="NAME=PATH")
    parser.add_argument("--title", default="ANGLE")
    args = parser.parse_args()

    overrides = {}
    for item in args.license:
        name, _, path = item.partition("=")
        if not path or not pathlib.Path(path).is_file():
            sys.exit(f"error: --license {item}: file not found")
        overrides[name] = pathlib.Path(path)

    names = set(args.component)
    if args.target:
        if not args.ninja_dir:
            sys.exit("error: --target needs --ninja-dir")
        names |= components_from_ninja(args.ninja_dir, args.target)

    try:
        commit = subprocess.run(["git", "-C", str(args.angle_root), "rev-parse", "HEAD"],
                                capture_output=True, text=True, check=True).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        commit = "unknown"

    out = [
        f"Third-party software in {args.title}",
        "=" * (24 + len(args.title)),
        "",
        "ANGLE itself is covered by LICENSE.ANGLE (BSD-3-Clause). The ANGLE",
        "binaries next to this file also contain, or depend on, the components",
        "below. Each is copyright its respective authors and is redistributed",
        "under the license reproduced here. Texts were taken from the ANGLE",
        f"checkout at commit {commit} (https://github.com/denizomer10/PearANGLE).",
        "",
    ]
    for name in sorted(names):
        path, spdx = license_file(args.angle_root, name, overrides)
        out += ["-" * 78, f"{name} ({spdx})", "-" * 78, "",
                path.read_text(errors="replace").rstrip(), ""]
        notice = path.parent / "NOTICE"
        if notice.is_file():
            out += [f"NOTICE for {name}:", "", notice.read_text(errors="replace").rstrip(), ""]

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("\n".join(out) + "\n")
    print(f"{args.out}: {', '.join(sorted(names)) or 'no third-party components'}")


if __name__ == "__main__":
    main()
