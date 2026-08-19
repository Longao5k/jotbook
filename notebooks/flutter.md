---
name: flutter
description: Flutter / Dart —— 依赖、代码生成、构建、真机调试
tags: [mobile, dart]
vars:
  device:
    desc: 目标设备
    from: shell
    cmd: flutter devices --machine
  level:
    desc: 版本级别
    from: ask
    options: ["patch", "minor", "major"]
---

## 拉取依赖

```sh
flutter pub get
```

## 升级依赖到允许范围内的最新版

```sh
flutter pub upgrade
```

## 升级依赖并突破 pubspec 的版本约束

```sh
flutter pub upgrade --major-versions
```

## 代码生成，跑一次

freezed / json_serializable / retrofit 改完模型后跑这个。--delete-conflicting-outputs 用来解决「输出文件已存在」的报错。

```sh @tags=codegen
dart run build_runner build --delete-conflicting-outputs
```

## 代码生成，监听文件变化

开发时挂在后台。

```sh @tags=codegen
dart run build_runner watch --delete-conflicting-outputs
```

## 代码生成（旧写法）

Flutter 2 时代的写法，新版本已改为 dart run，但很多老项目文档里还是这个。

```sh @tags=codegen
flutter packages pub run build_runner watch --delete-conflicting-outputs
```

## 清理构建缓存

改了原生依赖、切了 Flutter 版本、或者构建报玄学错误时的第一反应。

```sh
flutter clean
```

## 清理并重新拉依赖

```sh
flutter clean && flutter pub get
```

## 环境体检

```sh
flutter doctor -v
```

## 列出可用设备

```sh
flutter devices
```

## 运行到指定设备

```sh
flutter run -d {{device}}
```

## 以 release 模式运行

排查性能问题必须用 release，debug 模式的性能没有参考价值。

```sh
flutter run --release
```

## 运行时指定编译期变量

```sh
flutter run --dart-define={{key}}={{value}}
```

## 构建 Android APK

```sh @tags=build
flutter build apk --release
```

## 构建按 ABI 拆分的 APK

体积能小一半以上。

```sh @tags=build
flutter build apk --release --split-per-abi
```

## 构建 Google Play 用的 AAB

```sh @tags=build
flutter build appbundle --release
```

## 构建 iOS 归档

```sh @platform=macos @tags=build
flutter build ipa --release
```

## 构建 Web

```sh @tags=build
flutter build web --release
```

## 构建 Windows 桌面版

```sh @platform=windows @tags=build
flutter build windows --release
```

## 静态分析

```sh
flutter analyze
```

## 自动修复 lint 问题

```sh
dart fix --apply
```

## 格式化全部代码

```sh
dart format .
```

## 跑全部测试

```sh @tags=test
flutter test
```

## 跑单个测试文件

```sh @tags=test
flutter test {{file}}
```

## 跑测试并生成覆盖率

```sh @tags=test
flutter test --coverage
```

## 查看依赖树

排查版本冲突用。

```sh
flutter pub deps
```

## 查看某个包为什么被引入

```sh
flutter pub deps --style=compact | grep {{package}}
```

## 升级 Flutter SDK

```sh
flutter upgrade
```

## 切换到指定 channel

```sh
flutter channel {{channel}} && flutter upgrade
```

## 查看当前版本与 channel

```sh
flutter --version
```

## 使用国内镜像（当前会话，PowerShell）

首次下载 Flutter 依赖或 Gradle 卡住时用。

```ps1 @platform=windows @tags=china
$env:PUB_HOSTED_URL = "https://pub.flutter-io.cn"
$env:FLUTTER_STORAGE_BASE_URL = "https://storage.flutter-io.cn"
```

## 使用国内镜像（当前会话，bash）

```sh @platform=linux,macos @tags=china
export PUB_HOSTED_URL=https://pub.flutter-io.cn
export FLUTTER_STORAGE_BASE_URL=https://storage.flutter-io.cn
```

## 生成应用图标

需要先在 pubspec.yaml 配置 flutter_launcher_icons。

```sh
dart run flutter_launcher_icons
```

## 生成启动页

```sh
dart run flutter_native_splash:create
```

## 查看 Android 签名指纹

接微信、高德、Firebase 时要填的那个 SHA-1。

```sh
keytool -list -v -keystore {{keystore}} -alias {{alias}}
```

## 抓 Android 设备日志

```sh
adb logcat -s flutter
```

## 列出已连接的 adb 设备

```sh
adb devices -l
```

## 无线调试连接设备

```sh
adb connect {{ip}}:5555
```

## 递增版本号

```sh
dart run cider bump {{level}}
```
