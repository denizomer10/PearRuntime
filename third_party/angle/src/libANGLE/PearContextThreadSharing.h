// Original PearRuntime compatibility contract; BSD-3-Clause, like ANGLE.
// Private to the PearRuntime fork, not a Khronos EGL extension. Opt-in only.
#ifndef LIBANGLE_PEAR_CONTEXT_THREAD_SHARING_H_
#define LIBANGLE_PEAR_CONTEXT_THREAD_SHARING_H_
#include <EGL/egl.h>
namespace gl { class Context; }
namespace egl {
// All GLES entry points must use ANGLE_ENABLE_CONTEXT_MUTEX. The Metal
// backend is the tested target. Unflagged contexts retain standard EGL rules.
constexpr EGLint kPearSerializedThreadSharing = 0x5E01;
bool PearIsThreadShared(const gl::Context *context);
void PearRegisterThreadShared(gl::Context *context, bool enabled);
void PearForgetThreadShared(gl::Context *context);
bool PearAddThreadBinding(gl::Context *context);
bool PearRemoveThreadBinding(gl::Context *context);
}
#endif
