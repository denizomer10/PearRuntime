# PearRuntime serialized context bindings

This fork adds a private EGL context attribute, `0x5E01`, with a boolean value.
It is a PearRuntime compatibility contract, not an advertised Khronos extension.
It is disabled by default, and accepted only in builds enabling both
`ANGLE_ENABLE_CONTEXT_MUTEX` and the Metal backend. PearRuntime selects Metal.

Flagged contexts may be current on several CPU threads. Existing ANGLE
ContextMutex locks serialize GLES commands and EGL binding transitions. Every
thread holds a real context reference and real EGL/TLS binding. Extra bindings
do not reset GL state or reattach an already-current surface. Only the last
binding un-makes the context, releases its surface and permits destruction.
The registry is removed when the context is finally destroyed. Applications
still need to order multi-call transactions; a lock serializes individual
commands, not an unsynchronized application's intended command order.

Unflagged contexts retain the ordinary EGL single-thread validation and
lifecycle. PearRuntime's low-level context initializer stays unflagged; its
EAGL game adapter explicitly opts in. There is no system EAGL implementation
and no successful binding over `EGL_NO_CONTEXT`.

Workspace validation: `scripts/test-angle-backend.sh` covers ordinary exclusive
ownership, genuine cross-thread GL state/readback, two concurrent peers
allocating 512 unique renderbuffer names, main-thread presentation after peer
unbind, context/surface resize, and thread-exit cleanup. Tests call public EGL
and GLES APIs and contain original synthetic scenes, not game implementation
or operating-system source.
