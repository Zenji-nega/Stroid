# البناء محلياً

## المتطلبات
- JDK 17
- Android SDK (compileSdk 35) و Android NDK
- Rust (stable) مع الهدف `aarch64-linux-android`
- `cargo-ndk`: `cargo install cargo-ndk --locked`
- متغير البيئة `ANDROID_NDK_HOME` يشير إلى الـ NDK

## بناء APK تجريبي
```sh
rustup target add aarch64-linux-android
./gradlew assembleDebug
```
الناتج: `app/build/outputs/apk/debug/app-debug.apk`

مهمة Gradle `cargoNdkBuild` تبني `libstroid_jni.so` تلقائياً قبل كل بناء.
لتجاوزها وإعادة استخدام المكتبة المبنية سابقاً: `./gradlew assembleDebug -PskipCargo`

## اختبار نواة Rust (بدون Android)
```sh
cd core
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## CI
ملف `.github/workflows/ci.yml` يشغّل:
1. **rust:** fmt و clippy والاختبارات.
2. **apk:** بناء APK تجريبي لمعمارية arm64-v8a ورفعه كـ artifact.
