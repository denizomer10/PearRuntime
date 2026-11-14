# iOSSupport'suz PearRuntime: mimari ve temiz oda kuralları

Durum: 2026-11-14 itibarıyla ilk hedef (GTA: Vice City – Definitive Edition
1.84, Unreal Engine 4, Metal) orijinal IPA'dan, Mac Catalyst dönüşümü ve
iOSSupport olmadan oynanış sahnesine ulaşıyor. Güncel durum, çalıştırma adımları ve sıradaki işler için
[HANDOFF.md](HANDOFF.md).

## 1. Hedef mimari

```
 ┌──────────────────────────────── uygulama (iOS IPA) ─────────────────────────────────┐
 │  arm64 iOS uygulaması                     │  32-bit iOS uygulaması (armv6/armv7)       │
 ├───────────────────────────────────────────┼───────────────────────────────────────────┤
 │  PearHost (host/)                          │  PearLegacyEngine (touchHLE çatalı)        │
 │   UIKit ve iOS'a özel çerçevelerin kendi   │   kendi HLE çerçeveleri                     │
 │   uygulaması; eksik çağrı raporlayıcısı    │                                            │
 ├───────────────────────────────────────────┴───────────────────────────────────────────┤
 │  Darwin sistem katmanı                                                                   │
 │   macOS: sistemin kendi libSystem / libobjc / CoreFoundation / Foundation / Metal'i      │
 │   Linux: PearDarwin (yazılacak) – Mach-O yükleyici, dyld, Mach/BSD → Linux sistem         │
 │          çağrıları (xnu referans), objc4, CoreFoundation, Foundation                     │
 ├──────────────────────────────────────────────────────────────────────────────────────────┤
 │  CPU: Apple silicon / arm64 Linux'ta doğal yürütme                                        │
 │       PearLegacyJIT: ARMv7→arm64 (bugün); A64→x86-64 (Dynarmic'in A64 ön ucu + x64 arka   │
 │       ucu ile, x86-64 Linux için)                                                         │
 ├──────────────────────────────────────────────────────────────────────────────────────────┤
 │  Grafik: PearANGLE (OpenGL ES → Metal / Vulkan), PearMetal (Metal → Vulkan, yazılacak)   │
 ├──────────────────────────────────────────────────────────────────────────────────────────┤
 │  Pencere/girdi arka ucu: AppKit (macOS, host/uikit/backend/appkit) │ SDL/Wayland (Linux)  │
 └──────────────────────────────────────────────────────────────────────────────────────────┘
```

Bugünkü Catalyst yolu (`modules/`, `build.sh`) değişmeden duruyor ve
PearSilicon'un varsayılanı olarak kalıyor. Yeni yol `host/` altında; uygulama
başına seçilecek.

### Neden macOS platformu?

`tools/host-prepare.sh` uygulamanın Mach-O derleme sürümünü (LC_BUILD_VERSION)
**macOS** olarak yeniden etiketler. Böylece dyld `/System/Library/...`
yollarını macOS'un kendisinden çözer, `/System/iOSSupport` hiç devreye girmez.
GameController, GameKit, StoreKit, AVFoundation, CoreMotion, UserNotifications
gibi çerçevelerin macOS sürümleri aynı yolda bulunduğu için yeniden yazılması
gerekmez (Linux'ta gerekir). Yalnızca macOS'ta karşılığı olmayanlar (UIKit,
iAd, …) PearHost imajlarına yönlendirilir.

Bu, Linux hedefiyle aynı modeldir: uygulama, "iOS" yerine bizim sağladığımız
bir platform üzerinde çalışır.

## 2. Temiz oda kuralları

Amaç: Apple kodunu kopyalamadan; arayüz ve gözlenen davranıştan uyumlu bir
uygulama yazmak.

**İzinli kaynaklar**
- SDK'daki genel başlık dosyaları ve `.tbd` stub'ları: yalnızca arayüz bilgisi
  (isim, imza, enum değeri, struct düzeni). Başlıklar depoya kopyalanmaz;
  `host/uikit/include/PearUIKit.h` kendi bildirimlerimizdir.
- Apple'ın genel belgeleri, WWDC oturumları.
- Açık kaynak Apple kodu (`apple-oss-distributions/*`: xnu, dyld, objc4, CF,
  Libc, libmalloc, …). Lisansı korunur (§3).
- Analiz edilen uygulamanın kendi ikili dosyası: import listesi, selector
  adları, çalışma zamanında yaptığı çağrılar (`tools/ios-import-census.py`,
  eksik çağrı raporu).
- Kendi test uygulamalarımızın gerçek iOS/iOSSupport üzerinde siyah kutu olarak
  çalıştırılıp davranışının kaydedilmesi.

**Yasak (uygulayıcı için)**
- `/System/iOSSupport` ve dyld shared cache içindeki Apple ikililerinin
  disassembly/decompile'ı, `class-dump` çıktısı, özel metot gövdeleri.
- Sızdırılmış Apple kaynak kodu.
- Bu tür materyalden türetilmiş notlar.

**Uygulama**
- Her dosyanın başında dayandığı kaynak yazılır (ör. "Clean-room: from the
  public UIAlertController documentation").
- Uygulamanın bir özel (private) anahtar/selector'a ihtiyaç duyduğu
  görülürse, kaynağı **uygulamanın kendi davranışı** olarak not edilir (ör.
  `UIDevice _feedbackSupportLevel`: GTA VC'nin KVC çağrısı, eksik çağrı
  raporundan).
- Tek geliştiricili düzende "spec ekibi" yoktur; bu yüzden ikili analiz hiç
  yapılmaz, yalnızca siyah kutu gözlem yapılır.

## 3. Lisanslar

- PearRuntime: MIT. `host/` altındaki kod MIT ve tamamen bizim.
- xnu, objc4, dyld, Libc, CF: **APSL 2.0** (dosya bazlı copyleft). Bu kodlar
  kullanılacaksa `third_party/<proje>` altında, kendi lisans dosyasıyla, ayrı
  tutulur; değiştirilen APSL dosyaların kaynağı yayımlanır. APSL 2.0, GPL ile
  uyumsuzdur.
- libdispatch, swift, swift-corelibs-foundation: Apache 2.0 (MIT ile uyumlu,
  NOTICE korunur).
- PearLegacyEngine MPL-2.0, PearLegacyJIT (Dynarmic) 0BSD/GPL bileşenleri
  açısından ayrıca kontrol edilmeli.

## 4. xnu'nun rolü

xnu, Linux tarafında (PearDarwin) Darwin sistem çağrısı ABI'sinin
referansıdır: BSD sistem çağrısı numaraları ve semantiği
(`bsd/kern/syscalls.master`), Mach trap'leri (`osfmk/kern/syscall_sw.c`),
`mach_msg`, port ve görev modeli, Mach-O ve `posix_spawn` davranışı. macOS
hedefinde gerek yoktur; uygulama gerçek çekirdekle konuşur.

## 5. Yol haritası

| Aşama | Kapsam | Durum |
| --- | --- | --- |
| 0 | Sayım (census) ve eksik çağrı raporlayıcısı | bitti |
| 1 | Oyunlar için UIKit çekirdeği (uygulama yaşam döngüsü, pencere, view, ekran, dokunma, uyarı) | GTA VC menüye ulaşıyor |
| 2 | Oyun servisleri: girdi eşleme, pointer lock, sanal gamepad, depolama yolları; Catalyst modüllerinin host'a taşınması | sırada |
| 3 | Genel UIKit: metin, kaydırma, tablo/koleksiyon, Auto Layout, gezinme | sonra |
| 4 | SwiftUI | ayrı karar |
| 5 | Linux: PearDarwin + SDL/Wayland arka ucu + PearANGLE/Vulkan; ardından PearMetal | sonra |
