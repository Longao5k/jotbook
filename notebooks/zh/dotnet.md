---
name: dotnet
description: .NET —— 运行、发布、EF Core 迁移、用户机密
tags: [backend, csharp]
vars:
  env:
    desc: 运行环境
    from: ask
    options: ["Development", "Staging", "Production"]
  rid:
    desc: 运行时标识
    from: ask
    options: ["win-x64", "linux-x64", "linux-arm64", "osx-arm64"]
  project:
    desc: 项目文件
    from: shell
    cmd: git ls-files "*.csproj"
---

## 运行项目

```sh
dotnet run
```

## 指定环境运行

```sh
dotnet run --environment {{env}}
```

## 指定端口运行

```sh
dotnet run --urls "http://localhost:{{port}}"
```

## 热重载开发

改代码自动重编译并重启，写 API 时应该常驻。

```sh
dotnet watch run
```

## 构建 Release

```sh @tags=build
dotnet build -c Release
```

## 发布到目录

```sh @tags=build
dotnet publish -c Release -o {{out}}
```

## 发布为自包含单文件

目标机器不用装 .NET 运行时，扔上去就能跑。

```sh @tags=build
dotnet publish -c Release -r {{rid}} --self-contained true -p:PublishSingleFile=true
```

## 发布为依赖框架的单文件

体积小很多，但目标机器要有对应版本的运行时。

```sh @tags=build
dotnet publish -c Release -r {{rid}} --self-contained false -p:PublishSingleFile=true
```

## 跑测试

```sh @tags=test
dotnet test
```

## 跑测试并输出详细日志

```sh @tags=test
dotnet test --logger "console;verbosity=detailed"
```

## 只跑匹配名字的测试

```sh @tags=test
dotnet test --filter "FullyQualifiedName~{{keyword}}"
```

## 跑测试并生成覆盖率

```sh @tags=test
dotnet test --collect:"XPlat Code Coverage"
```

## 添加 NuGet 包

```sh
dotnet add package {{package}}
```

## 添加指定版本的包

```sh
dotnet add package {{package}} --version {{version}}
```

## 移除包

```sh
dotnet remove package {{package}}
```

## 还原依赖

```sh
dotnet restore
```

## 清理构建产物

```sh
dotnet clean
```

## 列出已安装的 SDK 和运行时

```sh
dotnet --list-sdks && dotnet --list-runtimes
```

## 查看有漏洞的依赖

```sh
dotnet list package --vulnerable --include-transitive
```

## 查看过期的依赖

```sh
dotnet list package --outdated
```

## 新增 EF Core 迁移

```sh @tags=ef
dotnet ef migrations add {{name}}
```

## 应用迁移到数据库

```sh @tags=ef
dotnet ef database update
```

## 回滚到指定迁移

填 0 表示回滚全部迁移。

```sh @tags=ef @confirm
dotnet ef database update {{migration}}
```

## 删除最后一个还没应用的迁移

已经 update 过就不能这样删，要先回滚。

```sh @tags=ef
dotnet ef migrations remove
```

## 列出所有迁移及其状态

```sh @tags=ef
dotnet ef migrations list
```

## 把迁移导出成 SQL 脚本

生产环境通常不允许直接跑 update，而是交付这个脚本给 DBA。

```sh @tags=ef
dotnet ef migrations script --idempotent -o {{file}}
```

## 从已有数据库反向生成实体

```sh @tags=ef
dotnet ef dbcontext scaffold "{{connstr}}" Microsoft.EntityFrameworkCore.SqlServer -o Models
```

## 指定启动项目跑 EF 命令

DbContext 和启动项目不在同一个 csproj 时必须这样写。

```sh @tags=ef
dotnet ef database update --project {{project}} --startup-project {{startup}}
```

## 设置用户机密

连接串、密钥不要写进 appsettings.json。

```sh @tags=secrets
dotnet user-secrets set "{{key}}" "{{value}}"
```

## 初始化用户机密

```sh @tags=secrets
dotnet user-secrets init
```

## 列出所有用户机密

```sh @tags=secrets
dotnet user-secrets list
```

## 安装全局工具

```sh
dotnet tool install -g {{tool}}
```

## 更新全局工具

```sh
dotnet tool update -g {{tool}}
```

## 安装 EF 命令行工具

第一次跑 dotnet ef 报「找不到命令」时需要。

```sh @tags=ef
dotnet tool install --global dotnet-ef
```

## 列出已安装的全局工具

```sh
dotnet tool list -g
```

## 格式化代码

```sh
dotnet format
```

## 新建 Web API 项目

```sh
dotnet new webapi -n {{name}}
```

## 新建解决方案并加入项目

```sh
dotnet new sln -n {{name}} && dotnet sln add {{project}}
```

## 生成开发用 HTTPS 证书

本地跑 HTTPS 报证书错误时用。

```sh
dotnet dev-certs https --trust
```
