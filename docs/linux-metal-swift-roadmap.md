# PearRuntime Linux ARM64 Yol Haritası: Modern 64-bit Metal + Swift iOS Oyunları

Bu belge, PearRuntime'ın mevcut Linux ARM64 altyapısı ([pear-dyld](../rust/crates/pear-dyld/src/lib.rs), [pear-libobjc](../rust/crates/pear-libobjc/src/lib.rs), [pear-foundation](../rust/crates/pear-foundation/src/lib.rs), [pear-host-linux](../rust/crates/pear-host-linux/src/lib.rs), [pear-runtime](../rust/crates/pear-runtime/src/lib.rs)) üzerinde **Metal + Swift** kullanan modern 64-bit iOS oyunlarını (ör. **Genshin Impact v7.2.0**) çalıştırmak için gereken mimari katmanları ve aşamalı yol haritasını tanımlar.

---

## 1. Hedef Mimari

Bugün `pear-runtime` Linux ARM64 üzerinde **OpenGL ES 1/2 + C++/ObjC** oyunlarını (ör. Burger Shop 1) X11/EGL ile çalıştırabiliyor. Modern 64-bit iOS yapımları için 5 ana dikey katman gerekir:

```
┌─────────────────────────────────────────────────────────────────────────┐
│             64-bit ARM64 iOS Uygulaması (.app + @rpath/*.framework)      │
├─────────────────────────────────────────────────────────────────────────┤
│ 1. Çoklu İmaj Yükleyici (pear-dyld)                                     │
│    LC_RPATH / @rpath / @executable_path / @loader_path, Weak/Reexport, │
│    TLV, __unwind_info / __eh_frame kaydı                                │
├─────────────────────────────────────────────────────────────────────────┤
│ 2. Darwin C / C++ / ObjC / Swift ABI Katmanı                            │
│    libSystem/Mach/XNU + libdispatch | Apple libc++ (std::__1) |         │
│    pear-libobjc (Class Realization) | pear-swift-rt (ObjC Interop)      │
├─────────────────────────────────────────────────────────────────────────┤
│ 3. Grafik & Shader Hattı (pear-metal-vk)                                │
│    Metal ObjC API → Vulkan 1.2/1.3 (X11/Wayland WSI)                    │
│    AIR / MTLB Bitcode + MSL → SPIR-V Derleyicisi & Disk Önbelleği       │
├─────────────────────────────────────────────────────────────────────────┤
│ 4. Uygulama Çerçeveleri                                                 │
│    Taşınabilir UIKit (AutoLayout, UIButton, UITextField, metin çizimi)  │
│    Security (TLS/RSA/Keychain → rustls/ring) + CFNetwork + AudioToolbox │
├─────────────────────────────────────────────────────────────────────────┤
│ 5. Linux Host (pear-host-linux)                                         │
│    X11 / Wayland + Vulkan GPU (Host / Lavapipe / Venus) + PipeWire/ALSA │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Ölçülen Referans Tablosu (Genshin Impact v7.2.0)

[com.miHoYo.GenshinImpact-7.2.0.json](census/com.miHoYo.GenshinImpact-7.2.0.json) sayımı ve `pearruntime --list-missing` başlangıç ölçümüne göre:
- **Toplam Kütüphane / Import:** 44 kütüphane, 2.828 import sembolü (`pearruntime` ilk ölçümde 2.379 eksik sembol, 236 gerçekleşmemiş sınıf).
- **Metal Shader (`MTLB`):** 2.069 derlenmiş Metal shader kütüphanesi (`globalgamemanagers.assets`: 1.403, `resources.assets`: 666), `Data/` altında 0 adet GLSL oyun shader'ı.
- **Swift ABI + Overlay İthalatları:** 1.264 sembol (`stdlib`: 688, `Foundation`: 371, `_swift_*` runtime: 109, `Dispatch`: 63, `ObjectiveC`: 21, `CoreGraphics`: 12).
- **C++ & Sistem:** `libc++` (`std::__1`): 276, `libSystem`/Mach/XNU: 262, `CoreFoundation`/`Foundation`: 213, `Security`: 138, `UIKit`: 102, `CoreGraphics`: 94, `CFNetwork`: 53, `AudioToolbox`: 40, `Accelerate`: 13.
- **Gömülü Framework'ler:** `@rpath/AstrolabeCommon.framework/AstrolabeCommon` ve `@rpath/Telemetry.framework/Telemetry` (`LC_RPATH`: `/usr/lib/swift`, `@executable_path/Frameworks`).

---

## 3. Aşamalı Uygulama Fazları ve Doğrulama Kapıları

### Faz 1 — Çoklu İmaj `pear-dyld`, `libc++` ve Darwin Çekirdek ABI
1. `pear-macho` içinde `LC_RPATH` ayrıştırması; `pear-dyld` içinde `@rpath` / `@executable_path` / `@loader_path` bağımlılık grafiği yükleyici (`ImageSet`, topolojik başlatma sırası, çapraz imaj sembol çözümleme).
2. `pear-runtime` içinde tam çoklu-imaj `_dyld_*` kayıt defteri (`_dyld_image_count`, `_dyld_get_image_header`, `_dyld_get_image_vmaddr_slide`, `_dyld_get_image_name`) ve misafir `dlsym` araması.
3. LLVM `libc++.so.1` + `libc++abi.so.1` (`std::__1`) ve `__unwind_info` / `__eh_frame` entegrasyonu; Darwin `os_unfair_lock`, `mach_vm_*`, `libdispatch` tamamlaması.
- **Kapı 1:** Çoklu `@rpath` framework'lü C++17 + `libdispatch` ARM64 iOS test ikilisi ve gömülü framework yüklemesi Linux/host `pearruntime` altında hatasız yüklenir.

### Faz 2 — Grafik ve Shader Motoru (`pear-metal-vk` + AIR/MTLB → SPIR-V)
1. `pear-metal-vk`: `MTLCreateSystemDefaultDevice`, `CAMetalLayer` → `VkSwapchainKHR`, `MTLCommandQueue`/`MTLCommandBuffer`/encoder'lar, `MTLBuffer`/`MTLTexture`/ASTC yedeklemesi, Y-ekseni tersliği (`VK_KHR_maintenance1`).
2. `MTLB` kapsayıcısı ve Apple IR (AIR) bitcode → SPIR-V derleyici hattı + `naga` (MSL → SPIR-V) + `~/.cache/pearruntime/shaders/` önbelleği.
- **Kapı 2:** Mesa `lavapipe` (yazılımsal Vulkan) ve donanım Vulkan sürücüleri üzerinde önce MSL test sahnesi, ardından `.metallib` (`MTLB`) kullanan Unity Metal test sahnesi render edilir.

### Faz 3 — Apple Swift ABI ve ObjC-Interop (`pear-swift-rt`)
1. `apple/swift` (**Apache-2.0**) runtime ve `Foundation`/`Dispatch` overlay'lerinin `SWIFT_OBJC_INTEROP=1` ile `pear-libobjc` ABI'sine karşı derlenmesi (veya çekirdek `_swift_*` katmanının aşamalı tamamlanması).
2. `pear-libobjc` içinde dinamik Swift/ObjC sınıf realizasyonu ve `NSObject` ↔ `SwiftObject` köprüsü (Genshin'deki 236 unrealized sınıfın çözülmesi).
- **Kapı 3:** `@objc` + Swift `Foundation` kullanan karma ARM64 iOS uygulaması `pearruntime` altında sıfır eksik Swift sembolü ile çalışır.

### Faz 4 — Ortak Taşınabilir `UIKit` + `Security` / `CFNetwork` / `Audio`
1. `pear-uikit` içindeki AutoLayout çözücüsü, `UIButton`, `UITextField`, `UILabel`, `UIImageView`, `UIWindow` ve dokunma/klavye mantığının platform-bağımsız çekirdeğe (`pear-libobjc` + `pear-host`) taşınması; Linux için `tiny-skia` + `cosmic-text` çizimi.
2. `Security` (`rustls`/`ring` + `pear-keychain`), `CFNetwork`/`NSURLSession` HTTP/2 istemcisi, `AudioToolbox` → PipeWire/ALSA, `Accelerate` → Rust/NEON.
- **Kapı 4:** Linux'ta AutoLayout'lu giriş kartı + HTTPS isteği + Keychain + klavye/fare etkileşimi uçtan uca çalışır.

### Faz 5 — Genshin Impact Uçtan Uca Entegrasyon
1. `pearruntime GenshinImpact.app --list-missing` çıktısının kritik eksiklerden arındırılması.
2. Unity `il2cpp` iş parçacığı/sinyal yakalayıcılarının Linux bağlamında güvenli hale getirilmesi ve Celestia 3D sahnesi + HoYoverse giriş ekranının Linux Vulkan penceresinde doğrulanması.

---

## 4. Lisans ve Temiz Oda Uyumu ([independent-runtime.md](independent-runtime.md))

- **MIT:** `pear-dyld`, `pear-libobjc`, `pear-metal-vk`, `pear-uikit`, `pear-runtime` (temiz-oda Rust).
- **Apache-2.0 (LLVM Exception / Swift):** `apple/swift`, `swift-corelibs-foundation`, `swift-corelibs-libdispatch`, LLVM `libc++` (`NOTICE` korunur).
- **MIT / Apache-2.0:** `ash`, `naga`, `rustls`, `ring`, `tiny-skia`, `cosmic-text`.
- **APSL 2.0 (`xnu`, `dyld`, `objc4`):** Yalnızca temiz-oda referans olarak kullanılır; kodu doğrudan alınırsa `third_party/` altında ayrı tutulur.
