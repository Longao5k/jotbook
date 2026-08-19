---
name: kubectl
description: Kubernetes —— Pod 排查、日志、转发、伸缩
tags: [ops, k8s]
vars:
  ns:
    desc: 命名空间
    from: profile
    cmd: kubectl get ns -o name
  pod:
    desc: Pod
    from: shell
    cmd: kubectl get pods -o name
  deploy:
    desc: Deployment
    from: shell
    cmd: kubectl get deploy -o name
---

## 列出当前命名空间的 Pod

```sh @tags=daily
kubectl get pods -o wide
```

## 列出所有命名空间的 Pod

```sh @tags=daily
kubectl get pods -A -o wide
```

## 只看不正常的 Pod

```sh @tags=daily
kubectl get pods -A --field-selector=status.phase!=Running
```

## 查看 Pod 的详细状态和事件

Pod 起不来时第一条命令，事件在输出最下面。

```sh @tags=debug
kubectl describe {{pod}} -n {{ns}}
```

## 查看 Pod 日志

```sh @tags=logs
kubectl logs {{pod}} -n {{ns}} --tail 200
```

## 实时跟踪 Pod 日志

```sh @tags=logs @remote
kubectl logs -f {{pod}} -n {{ns}} --tail 100
```

## 查看上一个已崩溃容器的日志

CrashLoopBackOff 时真正有用的日志在这里。

```sh @tags=logs @tags=debug
kubectl logs {{pod}} -n {{ns}} --previous
```

## 按标签查看多个 Pod 的日志

```sh @tags=logs
kubectl logs -l app={{app}} -n {{ns}} --tail 100 --max-log-requests 10
```

## 进入 Pod

```sh @tags=debug
kubectl exec -it {{pod}} -n {{ns}} -- sh
```

## 在 Pod 里执行一条命令

```sh @tags=debug
kubectl exec {{pod}} -n {{ns}} -- {{command}}
```

## 把 Pod 端口转发到本地

访问集群内部服务、连数据库调试都靠它。

```sh @tags=debug
kubectl port-forward {{pod}} {{localport}}:{{remoteport}} -n {{ns}}
```

## 把 Service 端口转发到本地

```sh @tags=debug
kubectl port-forward svc/{{service}} {{localport}}:{{remoteport}} -n {{ns}}
```

## 查看集群近期事件

按时间排序，排查调度失败、拉镜像失败。

```sh @tags=debug
kubectl get events -n {{ns}} --sort-by=.lastTimestamp | tail -30
```

## 查看节点资源占用

```sh
kubectl top nodes
```

## 查看 Pod 资源占用

```sh
kubectl top pods -n {{ns}} --sort-by=memory
```

## 重启一个 Deployment

滚动重启，不改任何配置，是最干净的「重启大法」。

```sh @confirm
kubectl rollout restart {{deploy}} -n {{ns}}
```

## 查看滚动更新状态

```sh
kubectl rollout status {{deploy}} -n {{ns}}
```

## 回滚到上一个版本

```sh @confirm
kubectl rollout undo {{deploy}} -n {{ns}}
```

## 查看发布历史

```sh
kubectl rollout history {{deploy}} -n {{ns}}
```

## 调整副本数

```sh @confirm
kubectl scale {{deploy}} --replicas={{n}} -n {{ns}}
```

## 修改镜像版本

```sh @confirm
kubectl set image {{deploy}} {{container}}={{image}} -n {{ns}}
```

## 删除 Pod 让它重建

```sh @confirm
kubectl delete {{pod}} -n {{ns}}
```

## 应用配置文件

```sh @confirm
kubectl apply -f {{file}}
```

## 预览应用配置会改什么

生产环境应该先跑这个。

```sh
kubectl diff -f {{file}}
```

## 导出资源的完整 YAML

```sh
kubectl get {{deploy}} -n {{ns}} -o yaml
```

## 查看 ConfigMap 内容

```sh
kubectl get configmap {{name}} -n {{ns}} -o jsonpath='{.data}'
```

## 解码 Secret

Secret 是 base64 编码不是加密，任何有权限的人都能看。

```sh @confirm
kubectl get secret {{name}} -n {{ns}} -o go-template='{{range $k,$v := .data}}{{$k}}={{$v|base64decode}}{{"\n"}}{{end}}'
```

## 切换默认命名空间

省掉每条命令都写 -n。

```sh
kubectl config set-context --current --namespace={{ns}}
```

## 查看当前上下文

```sh
kubectl config current-context
```

## 切换集群上下文

```sh @confirm
kubectl config use-context {{context}}
```

## 起一个临时调试 Pod

集群内网络排查神器。

```sh @tags=debug
kubectl run tmp-debug --rm -it --image=nicolaka/netshoot -n {{ns}} -- bash
```
