# Bootstrap والبيئة الأصلية

## لماذا targetSdk 28
منذ Android 10، التطبيقات التي تستهدف API 29 أو أعلى لا تستطيع تشغيل ملفات تنفيذية من مجلد بياناتها (قيد W^X).
الـ terminal الحقيقي وتثبيت toolchains يعتمدان على ذلك، لذلك يستهدف Stroid **API 28** (نفس نهج Termux)
ويُوزَّع خارج Google Play (GitHub Releases و F-Droid).

## هيكل البيئة
```
<filesDir>/usr    PREFIX: bin, lib, etc, tmp, share
<filesDir>/home   HOME
```
في المرحلة 0 يُنشأ هذا الهيكل فقط (`Bootstrap.ensureLayout()`).
ملف `usr/.stroid-bootstrap-complete` علامة اكتمال تثبيت الحزم الأساسية.

## ما تبقى من bootstrap (المرحلة 0 ثم 1)
- حزم Termux الجاهزة مبنية لمسار `/data/data/com.termux/files/usr`، ولا تصلح لتطبيقنا (`dev.stroid.app`)
  لأن المسارات مضمّنة داخل الملفات التنفيذية.
- الخيارات:
  1. **بناء مستودع حزم خاص بنا** بـ PREFIX الخاص بـ Stroid، باستخدام `termux-packages` (الأقوى والأنظف).
  2. استخدام مجلد PREFIX بمسار متوافق عبر proot (أبطأ).
- القرار المقترح: الخيار 1، مع حزمة bootstrap أولية صغيرة (shell، coreutils، apt/pkg، curl، git) تُنزَّل عند أول تشغيل.
- الحزمة تُبنى في CI وتُرفع كـ GitHub Release، والتطبيق يتحقق من checksum قبل فكها.
