---
name: npm
description: npm / pnpm / yarn - dependencies, scripts, publishing, registries
tags: [node, frontend]
vars:
  script:
    desc: script from package.json
    from: shell
    cmd: node -p "Object.keys(require('./package.json').scripts||{}).join('\n')"
  level:
    desc: version bump level
    from: ask
    options: ["patch", "minor", "major"]
---

## Install dependencies

```sh @tags=daily
npm install
```

## Install strictly from the lock file

Always use this in CI: it never rewrites the lock file, and it is much faster.

```sh @tags=daily
npm ci
```

## Run a script

```sh @tags=daily
npm run {{script}}
```

## List the available scripts

```sh
npm run
```

## Add a dependency

```sh
npm i {{package}}
```

## Add a dev dependency

```sh
npm i -D {{package}}
```

## Add a specific version

```sh
npm i {{package}}@{{version}}
```

## Install globally

```sh
npm i -g {{package}}
```

## Remove a dependency

```sh
npm uninstall {{package}}
```

## See which dependencies have newer versions

```sh @tags=upgrade
npm outdated
```

## Upgrade within the semver ranges

Will not cross a major version, so it is relatively safe.

```sh @tags=upgrade
npm update
```

## Upgrade to the latest major versions

Needs npm-check-updates installed. This breaks compatibility, so run the tests afterwards.

```sh @tags=upgrade @confirm
npx npm-check-updates -u && npm install
```

## Audit for vulnerabilities

```sh @tags=audit
npm audit
```

## Fix vulnerabilities automatically

```sh @tags=audit
npm audit fix
```

## List every published version of a package

```sh
npm view {{package}} versions --json
```

## Show a package's latest version and dependencies

```sh
npm view {{package}}
```

## Find out why a dependency was installed

For chasing phantom dependencies and version conflicts.

```sh
npm ls {{package}}
```

## List globally installed packages

```sh
npm ls -g --depth=0
```

## Clear the npm cache

For when an install fails with a strange integrity error.

```sh @confirm
npm cache clean --force
```

## Reinstall dependencies from scratch (PowerShell)

```ps1 @platform=windows @confirm
Remove-Item -Recurse -Force node_modules,package-lock.json -ErrorAction SilentlyContinue; npm install
```

## Reinstall dependencies from scratch (bash)

```sh @platform=linux,macos @confirm
rm -rf node_modules package-lock.json && npm install
```

## Switch to a China mirror

```sh @tags=china
npm config set registry https://registry.npmmirror.com
```

## Switch back to the official registry

```sh @tags=china
npm config set registry https://registry.npmjs.org
```

## Show the registry in use

```sh @tags=china
npm config get registry
```

## Use a mirror for one install only

```sh @tags=china
npm i {{package}} --registry=https://registry.npmmirror.com
```

## Show the full npm configuration

```sh
npm config list
```

## Run a package without installing it

```sh
npx {{package}}
```

## Bump the version and create a git tag

```sh @tags=publish
npm version {{level}}
```

## Publish to npm

```sh @tags=publish @confirm
npm publish --access public
```

## Preview which files would be published

Stops you shipping source or secrets by accident.

```sh @tags=publish
npm pack --dry-run
```

## Log in to npm

```sh @tags=publish
npm login
```

## Create a package.json

```sh
npm init -y
```

## See how much space each dependency takes

```sh
npx cost-of-modules
```

## pnpm: install dependencies

```sh @tags=pnpm
pnpm install
```

## pnpm: add a dependency

```sh @tags=pnpm
pnpm add {{package}}
```

## pnpm: install into one workspace package

The everyday monorepo case.

```sh @tags=pnpm
pnpm --filter {{workspace}} add {{package}}
```

## pnpm: run a script across every workspace

```sh @tags=pnpm
pnpm -r run {{script}}
```

## yarn: install dependencies

```sh @tags=yarn
yarn install --frozen-lockfile
```

## Show the node and npm versions

```sh
node -v && npm -v
```

## Switch node version with nvm

```sh @platform=linux,macos
nvm use {{version}}
```

## Switch node version with nvm-windows

```ps1 @platform=windows
nvm use {{version}}
```
