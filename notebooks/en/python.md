---
name: python
description: Python - virtual environments, pip, uv, debugging
tags: [python]
---

## Create a virtual environment

```sh @tags=venv
python -m venv .venv
```

## Activate the virtual environment (PowerShell)

If it refuses with a script execution error, run Set-ExecutionPolicy -Scope CurrentUser RemoteSigned first.

```ps1 @platform=windows @tags=venv
.\.venv\Scripts\Activate.ps1
```

## Activate the virtual environment (bash)

```sh @platform=linux,macos @tags=venv
source .venv/bin/activate
```

## Leave the virtual environment

```sh @tags=venv
deactivate
```

## Install dependencies

```sh @tags=pip
pip install -r requirements.txt
```

## Install a single package

```sh @tags=pip
pip install {{package}}
```

## Freeze the current environment

This writes out every transitive dependency too, which is usually not what you want.

```sh @tags=pip
pip freeze > requirements.txt
```

## Install the local project in editable mode

```sh @tags=pip
pip install -e .
```

## Upgrade pip itself

```sh @tags=pip
python -m pip install --upgrade pip
```

## Show where a package is installed and which version

```sh @tags=pip
pip show {{package}}
```

## List packages with newer versions

```sh @tags=pip
pip list --outdated
```

## Switch to a China mirror

```sh @tags=china
pip config set global.index-url https://pypi.tuna.tsinghua.edu.cn/simple
```

## Use a mirror for one install only

```sh @tags=china
pip install {{package}} -i https://pypi.tuna.tsinghua.edu.cn/simple
```

## Create an environment and install with uv

uv is an order of magnitude faster than pip; worth it on new projects.

```sh @tags=uv
uv venv && uv pip install -r requirements.txt
```

## Run a script directly with uv

```sh @tags=uv
uv run {{file}}
```

## Add a dependency with uv

```sh @tags=uv
uv add {{package}}
```

## Serve the current directory over HTTP

Handy for sharing a file quickly or poking at a frontend.

```sh
python -m http.server {{port=8000}}
```

## Pretty-print JSON

```sh
python -m json.tool {{file}}
```

## Drop into the debugger

Put this line in your code and execution stops when it gets there.

```py @tags=reference
breakpoint()
```

## Run the tests

```sh @tags=test
pytest -v
```

## Run only tests matching a name

```sh @tags=test
pytest -k "{{keyword}}" -v
```

## Run the tests with coverage

```sh @tags=test
pytest --cov={{package}} --cov-report=term-missing
```

## Stop at the first failure

```sh @tags=test
pytest -x
```

## Format and auto-fix lint

```sh
ruff format . && ruff check --fix .
```

## Show which Python interpreter is in use

Confirms which environment you are actually in.

```ps1 @platform=windows
(Get-Command python).Source
```

## Show which Python interpreter is in use (bash)

```sh @platform=linux,macos
which -a python python3
```
