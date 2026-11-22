#!/usr/bin/env python3
"""ABI census of a PearHost framework build (macOS platform), as JSON.

  tools/host-abi-census.py build/host > census.json
  tools/host-abi-census.py build/host --compare before.json

Loads UIKit, OpenGLES and MetalKit from <host build dir> into this (macOS,
arm64) Python process, which runs their image initializers, then records:

- exported symbols (nm -gU) per framework;
- the value of every exported NSString/numeric constant;
- every Objective-C class the image defines: superclass, instance size,
  ivars, properties, and instance/class methods with type encodings. Each
  method also records where its implementation lives (Objective-C/C source
  symbol such as "-[UIView frame]", or Rust), so a port can show that
  behaviour moved rather than vanished.

--compare prints the differences against an earlier census (ignoring the
implementation origins, which a port changes by design) and exits 1 on any.
"""
import argparse
import ctypes
import ctypes.util
import json
import os
import re
import subprocess
import sys

FRAMEWORKS = ("UIKit", "OpenGLES", "MetalKit")


def runtime():
    objc = ctypes.CDLL(ctypes.util.find_library("objc"))
    for name, res, args in (
        ("objc_copyClassNamesForImage", ctypes.POINTER(ctypes.c_char_p), [ctypes.c_char_p, ctypes.POINTER(ctypes.c_uint)]),
        ("objc_getClass", ctypes.c_void_p, [ctypes.c_char_p]),
        ("class_getSuperclass", ctypes.c_void_p, [ctypes.c_void_p]),
        ("class_getName", ctypes.c_char_p, [ctypes.c_void_p]),
        ("class_getInstanceSize", ctypes.c_size_t, [ctypes.c_void_p]),
        ("class_copyIvarList", ctypes.POINTER(ctypes.c_void_p), [ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint)]),
        ("ivar_getName", ctypes.c_char_p, [ctypes.c_void_p]),
        ("ivar_getTypeEncoding", ctypes.c_char_p, [ctypes.c_void_p]),
        ("ivar_getOffset", ctypes.c_long, [ctypes.c_void_p]),
        ("class_copyPropertyList", ctypes.POINTER(ctypes.c_void_p), [ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint)]),
        ("property_getName", ctypes.c_char_p, [ctypes.c_void_p]),
        ("property_getAttributes", ctypes.c_char_p, [ctypes.c_void_p]),
        ("class_copyMethodList", ctypes.POINTER(ctypes.c_void_p), [ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint)]),
        ("method_getName", ctypes.c_void_p, [ctypes.c_void_p]),
        ("method_getTypeEncoding", ctypes.c_char_p, [ctypes.c_void_p]),
        ("method_getImplementation", ctypes.c_void_p, [ctypes.c_void_p]),
        ("sel_getName", ctypes.c_char_p, [ctypes.c_void_p]),
        ("object_getClass", ctypes.c_void_p, [ctypes.c_void_p]),
        ("class_getIvarLayout", ctypes.c_char_p, [ctypes.c_void_p]),
        ("class_getWeakIvarLayout", ctypes.c_char_p, [ctypes.c_void_p]),
    ):
        f = getattr(objc, name)
        f.restype, f.argtypes = res, args
    return objc


class DlInfo(ctypes.Structure):
    _fields_ = [("dli_fname", ctypes.c_char_p), ("dli_fbase", ctypes.c_void_p),
                ("dli_sname", ctypes.c_char_p), ("dli_saddr", ctypes.c_void_p)]


def origin(libc, imp):
    info = DlInfo()
    if not imp or not libc.dladdr(ctypes.c_void_p(imp), ctypes.byref(info)):
        return None
    name = (info.dli_sname or b"").decode()
    image = os.path.basename((info.dli_fname or b"").decode())
    if name.startswith(("-[", "+[")):
        return f"{image}:objc"
    if name.startswith("_R") or "pear_" in name:
        return f"{image}:rust"
    return f"{image}:{name or '?'}"


def methods(objc, libc, cls):
    count = ctypes.c_uint()
    listed = objc.class_copyMethodList(cls, ctypes.byref(count))
    result = {}
    for i in range(count.value):
        m = listed[i]
        sel = objc.sel_getName(objc.method_getName(m)).decode()
        result[sel] = {"types": (objc.method_getTypeEncoding(m) or b"").decode(),
                       "impl": origin(libc, objc.method_getImplementation(m))}
    return dict(sorted(result.items()))


def classes(objc, libc, image, handle):
    count = ctypes.c_uint()
    names = objc.objc_copyClassNamesForImage(image.encode(), ctypes.byref(count))
    libc.dlsym.restype, libc.dlsym.argtypes = ctypes.c_void_p, [ctypes.c_void_p, ctypes.c_char_p]
    result = {}
    for i in range(count.value):
        name = names[i]
        # This image's own class object: a system class may share the name
        # (UIFoundation's UIFont), and objc_getClass returns only one of them.
        cls = libc.dlsym(handle, b"OBJC_CLASS_$_" + name) or objc.objc_getClass(name)
        sup = objc.class_getSuperclass(cls)
        ivars = []
        n = ctypes.c_uint()
        listed = objc.class_copyIvarList(cls, ctypes.byref(n))
        for j in range(n.value):
            ivars.append([objc.ivar_getName(listed[j]).decode(), (objc.ivar_getTypeEncoding(listed[j]) or b"").decode(),
                          objc.ivar_getOffset(listed[j])])
        props = {}
        listed = objc.class_copyPropertyList(cls, ctypes.byref(n))
        for j in range(n.value):
            props[objc.property_getName(listed[j]).decode()] = objc.property_getAttributes(listed[j]).decode()
        layout = objc.class_getIvarLayout(cls)
        weak = objc.class_getWeakIvarLayout(cls)
        result[name.decode()] = {
            "superclass": objc.class_getName(sup).decode() if sup else None,
            "instanceSize": objc.class_getInstanceSize(cls),
            "ivars": ivars,
            "ivarLayout": layout.hex() if layout else None,
            "weakIvarLayout": weak.hex() if weak else None,
            "properties": dict(sorted(props.items())),
            "instanceMethods": methods(objc, libc, cls),
            "classMethods": methods(objc, libc, objc.object_getClass(cls)),
        }
    return dict(sorted(result.items()))


def exports(path):
    """{symbol: nm type letter} of the image's exported definitions."""
    out = subprocess.check_output(["nm", "-gU", path], text=True)
    result = {}
    for line in out.splitlines():
        parts = line.split()
        if len(parts) >= 3:
            result[parts[-1]] = parts[-2]
    return result


def constants(path, symbols):
    """Values of exported data symbols: CFString contents, else 8 raw bytes."""
    lib = ctypes.CDLL(path)
    cf = ctypes.CDLL("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")
    cf.CFGetTypeID.restype, cf.CFGetTypeID.argtypes = ctypes.c_ulong, [ctypes.c_void_p]
    cf.CFStringGetTypeID.restype = ctypes.c_ulong
    cf.CFStringGetCString.restype = ctypes.c_bool
    cf.CFStringGetCString.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_long, ctypes.c_uint32]
    result = {}
    for sym, kind in symbols.items():
        # Public C data names only: not ObjC metadata, nor Rust/C++ internals
        # (thread-locals among them cannot be read as plain data).
        if kind not in ("S", "D") or not re.fullmatch(r"_[a-zA-Z][A-Za-z0-9_]*", sym) or sym.startswith(("_OBJC_", "_R")):
            continue
        cell = ctypes.c_uint64.in_dll(lib, sym[1:])
        value = cell.value
        text = None
        if value and value % 8 == 0 and kind == "S":
            info = DlInfo()
            libc = ctypes.CDLL(None)
            libc.dladdr.argtypes = [ctypes.c_void_p, ctypes.POINTER(DlInfo)]
            # A pointer back into one of our images may be a constant CFString.
            if libc.dladdr(ctypes.c_void_p(value), ctypes.byref(info)) and info.dli_fname and \
                    os.path.basename(info.dli_fname.decode()) in FRAMEWORKS:
                if cf.CFGetTypeID(value) == cf.CFStringGetTypeID():
                    buffer = ctypes.create_string_buffer(4096)
                    if cf.CFStringGetCString(value, buffer, 4096, 0x08000100):
                        text = "@" + buffer.value.decode()
        result[sym] = text if text is not None else f"0x{value:016x}"
    return result


def census(build):
    libc = ctypes.CDLL(None)
    libc.dladdr.argtypes = [ctypes.c_void_p, ctypes.POINTER(DlInfo)]
    objc = runtime()
    result = {}
    for fw in FRAMEWORKS:
        path = os.path.join(build, f"{fw}.framework", fw)
        handle = ctypes.CDLL(path, mode=ctypes.RTLD_GLOBAL)._handle
        symbols = exports(path)
        result[fw] = {"exports": sorted(symbols), "constants": constants(path, symbols),
                      "classes": classes(objc, libc, os.path.realpath(path), handle)}
    return result


def strip_origins(value):
    if isinstance(value, dict):
        return {k: strip_origins(v) for k, v in value.items() if k != "impl"}
    if isinstance(value, list):
        return [strip_origins(v) for v in value]
    return value


def compare(before, after, path=""):
    diffs = []
    if isinstance(before, dict) and isinstance(after, dict):
        for key in sorted(set(before) | set(after)):
            if key not in after:
                diffs.append(f"- {path}/{key}")
            elif key not in before:
                diffs.append(f"+ {path}/{key}")
            else:
                diffs += compare(before[key], after[key], f"{path}/{key}")
    elif isinstance(before, list) and isinstance(after, list) and all(isinstance(x, str) for x in before + after):
        for item in sorted(set(before) - set(after)):
            diffs.append(f"- {path}: {item}")
        for item in sorted(set(after) - set(before)):
            diffs.append(f"+ {path}: {item}")
    elif before != after:
        diffs.append(f"~ {path}: {before!r} -> {after!r}")
    return diffs


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("build", help="host build directory (contains UIKit.framework)")
    parser.add_argument("--compare", help="earlier census JSON to diff against")
    args = parser.parse_args()
    current = census(os.path.abspath(args.build))
    if not args.compare:
        json.dump(current, sys.stdout, indent=1, sort_keys=True)
        print()
        return
    with open(args.compare) as f:
        before = json.load(f)
    diffs = compare(strip_origins(before), strip_origins(current))
    print("\n".join(diffs) if diffs else "census identical (implementation origins aside)")
    sys.exit(1 if diffs else 0)


if __name__ == "__main__":
    main()
