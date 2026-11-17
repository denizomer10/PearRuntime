// PearANGLE smoke test: EGL display on Metal, ES3 context on a pbuffer,
// clear to a known color, read it back.
//   cc tools/tests/pearangle-smoke.c -I third_party/angle/include \
//      -L build/host/PearANGLE-src -lEGL -lGLESv2 -Wl,-rpath,build/host/PearANGLE-src
#include <EGL/egl.h>
#include <EGL/eglext.h>
#include <GLES3/gl3.h>
#include <stdio.h>

int main(void) {
    EGLint dattrs[] = {EGL_PLATFORM_ANGLE_TYPE_ANGLE, EGL_PLATFORM_ANGLE_TYPE_METAL_ANGLE, EGL_NONE};
    EGLDisplay d = eglGetPlatformDisplay(EGL_PLATFORM_ANGLE_ANGLE, (void *)EGL_DEFAULT_DISPLAY, (const EGLAttrib *)0);
    PFNEGLGETPLATFORMDISPLAYEXTPROC getExt = (PFNEGLGETPLATFORMDISPLAYEXTPROC)eglGetProcAddress("eglGetPlatformDisplayEXT");
    if (getExt) d = getExt(EGL_PLATFORM_ANGLE_ANGLE, (void *)EGL_DEFAULT_DISPLAY, dattrs);
    EGLint major = 0, minor = 0;
    if (!eglInitialize(d, &major, &minor)) { printf("FAIL eglInitialize 0x%x\n", eglGetError()); return 1; }
    EGLint cattrs[] = {EGL_RED_SIZE, 8, EGL_GREEN_SIZE, 8, EGL_BLUE_SIZE, 8, EGL_ALPHA_SIZE, 8,
                       EGL_SURFACE_TYPE, EGL_PBUFFER_BIT, EGL_RENDERABLE_TYPE, EGL_OPENGL_ES3_BIT, EGL_NONE};
    EGLConfig cfg; EGLint n = 0;
    if (!eglChooseConfig(d, cattrs, &cfg, 1, &n) || n < 1) { printf("FAIL eglChooseConfig\n"); return 1; }
    EGLint sattrs[] = {EGL_WIDTH, 64, EGL_HEIGHT, 64, EGL_NONE};
    EGLSurface s = eglCreatePbufferSurface(d, cfg, sattrs);
    EGLint xattrs[] = {EGL_CONTEXT_CLIENT_VERSION, 3, EGL_NONE};
    EGLContext c = eglCreateContext(d, cfg, EGL_NO_CONTEXT, xattrs);
    if (!s || !c || !eglMakeCurrent(d, s, s, c)) { printf("FAIL context 0x%x\n", eglGetError()); return 1; }
    glClearColor(0.25f, 0.5f, 0.75f, 1.0f);
    glClear(GL_COLOR_BUFFER_BIT);
    unsigned char px[4] = {0};
    glReadPixels(32, 32, 1, 1, GL_RGBA, GL_UNSIGNED_BYTE, px);
    printf("EGL %d.%d\nGL_RENDERER: %s\nGL_VERSION: %s\npixel: %u %u %u %u\n", major, minor,
           (const char *)glGetString(GL_RENDERER), (const char *)glGetString(GL_VERSION), px[0], px[1], px[2], px[3]);
    int ok = px[0] >= 62 && px[0] <= 66 && px[1] >= 126 && px[1] <= 130 && px[2] >= 189 && px[2] <= 193 && px[3] == 255;
    printf("%s\n", ok ? "PASS" : "FAIL pixel");
    eglTerminate(d);
    return ok ? 0 : 1;
}
