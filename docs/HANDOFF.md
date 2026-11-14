# PearHost devir notları

Bir sonraki ajan veya geliştirici buradan başlasın. Mimari ve temiz oda
kuralları: [independent-runtime.md](independent-runtime.md). Bu dosya her
oturum sonunda güncellenir.

- Dal: `independent-runtime` (PearRuntime)
- Son güncelleme: 2026-11-14

## Durum

GTA: Vice City – Definitive Edition 1.84 (UE4, Metal), iOSSupport **ve Mac
Catalyst olmadan**:

- Orijinal IPA'dan (iOS platformlu, şifresiz ikili) doğrudan hazırlanıyor;
  PearSilicon'un Catalyst dönüşümüne gerek yok. 95 saniyelik çalışmada oyun
  HUD, mini harita ve araçla oynanış sahnesine ulaştı (otomatik devam eden
  sahne; girdi verilmedi).

- dyld bağlanıyor; 40 saniyelik bir çalışmada 1.298 imaj yüklendi, hiçbiri
  `/System/iOSSupport` altından değil (`DYLD_PRINT_LIBRARIES=1` ile doğrulandı).
  Not: `vmmap` iOSSupport yollarını gösterir, ama bunlar her süreçte eşlenen
  shared cache bölgeleridir; yükleme kanıtı değildir.
- `UIApplicationMain` → `IOSAppDelegate` → pencere ve Metal view → pak
  dosyaları yükleniyor → Rockstar intro → 3B sahne üzerinde CONTROLS menüsü.
- Girdi (fare/klavye/gamepad ile menüde ilerleme ve oynanış) **henüz
  denenmedi**.
- Ses, oyun içi oynanış, kayıt/yükleme **doğrulanmadı**.

Minecraft (Bedrock) 1.26.60, orijinal IPA'dan, iOSSupport ve Catalyst olmadan:

- `MainWindow.nib` (NIBArchive) bizim UINib yükleyicimizle açılıyor; uygulama
  delegesi, pencere ve view controller nib bağlantılarıyla kuruluyor.
- MTKView (bizim UIView tabanlı MetalKit'imiz) ile Metal çizimi: ana ekran
  ve "Loading.. / Survive" dünya yükleme ekranı görüldü, log'da dünya açılıyor
  ("Opening level ..."). Oynanış ve girdi denenmedi.
- Keychain, Catalyst'teki `modules/storage/KeychainShim.mm` ile (aynı dosya,
  host'a da derleniyor).

## Dosya düzeni

| Yol | İçerik |
| --- | --- |
| `host/core/` | `PearHostLog`, eksik çağrı raporlayıcısı (`PearMissing.m`), `pear_missing_zero` |
| `host/uikit/include/PearUIKit.h` | kendi UIKit bildirimlerimiz (ABI değerleri genel başlıklarla aynı) |
| `host/uikit/src/` | elle yazılmış sınıflar: UIApplication, UIView/UIWindow/UIViewController, UIScreen, UIDevice, UIResponder, UITouch/UIEvent, UIColor, UITraitCollection, UIAlertController |
| `host/uikit/backend/appkit/` | macOS arka ucu: NSWindow, layer hosting, display link, fare → dokunma, NSAlert |
| `host/uikit/src/UINib.m` | NIBArchive nib yükleyici: UINib, outlet bağlantıları, NSMainNibFile, NSBundle loadNibNamed:owner:options:, NSCoder geometri kategorisi |
| `host/metalkit/MTKView.m` | MetalKit.framework: UIView tabanlı MTKView (macOS'unki NSView olduğu için) |
| `host/opengles/EAGL.m` | OpenGLES.framework: GL'yi ANGLE'dan yeniden dışa aktarır; EAGL sabitleri; EAGLContext şimdilik nil döndürür |
| `host/core/PearCatalystCompat.h` | UIKit'e bağlı olmayan Catalyst modüllerini host'a derlemek için (şu an KeychainShim) |
| `tools/nibdump.py` | derlenmiş nib (NIBArchive) döküm aracı; biçim notları dosyanın başında |
| `host/uikit/generated/` | `tools/gen-uikit-stubs.py` çıktısı; elle düzenlenmez |
| `host/build-host.sh` | `build/host/UIKit.framework`, `iAd.framework` üretir |
| `tools/ios-import-census.py` | bir uygulamanın hangi API'leri kullandığını çıkarır |
| `tools/gen-uikit-stubs.py` | sayımdaki her UIKit adı için yer tutucu üretir |
| `tools/host-prepare.sh` | yüklü bir uygulamanın iOSSupport'suz kopyasını hazırlar |
| `tools/host-run.sh` | kopyayı kendi container'ında çalıştırır, rapor klasörü üretir |
| `docs/census/` | uygulama başına sayım JSON'ları (stub üretiminin girdisi) |
| `docs/runs/` | saklanan eksik çağrı raporları |

## Çalıştırma

```bash
host/build-host.sh
```

```bash
tools/host-prepare.sh ~/Downloads/GTA-Vice-City-Definitive-v1-84.ipa build/host-apps/gtavc-ipa.app
```

```bash
PEAR_HOST_SEED="$HOME/Library/Application Support/PearSilicon/Games/com.rockstargames.gtavc.de" tools/host-run.sh build/host-apps/gtavc-ipa.app build/host-apps/runs/latest 90
```

IPA, hedefin yanına `<ad>.payload/` olarak açılır (bir kez); kopyadaki
kaynaklar oraya mutlak symlink'tir. FairPlay ile şifreli ikililer (cryptid ≠ 0)
reddedilir. PearSilicon'un Catalyst'e hazırladığı `.app` de girdi olarak
kabul edilir.

`PEAR_HOST_SEED`, gerçek container'ı ilk seferde APFS klonu olarak kopyalar
(ek yer kaplamaz, asıl kayıtlara yazılmaz). GTA VC, 1,2 GB'lık indirilmiş pak
dosyalarını bu container'da arar; klon olmadan "Download Required" uyarısı
verir.

Ortam değişkenleri: `PEAR_MISSING_REPORT` (JSONL), `PEAR_MISSING_FATAL=1`
(ilk eksik çağrıda abort, yığın izi için), `PEAR_LOG_FILE`,
`PEAR_HOST_SCREEN=1280x720`, `PEAR_HOST_IOS_VERSION`.

## Eksik çağrı raporu nasıl okunur

Her satır bir API'nin **ilk** çağrısıdır (`event: first`); çıkışta (SIGTERM
dahil) `event: summary` satırları sayılarla yazılır. `caller`, çağıran
imaj+ofsettir (ör. `Gameface+0x9f3f8c`). Tür:

- `objc-instance` / `objc-class`: bizde olmayan selector; sıfır/nil döndü.
- `c-function`: üretilmiş C stub'ı; sıfır döndü.
- `behavior`: metot var ama istenen davranış eksik (ör. gesture recognizer'lar,
  nib yükleme, KVC ile istenen bilinmeyen anahtar).

Yeni bir eksik çağrı uygulanırken: davranışı genel belgeden yaz, dosya
başlığına kaynağı ekle, `host/build-host.sh` stub'ı otomatik düşürür.

## GTA VC'nin son çalışmadaki eksikleri (`docs/runs/gtavc-1.84-ipa-2026-11-14-missing.jsonl`)

| API | Sayı | Etki / yapılacak |
| --- | --- | --- |
| `+[UIStoryboard storyboardWithName:bundle:]` | 1 | açılış ekranı storyboard'u; nil yeterli görünüyor. Derlenmiş storyboard (`.storyboardc`) okuma ileride. |
| `UIAccessibilityIsVoiceOverRunning()` | 2 | NO doğru cevap; elle uygulanmalı. |
| `-[UITraitCollection displayGamut]` | 1 | P3 (1) döndürülmeli. |
| `-[UIImpactFeedbackGenerator initWithStyle:]` | 1 | nil döndü; haptik yok, ama nesne döndürmeli (sonraki `impactOccurred` nil'e gider, zararsız). |

Önceki çalışmalarda çözülenler: `UIDevice` KVC `_feedbackSupportLevel`
(çökme → 0 döndürülüyor), `UIAlertController` (NSAlert ile gösteriliyor ve
loglanıyor), `+[CADisplayLink displayLinkWithTarget:selector:]` (macOS'ta yok,
NSView display link'ine bağlandı), Backtrace'in MobileCoreServices sembolleri
(CoreServices'e yönlendirildi), Swift çalışma zamanı rpath'i.

## Minecraft'ın son çalışmadaki eksikleri (`docs/runs/minecraft-1.26.60-2026-11-14-missing.jsonl`)

| API | Etki / yapılacak |
| --- | --- |
| `-[UITextView setDelegate:/setAutocorrectionType:/setAutocapitalizationType:/setSmartQuotesType:]` | UITextView yalnızca stub; oyun içi metin girişi (sohbet, dünya adı) için gerçek bir metin girişi (UITextInput + AppKit metin girişi) gerekiyor. |
| `UIAccessibilityIsVoiceOverRunning()` | NO döndürmeli. |

## Bilinen sorunlar

1. `UIFont` sınıfı macOS'un `UIFoundation` özel çerçevesinde de var; objc iki
   kopya uyarısı veriyor. Uygulamanın doğrudan referansları bizimkine bağlanır,
   ama `NSClassFromString(@"UIFont")` hangisini döndüreceği belirsiz. UIFont'u
   elle uygularken bunu ele al.
2. `PLCrash*` sınıfları hem Gameface'te hem Backtrace_PLCrashReporter'da
   (uygulamanın kendi sorunu; Catalyst'te de var).
3. İlk çalışmada pencere bir kez büyüyüp küçülüyor (resize animasyonu); kaynağı
   bulunmadı.
4. Çoklu dokunma yok (tek fare = tek dokunma). Catalyst `input` modülündeki
   keymapper/sanal gamepad henüz host'a taşınmadı.
5. Ses yolu (AVAudioSession macOS'ta) doğrulanmadı.
6. Host OpenGLES, PearLegacyEngine'in macOS ANGLE derlemesini kullanıyor
   (`PEAR_HOST_ANGLE_DIR`, varsayılan `~/PearLegacyEngine/vendor/angle/out/MacMetal`,
   upstream ANGLE). PearANGLE çatalının macOS-platform derlemesine geçilmeli.
   EAGLContext henüz yok (nil döner); GLES oyunları için `modules/graphics-gl`
   EAGL-on-EGL kodu host'a taşınmalı.
7. macOS display link'i `setFrameInterval:` vb. için istisna fırlatıyor; host,
   kurulumda bu selector'ları deneyip fırlatanları değiştiriyor (log: "provided by
   the host").
8. Launch bloğundaki istisnalar artık yığın izi ile loglanıp abort ediliyor
   (AppKit bunları sessizce yutuyordu).

## Sıradaki işler (öncelik sırasıyla)

1. **Girdi:** GTA VC'de klavye/fare ve gamepad ile menü + oynanış testi.
   macOS GameController (GCKeyboard/GCMouse/GCController) doğal olarak
   çalışmalı; çalışmazsa `modules/input` mantığı `host/` altına taşınır.
2. Yukarıdaki dört eksik çağrıyı elle uygula.
3. Ses ve kayıt/yüklemeyi doğrula; `missing.jsonl` sıfıra inene kadar devam.
4. PearSilicon'a "PearHost modu" seçeneği (IPA'dan doğrudan; Catalyst dönüşümü atlanır): `host-prepare.sh` adımlarını
   kurulum hattına taşı, uygulama başına mod seçimi.
5. Minecraft: UITextView/metin girişi, dokunma/fare ve klavye ile oynanış.
   Diğer oyunlarla sayım (`docs/census/`): Asphalt 6 (32-bit, PearLegacyEngine),
   Miku Flick. Her yeni oyun yeni sınıflar getirir;
   üretici stub'ları otomatik ekler.
6. Linux hazırlığı: arka ucu (`backend/appkit`) SDL/Wayland eşleniğiyle
   çoğaltmak için `PearUIKitPrivate.h` arayüzü zaten platformdan bağımsız.
   PearDarwin (Mach-O yükleyici + Darwin ABI) ayrı bir proje olarak
   başlatılmalı; xnu sistem çağrısı referansıdır.
7. PearLegacyJIT entegrasyonu: x86-64 Linux için Dynarmic'in A64 ön ucu ile
   arm64 → x86-64 çeviri. Önce PearDarwin'in arm64 Linux'ta doğal çalışması
   gerekir.
