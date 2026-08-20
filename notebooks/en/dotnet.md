---
name: dotnet
description: .NET - running, publishing, EF Core migrations, user secrets
tags: [backend, csharp]
vars:
  env:
    desc: environment
    from: ask
    options: ["Development", "Staging", "Production"]
  rid:
    desc: runtime identifier
    from: ask
    options: ["win-x64", "linux-x64", "linux-arm64", "osx-arm64"]
  project:
    desc: project file
    from: shell
    cmd: git ls-files "*.csproj"
---

## Run the project

```sh
dotnet run
```

## Run against a specific environment

```sh
dotnet run --environment {{env}}
```

## Run on a specific port

```sh
dotnet run --urls "http://localhost:{{port}}"
```

## Develop with hot reload

Recompiles and restarts on every edit. Leave it running while writing an API.

```sh
dotnet watch run
```

## Build in Release

```sh @tags=build
dotnet build -c Release
```

## Publish to a directory

```sh @tags=build
dotnet publish -c Release -o {{out}}
```

## Publish as a self-contained single file

The target machine needs no .NET runtime; copy it across and it runs.

```sh @tags=build
dotnet publish -c Release -r {{rid}} --self-contained true -p:PublishSingleFile=true
```

## Publish as a framework-dependent single file

Much smaller, but the target machine needs the matching runtime.

```sh @tags=build
dotnet publish -c Release -r {{rid}} --self-contained false -p:PublishSingleFile=true
```

## Run the tests

```sh @tags=test
dotnet test
```

## Run the tests with detailed output

```sh @tags=test
dotnet test --logger "console;verbosity=detailed"
```

## Run only tests matching a name

```sh @tags=test
dotnet test --filter "FullyQualifiedName~{{keyword}}"
```

## Run the tests with coverage

```sh @tags=test
dotnet test --collect:"XPlat Code Coverage"
```

## Add a NuGet package

```sh
dotnet add package {{package}}
```

## Add a specific version of a package

```sh
dotnet add package {{package}} --version {{version}}
```

## Remove a package

```sh
dotnet remove package {{package}}
```

## Restore dependencies

```sh
dotnet restore
```

## Clean build output

```sh
dotnet clean
```

## List installed SDKs and runtimes

```sh
dotnet --list-sdks && dotnet --list-runtimes
```

## List dependencies with known vulnerabilities

```sh
dotnet list package --vulnerable --include-transitive
```

## List outdated dependencies

```sh
dotnet list package --outdated
```

## Add an EF Core migration

```sh @tags=ef
dotnet ef migrations add {{name}}
```

## Apply migrations to the database

```sh @tags=ef
dotnet ef database update
```

## Roll back to a specific migration

Pass 0 to roll back every migration.

```sh @tags=ef @confirm
dotnet ef database update {{migration}}
```

## Remove the last unapplied migration

This will not work once it has been applied; roll back first.

```sh @tags=ef
dotnet ef migrations remove
```

## List every migration and its status

```sh @tags=ef
dotnet ef migrations list
```

## Export migrations as a SQL script

Production usually forbids running update directly; you hand this script to the DBA instead.

```sh @tags=ef
dotnet ef migrations script --idempotent -o {{file}}
```

## Scaffold entities from an existing database

```sh @tags=ef
dotnet ef dbcontext scaffold "{{connstr}}" Microsoft.EntityFrameworkCore.SqlServer -o Models
```

## Run an EF command against a separate startup project

Required when the DbContext and the startup project live in different csproj files.

```sh @tags=ef
dotnet ef database update --project {{project}} --startup-project {{startup}}
```

## Set a user secret

Connection strings and keys do not belong in appsettings.json.

```sh @tags=secrets
dotnet user-secrets set "{{key}}" "{{value}}"
```

## Initialise user secrets

```sh @tags=secrets
dotnet user-secrets init
```

## List every user secret

```sh @tags=secrets
dotnet user-secrets list
```

## Install a global tool

```sh
dotnet tool install -g {{tool}}
```

## Update a global tool

```sh
dotnet tool update -g {{tool}}
```

## Install the EF command-line tool

Needed the first time dotnet ef reports that the command does not exist.

```sh @tags=ef
dotnet tool install --global dotnet-ef
```

## List installed global tools

```sh
dotnet tool list -g
```

## Format the code

```sh
dotnet format
```

## Create a Web API project

```sh
dotnet new webapi -n {{name}}
```

## Create a solution and add a project to it

```sh
dotnet new sln -n {{name}} && dotnet sln add {{project}}
```

## Trust the local HTTPS development certificate

For certificate errors when running HTTPS locally.

```sh
dotnet dev-certs https --trust
```
