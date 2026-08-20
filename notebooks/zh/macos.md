---
name: macos
description: macOS —— Homebrew、端口、权限、Finder 与系统设置
tags: [macos, shell]
platform: [macos]
---

## 查看谁占用了某个端口

```sh @tags=daily @tags=port
sudo lsof -nP -iTCP:{{port}} -sTCP:LISTEN
```

## 杀掉占用某端口的进程

```sh @tags=port @confirm
sudo kill -9 $(sudo lsof -t -iTCP:{{port}} -sTCP:LISTEN)
```

## 列出所有监听端口

```sh @tags=port
sudo lsof -nP -iTCP -sTCP:LISTEN
```

## Homebrew：安装

```sh @tags=brew
brew install {{package}}
```

## Homebrew：安装图形应用

```sh @tags=brew
brew install --cask {{app}}
```

## Homebrew：搜索

```sh @tags=brew
brew search {{keyword}}
```

## Homebrew：更新所有包

```sh @tags=brew @confirm
brew update && brew upgrade
```

## Homebrew：清理旧版本

brew 会留着所有历史版本，久了能占几十 G。

```sh @tags=brew @confirm
brew cleanup --prune=all
```

## Homebrew：看什么占了空间

```sh @tags=brew
brew list --formula | xargs -n1 -I{} sh -c 'echo "$(du -sh $(brew --cellar {}) 2>/dev/null | cut -f1)\t{}"' | sort -hr | head -20
```

## Homebrew：查看服务

数据库、redis 之类用 brew services 起的。

```sh @tags=brew
brew services list
```

## Homebrew：重启一个服务

```sh @tags=brew @confirm
brew services restart {{service}}
```

## Homebrew：换国内镜像

```sh @tags=brew @tags=china
export HOMEBREW_API_DOMAIN="https://mirrors.tuna.tsinghua.edu.cn/homebrew-bottles/api"
export HOMEBREW_BOTTLE_DOMAIN="https://mirrors.tuna.tsinghua.edu.cn/homebrew-bottles"
```

## 解除「已损坏，无法打开」的限制

从网上下载的未签名 app 会被 Gatekeeper 拦下。

```sh @confirm
sudo xattr -dr com.apple.quarantine "{{app}}"
```

## 查看文件的扩展属性

```sh
xattr -l {{file}}
```

## 在 Finder 里显示隐藏文件

```sh
defaults write com.apple.finder AppleShowAllFiles -bool true && killall Finder
```

## 关闭 .DS_Store 在网络盘上的生成

```sh
defaults write com.apple.desktopservices DSDontWriteNetworkStores -bool true
```

## 递归删除所有 .DS_Store

```sh @confirm
find . -name '.DS_Store' -type f -delete
```

## 用 Finder 打开当前目录

```sh
open .
```

## 用默认程序打开文件

```sh
open {{file}}
```

## 用指定应用打开

```sh
open -a "{{app}}" {{file}}
```

## 把命令输出复制到剪贴板

```sh
{{command}} | pbcopy
```

## 把剪贴板内容写到文件

```sh
pbpaste > {{file}}
```

## 查看系统版本和芯片

```sh
sw_vers && uname -m
```

## 查看硬件概况

```sh
system_profiler SPHardwareDataType
```

## 查看电池健康度

```sh
system_profiler SPPowerDataType | grep -A3 "Health Information"
```

## 阻止系统休眠

跑长任务时挂着，Ctrl+C 结束。

```sh
caffeinate -dims
```

## 强制清空 DNS 缓存

改了 /etc/hosts 不生效时用。

```sh @confirm
sudo dscacheutil -flushcache && sudo killall -HUP mDNSResponder
```

## 查看当前 Wi-Fi 密码

```sh @confirm
security find-generic-password -ga "{{ssid}}" 2>&1 | grep password
```

## 重建 Spotlight 索引

搜索不出东西时用，会跑很久。

```sh @confirm
sudo mdutil -E /
```

## 安装 Xcode 命令行工具

git、clang、make 都靠它。

```sh
xcode-select --install
```

## 切换 Xcode 版本

装了多个 Xcode 时用。

```sh @confirm
sudo xcode-select -s {{path}}
```

## 查看代码签名信息

```sh
codesign -dv --verbose=4 "{{app}}"
```

## 压缩成 zip（排除 macOS 元数据）

发给 Windows 用户时避免出现一堆 __MACOSX。

```sh
zip -r -X {{out}}.zip {{dir}}
```

## 显示目录树

```sh
find {{dir}} -maxdepth 2 -not -path '*/.*' | sed -e 's;[^/]*/;|____;g;s;____|; |;g'
```
