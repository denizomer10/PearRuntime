# PearRuntime Linux ARM64 Yol Haritası: Modern 64-bit Metal + Swift iOS Oyunları

**Son güncelleme:** 2026-11-27 (Faz 5.19 sonrası; HEAD: `main`)

Bu belge, PearRuntime'ın saf-Rust ve taşınabilir çalışma zamanı yığını ([`pear-dyld`](../rust/crates/pear-dyld), [`pear-libobjc`](../rust/crates/pear-libobjc), [`pear-swift-rt`](../rust/crates/pear-swift-rt), [`pear-foundation`](../rust/crates/pear-foundation), [`pear-metal-vk`](../rust/crates/pear-metal-vk), [`pear-host-linux`](../rust/crates/pear-host-linux), [`pear-runtime`](../rust/crates/pear-runtime)) üzerinde **Metal + Swift** kullanan modern 64-bit iOS oyunlarını (birincil doğrulama hedefi: **Genshin Impact v7.2.0**) çalıştırmak için kurulan mimari katmanları ve aşamalı ilerleme durumunu özetler.

---

## 1. Uygulanan 5 Katmanlı Mimari

`pearruntime` CLI ve `pear-runtime` yığını bugün macOS ve Linux ARM64 üzerinde aynı temiz-oda Rust çekirdeğiyle çalışır:

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│          64-bit ARM64 iOS Uygulaması (.app + @rpath/*.framework)             │
├──────────────────────────────────────────────────────────────────────────────┤
│ 1. Çoklu İmaj Yükleyici (pear-macho + pear-dyld)                             │
│    • LC_RPATH / @rpath / @executable_path / @loader_path bağımlılık grafiği  │
│    • Chained / dyld-info fixups, re-export sembol çözümleme, topolojik init  │
│    • Per-thread Darwin TLV (tlv_get_addr) ve __unwind_info / __eh_frame      │
├──────────────────────────────────────────────────────────────────────────────┤
│ 2. Darwin C / C++ / ObjC / Swift ABI Katmanı                                 │
│    • pear-runtime: libSystem/Mach/XNU, libdispatch, Apple libc++ (std::__1)  │
│    • pear-libobjc: Apple 64-bit ABI, ARC, blocks, DWARF LSDA EH, yansıma     │
│    • pear-swift-rt: Swift 5 ABI, metadata çözümleme, _TtCs12_SwiftObject     │
├──────────────────────────────────────────────────────────────────────────────┤
│ 3. Grafik & Shader Hattı (pear-metal-vk + pear-graphics + pear-image)        │
│    • Metal ObjC API (MTLDevice, MTLCommandQueue/Buffer/Encoder, CAMetalLayer)│
│    • MTLB / AIR bitcode + SPIR-V hattı ve Vulkan 1.2/1.3 (swapchain/readback)│
│    • Bit-birebir Khronos ASTC LDR çözücü, RwLock texture view aliasing       │
│    • 3D kırpma korumalı, bölmesiz özelleştirilmiş tarama-satırı rasterizer   │
├──────────────────────────────────────────────────────────────────────────────┤
│ 4. Uygulama ve Sistem Çerçeveleri                                            │
│    • pear-foundation: CoreFoundation/Foundation, NSURLSession &              │
│      NSURLConnection HTTP/HTTPS yığını (PEAR_NET_TRACE=1 desteği)            │
│    • Security (SecItem/SecKey/TLS), CFNetwork, AudioToolbox/AVAudioSession,  │
│      Accelerate, CoreMotion, GameController ve taşınabilir UIKit köprüleri   │
├──────────────────────────────────────────────────────────────────────────────┤
│ 5. Konak Platform Sınırı (pear-host → pear-host-linux / pear-host-macos)     │
│    • X11 / Wayland / AppKit pencere & yüzeyler + headless PPM döküm kipi     │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Ölçülen Referans Tablosu (Genshin Impact v7.2.0)

[com.miHoYo.GenshinImpact-7.2.0.json](census/com.miHoYo.GenshinImpact-7.2.0.json) sayımı ile başlangıç (`2026-11-26`) ve Faz 5.19 (`2026-11-27`) `pearruntime GenshinImpact.app --list-missing-summary` ölçümlerinin karşılaştırması:

| Ölçüt | Başlangıç Ölçümü | Güncel Durum (Faz 5.19) |
| --- | --- | --- |
| Yüklenen İmaj (`images`) | Tek imaj | **3** (`GenshinImpact` + gömülü `@rpath` framework'ler) |
| Gerçeklenen Sınıf (`classes`) | Kısmi | **1.472** |
| Gerçekleşmemiş Sınıf (`unrealized`) | **236** | **0** |
| Kategori (`categories`) / Selector (`selectors`) | Kısmi | **34** / **12.556** |
| Eksik Sembol (`missing symbols`) | **2.379** (44 kütüphanede 2.828 import) | **0** (`missing symbols (0):`) |
| Saf-Rust Test Paketi (`cargo test`, 12 crate) | — | **68 / 68 geçti** (`0` hata, `0` yeni uyarı) |
| Çalışma Zamanı İlerlemesi | Başlangıçta duruyordu | `Home LoadScene` → canlı HTTPS `query_region_list` (`200 OK`, `3020 B`) → `OnConnectGlobalDispatch` → `1280×720` oyun içi UI çizimi |

Ek envanter verileri:
- **Metal Shader (`MTLB`):** 2.069 derlenmiş Metal shader kütüphanesi (`globalgamemanagers.assets`: 1.403, `resources.assets`: 666), `Data/` altında 0 adet GLSL oyun shader'ı.
- **Swift ABI + Overlay İthalatları:** 1.264 sembol (`stdlib`, `Foundation`, `_swift_*` runtime, `Dispatch`, `ObjectiveC`, `CoreGraphics`) [`pear-swift-rt`](../rust/crates/pear-swift-rt) üzerinden sıfır eksik ile çözülür.
- **Gömülü Framework'ler:** `@rpath/AstrolabeCommon.framework/AstrolabeCommon` ve `@rpath/Telemetry.framework/Telemetry` (`pear-dyld` tarafından yüklenir).

---

## 3. Aşamalı Uygulama Fazları ve Mevcut Durum

### Faz 1 — Çoklu İmaj `pear-dyld`, `libc++` ve Darwin Çekirdek ABI (**Tamamlandı**)
1. [`pear-macho`](../rust/crates/pear-macho) içinde `LC_RPATH` ayrıştırması; [`pear-dyld`](../rust/crates/pear-dyld) içinde `@rpath` / `@executable_path` / `@loader_path` bağımlılık grafiği yükleyici, topolojik başlatma sırası ve çapraz imaj sembol çözümleme tamamlandı.
2. [`pear-runtime`](../rust/crates/pear-runtime) içinde çoklu-imaj `_dyld_*` kayıt defteri, misafir `dlopen`/`dlsym`/`dladdr`, `__unwind_info` / `__eh_frame` C++ EH kaydı, `apple_libcxx` (`std::__1`), `os_unfair_lock`, `mach_vm_*`, kqueue ve `libdispatch` senkronizasyon ilkelleri eklendi.

### Faz 2 — Grafik ve Shader Motoru (`pear-metal-vk`) (**Tamamlandı**)
1. [`pear-metal-vk`](../rust/crates/pear-metal-vk) içinde Metal ObjC API (`MTLDevice`, `MTLCommandQueue`, `MTLCommandBuffer`, render/blit encoder'lar, `MTLBuffer`/`MTLTexture`, `CAMetalLayer`), Vulkan swapchain clear/present/readback ve `MTLB` / AIR bitcode + SPIR-V altyapısı kuruldu.
2. Bit-birebir Khronos ASTC LDR çözücü (`astc.rs`), `Arc<RwLock<Vec<u8>>>` canlı texture view aliasing, 3D clip/negatif-`w` korumaları, ARC bellek sahipliği ve bölmesiz (`sdiv`-siz) özelleştirilmiş tarama-satırı rasterizer eklendi.

### Faz 3 — Apple Swift ABI ve ObjC-Interop (`pear-swift-rt`) (**Tamamlandı**)
1. [`pear-swift-rt`](../rust/crates/pear-swift-rt) üzerinde temiz-oda Swift 5 ABI runtime ve `Foundation`/`Dispatch`/`ObjectiveC`/`CoreGraphics` sembol katmanı tamamlandı.
2. [`pear-libobjc`](../rust/crates/pear-libobjc) ile dinamik Swift/ObjC sınıf realizasyonu ve `_TtCs12_SwiftObject` köprüsü kurularak Genshin Impact'teki **236** unrealized sınıf **0**'a indirildi.

### Faz 4 — Taşınabilir Çerçeveler (`Foundation` / `Security` / `UIKit` / `Accelerate`) (**Tamamlandı**)
1. [`pear-foundation`](../rust/crates/pear-foundation) içinde `NSString`, koleksiyonlar, `NSMapTable`/`NSHashTable`/`NSCache`, `NSUserDefaults`, `NSRunLoop`, `NSTimeZone` ve `NSURLSession` / `NSURLConnection` HTTP/HTTPS istemcisi yazıldı.
2. [`pear-runtime`](../rust/crates/pear-runtime) içindeki `Security`, `CoreGraphics`, `CFNetwork`, `Accelerate`, `AudioToolbox`/`AVAudioSession`, `CoreMotion`, `GameController` ve taşınabilir `UIKit` köprüleriyle `GenshinImpact.app` **0 eksik sembole** ulaştı.

### Faz 5 — Genshin Impact Uçtan Uca Entegrasyon (**Faz 5.1–5.19 Tamamlandı; 5.20+ Aktif**)
- **Tamamlananlar (5.1 – 5.19):**
  - Unity `il2cpp` ve `UnityAppController` tam başlatması (`GameManager Awake -> ShowLogo -> Home LoadScene -> LoginMainPageContext OnPreInit -> InitServerDispatch`).
  - `PEAR_NET_TRACE=1` telemetrisi ve gerçek `NSURLConnection` HTTPS akışı ile `https://dispatchosglobal.yuanshen.com/query_region_list` sunucusundan `status=200`, `3020` bayt yanıt alınıp `NetworkManager OnConnectGlobalDispatch` aşamasına geçildi.
  - `NSDictionary keyEnumerator` içindeki `store()` kilitlenmesinin giderilmesiyle (`Phase 5.19`) açılış akışı ilerledi ve ilk `1280×720` oyun içi UI penceresi çizildi (`Network error / Failed to connect to the server 4204 / Confirm` diyalog ekranı).
- **Sıradaki Adımlar (Faz 5.20+):**
  1. `OnConnectGlobalDispatch` sonrası ortaya çıkan `4204` bağlantı/çözümleme hatasının kaynağını ([`security.rs`](../rust/crates/pear-runtime/src/security.rs) içindeki `sec_key_copy_passthrough` RSA `SecKey` düzeneği veya takip eden `query_cur_region` / TLS / başlık akışı) gidermek.
  2. `sample` profilinde öne çıkan `rasterize_textured_span` sıcak noktası için `1:1` opak tarama-satırı `memcpy` hızlı yolunu eklemek ve donanım Vulkan / Mesa Lavapipe shader yürütmesini tam sahne çizimine taşımak.
  3. Tam doğrulamayı Linux ARM64 Vulkan penceresinde uçtan uca tekrarlamak.

---

## 4. Lisans ve Temiz Oda Uyumu ([independent-runtime.md](independent-runtime.md))

- **MIT ([LICENSE](../LICENSE)):** `pear-macho`, `pear-dyld`, `pear-libobjc`, `pear-swift-rt`, `pear-foundation`, `pear-graphics`, `pear-image`, `pear-metal-vk`, `pear-runtime` ve tüm özgün `pear-*` crate'leri (çekirdek çalışma zamanında sıfır harici `crates.io` bağımlılığı ilkesiyle).
- **Apache-2.0 / LLVM & Swift:** `apple/swift`, `swift-corelibs-foundation`, `swift-corelibs-libdispatch` (`NOTICE` korunur).
- **Üçüncü Taraf ([THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md)):** `PearANGLE` (BSD-3), `PearLegacyEngine` (MPL-2.0 / GPL-3.0), `PearLegacyJIT` (0BSD / Dynarmic).
- **APSL 2.0 (`xnu`, `dyld`, `objc4`):** Yalnızca temiz-oda ABI/semantik referansı olarak kullanılır; kod alınması durumunda `third_party/` altında ayrı tutulur.
