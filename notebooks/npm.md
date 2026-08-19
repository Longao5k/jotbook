---
name: npm
description: npm / pnpm / yarn —— 依赖、脚本、发布、镜像源
tags: [node, frontend]
vars:
  script:
    desc: package.json 里的脚本
    from: shell
    cmd: node -p "Object.keys(require('./package.json').scripts||{}).join('\n')"
  level:
    desc: 版本级别
    from: ask
    options: ["patch", "minor", "major"]
---

## 安装依赖

```sh @tags=daily
npm install
```

## 严格按 lock 文件安装

CI 里应该永远用这个，它不会修改 lock 文件，也快得多。

```sh @tags=daily
npm ci
```

## 跑一个脚本

```sh @tags=daily
npm run {{script}}
```

## 列出所有可用脚本

```sh
npm run
```

## 添加依赖

```sh
npm i {{package}}
```

## 添加开发依赖

```sh
npm i -D {{package}}
```

## 添加指定版本

```sh
npm i {{package}}@{{version}}
```

## 全局安装

```sh
npm i -g {{package}}
```

## 移除依赖

```sh
npm uninstall {{package}}
```

## 查看哪些依赖有新版本

```sh @tags=upgrade
npm outdated
```

## 在 semver 范围内升级

不会跨大版本，相对安全。

```sh @tags=upgrade
npm update
```

## 升级到最新的大版本

需要先装 npm-check-updates。会破坏兼容性，升完必须跑测试。

```sh @tags=upgrade @confirm
npx npm-check-updates -u && npm install
```

## 安全审计

```sh @tags=audit
npm audit
```

## 自动修复安全问题

```sh @tags=audit
npm audit fix
```

## 查看某个包的所有版本

```sh
npm view {{package}} versions --json
```

## 查看某个包的最新版本和依赖

```sh
npm view {{package}}
```

## 查看某个依赖为什么被安装进来

排查幽灵依赖、版本冲突。

```sh
npm ls {{package}}
```

## 列出全局安装的包

```sh
npm ls -g --depth=0
```

## 清空 npm 缓存

安装报奇怪的完整性校验错误时用。

```sh @confirm
npm cache clean --force
```

## 彻底重装依赖（PowerShell）

```ps1 @platform=windows @confirm
Remove-Item -Recurse -Force node_modules,package-lock.json -ErrorAction SilentlyContinue; npm install
```

## 彻底重装依赖（bash）

```sh @platform=linux,macos @confirm
rm -rf node_modules package-lock.json && npm install
```

## 切换到国内镜像源

```sh @tags=china
npm config set registry https://registry.npmmirror.com
```

## 切回官方源

```sh @tags=china
npm config set registry https://registry.npmjs.org
```

## 查看当前使用的源

```sh @tags=china
npm config get registry
```

## 只为某次安装临时用镜像

```sh @tags=china
npm i {{package}} --registry=https://registry.npmmirror.com
```

## 查看所有 npm 配置

```sh
npm config list
```

## 不安装直接执行一个包

```sh
npx {{package}}
```

## 递增版本号并打 git tag

```sh @tags=publish
npm version {{level}}
```

## 发布到 npm

```sh @tags=publish @confirm
npm publish --access public
```

## 发布前预览会打包哪些文件

避免把源码或密钥误传上去。

```sh @tags=publish
npm pack --dry-run
```

## 登录 npm

```sh @tags=publish
npm login
```

## 初始化 package.json

```sh
npm init -y
```

## 查看包安装后的体积

```sh
npx cost-of-modules
```

## pnpm：安装依赖

```sh @tags=pnpm
pnpm install
```

## pnpm：添加依赖

```sh @tags=pnpm
pnpm add {{package}}
```

## pnpm：在某个 workspace 包里安装

monorepo 常用。

```sh @tags=pnpm
pnpm --filter {{workspace}} add {{package}}
```

## pnpm：在所有 workspace 里跑脚本

```sh @tags=pnpm
pnpm -r run {{script}}
```

## yarn：安装依赖

```sh @tags=yarn
yarn install --frozen-lockfile
```

## 查看 node 和 npm 版本

```sh
node -v && npm -v
```

## 用 nvm 切换 node 版本

```sh @platform=linux,macos
nvm use {{version}}
```

## 用 nvm-windows 切换 node 版本

```ps1 @platform=windows
nvm use {{version}}
```
