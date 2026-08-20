---
name: macos
description: macOS - Homebrew, ports, permissions, Finder and system settings
tags: [macos, shell]
platform: [macos]
---

## Find what is using a port

```sh @tags=daily @tags=port
sudo lsof -nP -iTCP:{{port}} -sTCP:LISTEN
```

## Kill whatever is holding a port

```sh @tags=port @confirm
sudo kill -9 $(sudo lsof -t -iTCP:{{port}} -sTCP:LISTEN)
```

## List every listening port

```sh @tags=port
sudo lsof -nP -iTCP -sTCP:LISTEN
```

## Homebrew: install

```sh @tags=brew
brew install {{package}}
```

## Homebrew: install a GUI app

```sh @tags=brew
brew install --cask {{app}}
```

## Homebrew: search

```sh @tags=brew
brew search {{keyword}}
```

## Homebrew: upgrade everything

```sh @tags=brew @confirm
brew update && brew upgrade
```

## Homebrew: clean out old versions

brew keeps every past version, which reaches tens of gigabytes given time.

```sh @tags=brew @confirm
brew cleanup --prune=all
```

## Homebrew: see what is using the space

```sh @tags=brew
brew list --formula | xargs -n1 -I{} sh -c 'echo "$(du -sh $(brew --cellar {}) 2>/dev/null | cut -f1)\t{}"' | sort -hr | head -20
```

## Homebrew: list services

Databases, redis and anything else started via brew services.

```sh @tags=brew
brew services list
```

## Homebrew: restart a service

```sh @tags=brew @confirm
brew services restart {{service}}
```

## Homebrew: use a China mirror

```sh @tags=brew @tags=china
export HOMEBREW_API_DOMAIN="https://mirrors.tuna.tsinghua.edu.cn/homebrew-bottles/api"
export HOMEBREW_BOTTLE_DOMAIN="https://mirrors.tuna.tsinghua.edu.cn/homebrew-bottles"
```

## Clear the "app is damaged and cannot be opened" block

Gatekeeper blocks unsigned apps downloaded from the web.

```sh @confirm
sudo xattr -dr com.apple.quarantine "{{app}}"
```

## Show a file's extended attributes

```sh
xattr -l {{file}}
```

## Show hidden files in Finder

```sh
defaults write com.apple.finder AppleShowAllFiles -bool true && killall Finder
```

## Stop .DS_Store being created on network shares

```sh
defaults write com.apple.desktopservices DSDontWriteNetworkStores -bool true
```

## Delete every .DS_Store recursively

```sh @confirm
find . -name '.DS_Store' -type f -delete
```

## Open the current directory in Finder

```sh
open .
```

## Open a file with its default application

```sh
open {{file}}
```

## Open a file with a specific application

```sh
open -a "{{app}}" {{file}}
```

## Copy a command's output to the clipboard

```sh
{{command}} | pbcopy
```

## Write the clipboard to a file

```sh
pbpaste > {{file}}
```

## Show the OS version and chip

```sh
sw_vers && uname -m
```

## Show a hardware overview

```sh
system_profiler SPHardwareDataType
```

## Check battery health

```sh
system_profiler SPPowerDataType | grep -A3 "Health Information"
```

## Keep the machine awake

Leave it running during a long job; Ctrl+C to stop.

```sh
caffeinate -dims
```

## Force-flush the DNS cache

For when an edit to /etc/hosts does not take effect.

```sh @confirm
sudo dscacheutil -flushcache && sudo killall -HUP mDNSResponder
```

## Show the current Wi-Fi password

```sh @confirm
security find-generic-password -ga "{{ssid}}" 2>&1 | grep password
```

## Rebuild the Spotlight index

For when search finds nothing. It takes a long while.

```sh @confirm
sudo mdutil -E /
```

## Install the Xcode command line tools

git, clang and make all come from here.

```sh
xcode-select --install
```

## Switch Xcode version

For when several Xcodes are installed.

```sh @confirm
sudo xcode-select -s {{path}}
```

## Show code signing information

```sh
codesign -dv --verbose=4 "{{app}}"
```

## Create a zip without macOS metadata

Avoids the pile of __MACOSX entries when sending it to a Windows user.

```sh
zip -r -X {{out}}.zip {{dir}}
```

## Print a directory tree

```sh
find {{dir}} -maxdepth 2 -not -path '*/.*' | sed -e 's;[^/]*/;|____;g;s;____|; |;g'
```
