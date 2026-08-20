---
name: flutter
description: Flutter / Dart - dependencies, codegen, builds, device debugging
tags: [mobile, dart]
vars:
  device:
    desc: target device
    from: shell
    cmd: flutter devices --machine
  level:
    desc: version bump level
    from: ask
    options: ["patch", "minor", "major"]
---

## Fetch dependencies

```sh
flutter pub get
```

## Upgrade dependencies within their allowed ranges

```sh
flutter pub upgrade
```

## Upgrade past the version constraints in pubspec

```sh
flutter pub upgrade --major-versions
```

## Run code generation once

Run this after changing models for freezed, json_serializable or retrofit. --delete-conflicting-outputs clears the "output already exists" error.

```sh @tags=codegen
dart run build_runner build --delete-conflicting-outputs
```

## Run code generation and watch for changes

Leave it running in the background while developing.

```sh @tags=codegen
dart run build_runner watch --delete-conflicting-outputs
```

## Run code generation (legacy form)

The Flutter 2 spelling. Newer versions use dart run, but plenty of older project docs still show this.

```sh @tags=codegen
flutter packages pub run build_runner watch --delete-conflicting-outputs
```

## Clear the build cache

The first thing to try after changing a native dependency, switching Flutter version, or hitting an inexplicable build error.

```sh
flutter clean
```

## Clean and refetch dependencies

```sh
flutter clean && flutter pub get
```

## Check the toolchain

```sh
flutter doctor -v
```

## List available devices

```sh
flutter devices
```

## Run on a specific device

```sh
flutter run -d {{device}}
```

## Run in release mode

Performance work must be done in release; debug-mode numbers mean nothing.

```sh
flutter run --release
```

## Run with a compile-time variable

```sh
flutter run --dart-define={{key}}={{value}}
```

## Build an Android APK

```sh @tags=build
flutter build apk --release
```

## Build APKs split per ABI

More than halves the size.

```sh @tags=build
flutter build apk --release --split-per-abi
```

## Build an AAB for Google Play

```sh @tags=build
flutter build appbundle --release
```

## Build an iOS archive

```sh @platform=macos @tags=build
flutter build ipa --release
```

## Build for the web

```sh @tags=build
flutter build web --release
```

## Build the Windows desktop app

```sh @platform=windows @tags=build
flutter build windows --release
```

## Run static analysis

```sh
flutter analyze
```

## Auto-fix lint issues

```sh
dart fix --apply
```

## Format all code

```sh
dart format .
```

## Run every test

```sh @tags=test
flutter test
```

## Run a single test file

```sh @tags=test
flutter test {{file}}
```

## Run the tests with coverage

```sh @tags=test
flutter test --coverage
```

## Show the dependency tree

For tracking down version conflicts.

```sh
flutter pub deps
```

## Find out why a package was pulled in

```sh
flutter pub deps --style=compact | grep {{package}}
```

## Upgrade the Flutter SDK

```sh
flutter upgrade
```

## Switch channel

```sh
flutter channel {{channel}} && flutter upgrade
```

## Show the current version and channel

```sh
flutter --version
```

## Use a China mirror for this session (PowerShell)

For when the first dependency or Gradle download stalls.

```ps1 @platform=windows @tags=china
$env:PUB_HOSTED_URL = "https://pub.flutter-io.cn"
$env:FLUTTER_STORAGE_BASE_URL = "https://storage.flutter-io.cn"
```

## Use a China mirror for this session (bash)

```sh @platform=linux,macos @tags=china
export PUB_HOSTED_URL=https://pub.flutter-io.cn
export FLUTTER_STORAGE_BASE_URL=https://storage.flutter-io.cn
```

## Generate app icons

Configure flutter_launcher_icons in pubspec.yaml first.

```sh
dart run flutter_launcher_icons
```

## Generate the splash screen

```sh
dart run flutter_native_splash:create
```

## Show the Android signing fingerprint

The SHA-1 that WeChat, Amap and Firebase ask for.

```sh
keytool -list -v -keystore {{keystore}} -alias {{alias}}
```

## Tail Android device logs

```sh
adb logcat -s flutter
```

## List connected adb devices

```sh
adb devices -l
```

## Connect to a device over wireless debugging

```sh
adb connect {{ip}}:5555
```

## Bump the version number

```sh
dart run cider bump {{level}}
```
