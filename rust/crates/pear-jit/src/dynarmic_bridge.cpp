// Clean-room C-ABI bridge for PearLegacyJIT (dynarmic A32 & A64 -> x86_64/arm64).
// Compiled when linking `third_party/legacy-jit` with `-DPEAR_ENABLE_DYNARMIC=1`.

#include <cstdint>
#include <cstring>
#include "dynarmic/interface/A32/a32.h"
#include "dynarmic/interface/A32/config.h"
#include "dynarmic/interface/A64/a64.h"
#include "dynarmic/interface/A64/config.h"
#include "dynarmic/interface/optimization_flags.h"

extern "C" {

typedef struct PearJitMemCallbacks {
    void* ctx;
    uint8_t (*read8)(void* ctx, uint64_t addr);
    uint16_t (*read16)(void* ctx, uint64_t addr);
    uint32_t (*read32)(void* ctx, uint64_t addr);
    uint64_t (*read64)(void* ctx, uint64_t addr);
    void (*write8)(void* ctx, uint64_t addr, uint8_t val);
    void (*write16)(void* ctx, uint64_t addr, uint16_t val);
    void (*write32)(void* ctx, uint64_t addr, uint32_t val);
    void (*write64)(void* ctx, uint64_t addr, uint64_t val);
    void (*call_svc)(void* ctx, uint32_t swi);
} PearJitMemCallbacks;

namespace {

class A64UserEnv final : public Dynarmic::A64::UserCallbacks {
public:
    explicit A64UserEnv(PearJitMemCallbacks cb) : cb_(cb) {}
    uint8_t MemoryRead8(Dynarmic::A64::VAddr vaddr) override { return cb_.read8(cb_.ctx, vaddr); }
    uint16_t MemoryRead16(Dynarmic::A64::VAddr vaddr) override { return cb_.read16(cb_.ctx, vaddr); }
    uint32_t MemoryRead32(Dynarmic::A64::VAddr vaddr) override { return cb_.read32(cb_.ctx, vaddr); }
    uint64_t MemoryRead64(Dynarmic::A64::VAddr vaddr) override { return cb_.read64(cb_.ctx, vaddr); }
    Dynarmic::A64::Vector MemoryRead128(Dynarmic::A64::VAddr vaddr) override {
        return {cb_.read64(cb_.ctx, vaddr), cb_.read64(cb_.ctx, vaddr + 8)};
    }
    void MemoryWrite8(Dynarmic::A64::VAddr vaddr, uint8_t value) override { cb_.write8(cb_.ctx, vaddr, value); }
    void MemoryWrite16(Dynarmic::A64::VAddr vaddr, uint16_t value) override { cb_.write16(cb_.ctx, vaddr, value); }
    void MemoryWrite32(Dynarmic::A64::VAddr vaddr, uint32_t value) override { cb_.write32(cb_.ctx, vaddr, value); }
    void MemoryWrite64(Dynarmic::A64::VAddr vaddr, uint64_t value) override { cb_.write64(cb_.ctx, vaddr, value); }
    void MemoryWrite128(Dynarmic::A64::VAddr vaddr, Dynarmic::A64::Vector value) override {
        cb_.write64(cb_.ctx, vaddr, value[0]);
        cb_.write64(cb_.ctx, vaddr + 8, value[1]);
    }
    void CallSVC(uint32_t swi) override {
        if (cb_.call_svc) cb_.call_svc(cb_.ctx, swi);
    }
    void ExceptionRaised(Dynarmic::A64::VAddr, Dynarmic::A64::Exception) override {}
    void InterpreterFallback(Dynarmic::A64::VAddr, size_t) override {}
    void AddTicks(uint64_t) override {}
    uint64_t GetTicksRemaining() override { return 1000000; }
private:
    PearJitMemCallbacks cb_;
};

struct PearA64JitHandle {
    A64UserEnv env;
    Dynarmic::A64::Jit jit;
    PearA64JitHandle(PearJitMemCallbacks cb, uint32_t opt_flags)
        : env(cb), jit(make_config(env, opt_flags)) {}
    static Dynarmic::A64::UserConfig make_config(A64UserEnv& env, uint32_t opt_flags) {
        Dynarmic::A64::UserConfig cfg{};
        cfg.callbacks = &env;
        cfg.optimizations = static_cast<Dynarmic::OptimizationFlag>(opt_flags);
        return cfg;
    }
};

} // namespace

void* pear_jit_a64_create(PearJitMemCallbacks cb, uint32_t opt_flags) {
    return new PearA64JitHandle(cb, opt_flags);
}

void pear_jit_a64_destroy(void* handle) {
    delete static_cast<PearA64JitHandle*>(handle);
}

uint32_t pear_jit_a64_run(void* handle) {
    return static_cast<uint32_t>(static_cast<PearA64JitHandle*>(handle)->jit.Run());
}

void pear_jit_a64_set_pc(void* handle, uint64_t pc) {
    static_cast<PearA64JitHandle*>(handle)->jit.SetPC(pc);
}

uint64_t pear_jit_a64_get_pc(void* handle) {
    return static_cast<PearA64JitHandle*>(handle)->jit.GetPC();
}

} // extern "C"
