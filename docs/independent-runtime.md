# iOSSupport'suz ve Bağımsız PearRuntime: Mimari ve Temiz Oda Kuralları

**Güncelleme tarihi:** 2026-11-27

**Durum özeti:** PearRuntime bugün **üç paralel uyumluluk hattını** tek bir temiz-oda Rust çalışma alanı ([rust/Cargo.toml](../rust/Cargo.toml)) altında birleştirir:
1. **Taşınabilir Bağımsız Çalışma Zamanı (`pearruntime` CLI — macOS & Linux ARM64):** Kendi çoklu-imaj Mach-O yükleyicisi ([`pear-dyld`](../rust/crates/pear-dyld)), Apple 64-bit Objective-C ABI'si ([`pear-libobjc`](../rust/crates/pear-libobjc)), Swift 5 çalışma zamanı ([`pear-swift-rt`](../rust/crates/pear-swift-rt)), saf-Rust `Foundation`/`CoreGraphics`/`NSURLSession` ([`pear-foundation`](../rust/crates/pear-foundation), [`pear-graphics`](../rust/crates/pear-graphics), [`pear-image`](../rust/crates/pear-image)) ve Metal → Vulkan/AIR/ASTC hattı ([`pear-metal-vk`](../rust/crates/pear-metal-vk)) ile `GenshinImpact.app` üzerinde `0` eksik sembol ve `0` gerçekleşmemiş sınıf ile Unity `il2cpp` açılışını, canlı HTTPS `query_region_list` (`200 OK`) isteğini ve `1280×720` kare çizimini tamamlar.
2. **Yerel macOS `PearHost` Çerçeveleri (`build/host/`):** `/System/iOSSupport` ve Mac Catalyst kullanmadan, Apple silicon macOS üzerinde `GTA: Vice City – Definitive Edition` oyun içi sahnesine ve `Minecraft` dünya yüklemesine ulaşır.
3. **32-bit `PearLegacyEngine` + `PearLegacyJIT`:** `armv6`/`armv7` iOS yapımları (ör. `Miku Flick`) için Dynarmic tabanlı yeniden derleyici ve uyumluluk motoru ([`third_party/legacy-engine`](../third_party/legacy-engine), [`third_party/legacy-jit`](../third_party/legacy-jit), [`pear-legacy-host`](../rust/crates/pear-legacy-host), [`pear-jit`](../rust/crates/pear-jit)).

Güncel geliştirme devir notları ve doğrulama komutları için [HANDOFF.md](HANDOFF.md), tam göç ve bağımsızlık planı için [independence-plan.md](independence-plan.md), Linux ARM64 + Metal + Swift aşamaları için [linux-metal-swift-roadmap.md](linux-metal-swift-roadmap.md) belgelerine bakın.

---

## 1. Hedef ve Çok Katmanlı Mimari

**Hedef:** Apple ikililerini Mac Catalyst dönüşümüne, `/System/iOSSupport` yığınına veya kapalı kaynaklı iOS çerçevelerine ihtiyaç duymadan; önce **macOS** ve **Linux ARM64**, ileride JIT destekli `x86_64` konaklar üzerinde temiz-oda Rust katmanlarıyla çalıştırmak. (Eski Mac Catalyst katmanları tamamen emekliye ayrılmış ve çalışma alanından kaldırılmıştır.)

```text
┌─────────────────────────── iOS / iPadOS Uygulaması (.app) ───────────────────────────┐
│          64-bit arm64 Mach-O (+ @rpath/*.framework)   │     32-bit armv6/armv7       │
├───────────────────────────┬───────────────────────────┼──────────────────────────────┤
│ Taşınabilir Çalışma Zamanı│ Yerel macOS PearHost      │ 32-bit Legacy & JIT          │
│ (`pearruntime` CLI)       │ (`build/host/*.framework`)│ (`PearLegacyEngine`)         │
├───────────────────────────┼───────────────────────────┼──────────────────────────────┤
│ • pear-macho / pear-dyld  │ • pear-uikit / pear-nib   │ • pear-legacy-host           │
│ • pear-libobjc            │ • pear-metalkit           │ • pear-jit (Dynarmic)        │
│ • pear-swift-rt           │ • pear-gles / pear-glkit  │ • third_party/legacy-engine  │
│ • pear-runtime            │   (Metal-only PearANGLE)  │ • third_party/legacy-jit     │
│ • pear-foundation         │ • pear-objc (statik ABI)  │                              │
│ • pear-graphics/image     │ • pear-keychain           │                              │
│ • pear-metal-vk           │ • pear-controller         │                              │
├───────────────────────────┴───────────────────────────┴──────────────────────────────┤
│ Ortak Konak Soyutlaması: pear-host (HostPlatform trait)                              │
│   ↳ pear-host-macos (AppKit / QuartzCore / IOKit)                                    │
│   ↳ pear-host-linux (X11 / Wayland / Vulkan / XDG)                                   │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

### İki 64-bit Yürütme Modeli Nasıl Çalışır?

- **Yerel macOS `PearHost` yolu (`tools/host-prepare.sh`):** Mach-O `LC_BUILD_VERSION` alanını **macOS** olarak yeniden etiketler; böylece sistem `dyld` çekirdek/sistem kütüphanelerini (`/System/Library/...`) doğrudan macOS'tan çözer ve `/System/iOSSupport` hiç yüklenmez. macOS'ta bulunmayan veya iOS ABI'sinden farklı olan çerçeveler (`UIKit`, `OpenGLES`, `MetalKit`, `GLKit`) Rust'tan statik Mach-O `__objc_classlist` verisi üreten [`pear-objc`](../rust/crates/pear-objc) ve `PearHost` görüntülerine yönlendirilir.
- **Taşınabilir Bağımsız Yol (`pearruntime`):** Sistem `dyld` ve sistem `libobjc`/`Foundation` bağımlılığını da ortadan kaldırır. Misafir Mach-O imajını ve `@rpath` bağımlılıklarını [`pear-dyld`](../rust/crates/pear-dyld) haritalar, `objc_*` çağrılarını [`pear-libobjc`](../rust/crates/pear-libobjc) karşılar, Swift ABI sembollerini [`pear-swift-rt`](../rust/crates/pear-swift-rt) çözer ve grafik/ağ/güvenlik çağrıları [`pear-metal-vk`](../rust/crates/pear-metal-vk), [`pear-foundation`](../rust/crates/pear-foundation) ve [`pear-runtime`](../rust/crates/pear-runtime) üzerinden yürütülür. Böylece aynı çekirdek 12 crate Linux ARM64 ve macOS üzerinde özdeş biçimde derlenip test edilebilir.

---

## 2. Temiz Oda (Clean-Room) Kuralları

Amaç: Apple'ın kapalı kaynaklı ikili kodlarını kopyalamadan veya tersine mühendislik uygulamadan; yalnızca kamuya açık arayüzlerden ve gözlemlenen kara kutu davranıştan uyumlu bir Rust uygulaması geliştirmek.

**İzinli kaynaklar**
- Kamuya açık SDK başlık dosyaları ve `.tbd` stub'ları: yalnızca arayüz bilgisi (sınıf/metot adı, imzalar, enum değerleri, struct yerleşimi). Başlık dosyaları depoya kopyalanmaz; UIKit sınıf yerleşimleri [`pear-uikit/src/objc_classes.rs`](../rust/crates/pear-uikit/src/objc_classes.rs), Metal/Foundation/Swift ABI yapıları ilgili crate'lerin kaynak dosyalarında tanımlanır.
- Apple'ın kamuya açık geliştirici belgeleri, WWDC sunumları ve açık standartlar (Khronos Vulkan, SPIR-V, ASTC, PNG/JPEG/HTTP RFC'leri).
- Açık kaynaklı Apple dağıtımları (`apple-oss-distributions/*`: `xnu`, `dyld`, `objc4`, `CF`, `Libc`, `libmalloc`, …) yalnızca **APSL 2.0** koşulları ve referans sınırları içinde (bkz. §3).
- Çözümlenen misafir uygulamaların kendi import/selector envanteri ([`tools/ios-import-census.py`](../tools/ios-import-census.py), [`docs/census/`](census/) ve çalışma zamanı eksik-çağrı / `PEAR_MISSING_REPORT` JSONL çıktıları).
- Kendi test uygulamalarımızın gerçek iOS/macOS üzerinde kara kutu olarak çalıştırılıp gözlemlenmesi.

**Yasak (kesin sınır)**
- `/System/iOSSupport` ve `dyld_shared_cache` içindeki Apple ikililerinin disassembly/decompile edilmesi, `class-dump` çıktıları veya özel metot gövdelerinin çıkarılması.
- Sızdırılmış Apple kaynak kodu.
- Bu tür materyalden türetilmiş tasarım notları veya kopya sabitler.

**Uygulama disiplini**
- Yeni yazılan her temiz-oda modülün veya dosyanın başında dayandığı kaynak belirtilir (ör. `"Clean-room: from public Apple documentation / Khronos specification / observed app behavior"`).
- Bir oyunun ihtiyaç duyduğu belgesiz bir anahtar veya selector saptandığında, kaynağı **uygulamanın gözlenen davranışı** olarak not edilir (ör. çalışma zamanı eksik-çağrı günlüğü veya `--list-missing` çıktısı).
- Çekirdek çalışma zamanı crate'lerinde harici `crates.io` bağımlılığı tutulmaz (saf Rust + standart kütüphane ilkesi).

---

## 3. Lisanslar ve Üçüncü Taraf Sınırları

- **PearRuntime özgün Rust kodu:** [MIT Lisansı](../LICENSE). `rust/crates/*` altındaki tüm özgün crate'ler MIT lisanslıdır.
- **APSL 2.0 (`xnu`, `objc4`, `dyld`, `Libc`, `CF`):** Yalnızca ABI/hata-kodu referansı olarak incelenir. Doğrudan kod alınması halinde `third_party/<proje>` altında kendi lisans dosyasıyla ayrı tutulur; çekirdek `rust/crates/*` içine APSL kodu karıştırılmaz (APSL 2.0, GPL ile uyumsuzdur).
- **Apache-2.0 (`swift`, `swift-corelibs-foundation`, `swift-corelibs-libdispatch`):** MIT ile uyumludur (`NOTICE` korunur).
- **PearANGLE (`third_party/angle`):** BSD-3-Clause; Metal-only arm64 ve Vulkan arka uçlarını içerir.
- **PearLegacyEngine (`third_party/legacy-engine`) & PearLegacyJIT (`third_party/legacy-jit`):** MPL-2.0 / 0BSD / GPL-3.0 bileşenleri içerir; tam atıf ve bildirimler [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md) içindedir.

---

## 4. Konak ve Sistem Sınırı (`pear-host` & `pear-runtime`)

- **macOS Hedefinde:** Uygulama `PearHost` kipinde çalışırken gerçek Darwin çekirdeğiyle konuşur; [`pear-host-macos`](../rust/crates/pear-host-macos) yalnızca AppKit, QuartzCore ve IOKit temasını üstlenir.
- **Linux ARM64 ve Taşınabilir `pearruntime` Hedefinde:** [`pear-runtime`](../rust/crates/pear-runtime) Darwin `arm64` ABI farklarını (variadic çağrı düzeni, `errno` / sinyal numaraları, `stat`/`dirent`/`sockaddr` yerleşimleri, `os_unfair_lock`, `libdispatch`, Apple `libc++` `std::__1` uyumu, `NSURLSession`/`NSURLConnection` ağ yığını, `Security` katmanı) POSIX/Linux üzerine taşır; [`pear-host-linux`](../rust/crates/pear-host-linux) X11/Wayland/headless pencere ve yüzey yönetimini sağlar.

---

## 5. Yol Haritası Durumu (2026-11-27)

| Aşama / Kilometre Taşı | Kapsam | Durum |
| --- | --- | --- |
| **M1–M2** | Envanter, sayım (`census`), eksik-çağrı raporlayıcı ve Mac Catalyst'in kaldırılması | **Tamamlandı** |
| **M3–M4** | `pear-host` sınırı (`pear-host-macos`), CoreText tabanlı `UIFont` ve saf-Rust [`pear-image`](../rust/crates/pear-image) / [`pear-graphics`](../rust/crates/pear-graphics) | **Tamamlandı** |
| **M5–M8** | [`pear-macho`](../rust/crates/pear-macho), [`pear-dyld`](../rust/crates/pear-dyld), [`pear-libobjc`](../rust/crates/pear-libobjc), [`pear-foundation`](../rust/crates/pear-foundation) ve Vulkan-hazır `PearANGLE` | **Tamamlandı** |
| **M9–M10** | [`pear-host-linux`](../rust/crates/pear-host-linux) + birleşik [`pear-runtime`](../rust/crates/pear-runtime) (Windows iptal; Linux-öncelikli) | **Tamamlandı** |
| **Faz 1–4** | Çoklu-imaj `@rpath` yükleyici, [`pear-metal-vk`](../rust/crates/pear-metal-vk), [`pear-swift-rt`](../rust/crates/pear-swift-rt), `Security`/`Accelerate`/`Audio`/`UIKit` (`GenshinImpact.app` `0` eksik sembol) | **Tamamlandı** |
| **Faz 5.1–5.19** | Unity `il2cpp` açılışı, Khronos ASTC LDR çözücü, tarama satırı rasterizer, gerçek `NSURLSession` / `NSURLConnection` HTTPS (`query_region_list` `200 OK` → `OnConnectGlobalDispatch`), `1280×720` UI çizimi | **Tamamlandı** |
| **Faz 5.20+** | Dispatch sonrası `4204` / `SecKey` RSA akışının çözülmesi, 1:1 rasterizer hızlı yolu, tam Vulkan shader/GPU yürütmesi ve Linux ARM64 uçtan uca doğrulama | **Sürüyor** |
