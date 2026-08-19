---
name: python
description: Python —— 虚拟环境、pip、uv、调试
tags: [python]
---

## 创建虚拟环境

```sh @tags=venv
python -m venv .venv
```

## 激活虚拟环境（PowerShell）

报「禁止运行脚本」时先执行 Set-ExecutionPolicy -Scope CurrentUser RemoteSigned。

```ps1 @platform=windows @tags=venv
.\.venv\Scripts\Activate.ps1
```

## 激活虚拟环境（bash）

```sh @platform=linux,macos @tags=venv
source .venv/bin/activate
```

## 退出虚拟环境

```sh @tags=venv
deactivate
```

## 安装依赖

```sh @tags=pip
pip install -r requirements.txt
```

## 安装单个包

```sh @tags=pip
pip install {{package}}
```

## 导出当前环境的依赖

会把所有间接依赖也写进去，通常不是你想要的。

```sh @tags=pip
pip freeze > requirements.txt
```

## 以可编辑模式安装本地项目

```sh @tags=pip
pip install -e .
```

## 升级 pip 自己

```sh @tags=pip
python -m pip install --upgrade pip
```

## 查看某个包装在哪、什么版本

```sh @tags=pip
pip show {{package}}
```

## 列出可升级的包

```sh @tags=pip
pip list --outdated
```

## 切换到国内镜像源

```sh @tags=china
pip config set global.index-url https://pypi.tuna.tsinghua.edu.cn/simple
```

## 只为这次安装用镜像

```sh @tags=china
pip install {{package}} -i https://pypi.tuna.tsinghua.edu.cn/simple
```

## 用 uv 创建环境并装依赖

uv 比 pip 快一个数量级，新项目推荐。

```sh @tags=uv
uv venv && uv pip install -r requirements.txt
```

## uv 直接运行脚本

```sh @tags=uv
uv run {{file}}
```

## uv 添加依赖

```sh @tags=uv
uv add {{package}}
```

## 起一个静态文件服务器

临时分享文件、调试前端很方便。

```sh
python -m http.server {{port=8000}}
```

## 格式化 JSON

```sh
python -m json.tool {{file}}
```

## 进入调试器

在代码里插一行，跑到这就会停下。

```py @tags=reference
breakpoint()
```

## 跑测试

```sh @tags=test
pytest -v
```

## 只跑匹配名字的测试

```sh @tags=test
pytest -k "{{keyword}}" -v
```

## 跑测试并显示覆盖率

```sh @tags=test
pytest --cov={{package}} --cov-report=term-missing
```

## 第一个失败就停

```sh @tags=test
pytest -x
```

## 格式化并修 lint

```sh
ruff format . && ruff check --fix .
```

## 查看 Python 解释器路径

确认自己到底用的是哪个环境。

```ps1 @platform=windows
(Get-Command python).Source
```

## 查看 Python 解释器路径（bash）

```sh @platform=linux,macos
which -a python python3
```
