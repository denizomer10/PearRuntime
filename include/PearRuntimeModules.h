// PearRuntimeModules.h
// PearRuntime is built from independent modules (core, input, graphics,
// device, storage, services). Every module is compiled into the same dylib,
// but each one can be switched off per launch without rebuilding:
//
//   PEAR_RUNTIME_DISABLE=services,graphics   # comma-separated module names
//   PEAR_RUNTIME_ONLY=core,input              # allow-list (core is implied)
//
// `core` (bootstrap, logging, host service) can never be disabled.
#pragma once

#ifdef __cplusplus
extern "C" {
#endif

/// YES when `name` is enabled for this process. The environment is parsed once;
/// the call is cheap enough for initializers and hot paths.
int PearRuntimeModuleEnabled(const char *name);

/// Comma-separated list of the enabled modules, for logs and diagnostics.
const char *PearRuntimeEnabledModules(void);

/// Version of the PearRuntime build (see VERSION).
const char *PearRuntimeVersion(void);

#ifdef __cplusplus
}
#endif

// The `performance` module (Apple silicon scheduling, Game Mode) follows the
// same rules.
// `graphics` owns shared screen/window geometry. Its backend children are
// `graphics-gl` and `graphics-metal`: selecting graphics includes both;
// disabling graphics disables both. Selecting a child includes the shared
// geometry but not the other backend. Explicit child disables take priority.
//
// Each source file is compiled with -DPEAR_RUNTIME_MODULE="<module>". An
// initializer opts into module gating with this guard as its first statement.
#ifndef PEAR_RUNTIME_MODULE
#define PEAR_RUNTIME_MODULE "core"
#endif
#define PEAR_MODULE_GUARD() \
    do { if (!PearRuntimeModuleEnabled(PEAR_RUNTIME_MODULE)) return; } while (0)
