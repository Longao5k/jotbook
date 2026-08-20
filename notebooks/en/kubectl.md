---
name: kubectl
description: Kubernetes - debugging pods, logs, port forwarding, scaling
tags: [ops, k8s]
vars:
  ns:
    desc: namespace
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

## List pods in the current namespace

```sh @tags=daily
kubectl get pods -o wide
```

## List pods across every namespace

```sh @tags=daily
kubectl get pods -A -o wide
```

## Show only unhealthy pods

```sh @tags=daily
kubectl get pods -A --field-selector=status.phase!=Running
```

## Show a pod's detail and events

The first command when a pod will not start; the events are at the bottom of the output.

```sh @tags=debug
kubectl describe {{pod}} -n {{ns}}
```

## Show a pod's logs

```sh @tags=logs
kubectl logs {{pod}} -n {{ns}} --tail 200
```

## Follow a pod's logs

```sh @tags=logs @remote
kubectl logs -f {{pod}} -n {{ns}} --tail 100
```

## Show the logs of the previous, crashed container

This is where the useful logs are during CrashLoopBackOff.

```sh @tags=logs @tags=debug
kubectl logs {{pod}} -n {{ns}} --previous
```

## Show logs from several pods by label

```sh @tags=logs
kubectl logs -l app={{app}} -n {{ns}} --tail 100 --max-log-requests 10
```

## Get a shell in a pod

```sh @tags=debug
kubectl exec -it {{pod}} -n {{ns}} -- sh
```

## Run one command in a pod

```sh @tags=debug
kubectl exec {{pod}} -n {{ns}} -- {{command}}
```

## Forward a pod's port to your machine

How you reach in-cluster services and connect to databases while debugging.

```sh @tags=debug
kubectl port-forward {{pod}} {{localport}}:{{remoteport}} -n {{ns}}
```

## Forward a service's port to your machine

```sh @tags=debug
kubectl port-forward svc/{{service}} {{localport}}:{{remoteport}} -n {{ns}}
```

## Show recent cluster events

Sorted by time; this is where scheduling and image pull failures show up.

```sh @tags=debug
kubectl get events -n {{ns}} --sort-by=.lastTimestamp | tail -30
```

## Show node resource usage

```sh
kubectl top nodes
```

## Show pod resource usage

```sh
kubectl top pods -n {{ns}} --sort-by=memory
```

## Restart a deployment

A rolling restart that changes no configuration, and the cleanest way to "just restart it".

```sh @confirm
kubectl rollout restart {{deploy}} -n {{ns}}
```

## Watch a rollout's progress

```sh
kubectl rollout status {{deploy}} -n {{ns}}
```

## Roll back to the previous version

```sh @confirm
kubectl rollout undo {{deploy}} -n {{ns}}
```

## Show the rollout history

```sh
kubectl rollout history {{deploy}} -n {{ns}}
```

## Change the replica count

```sh @confirm
kubectl scale {{deploy}} --replicas={{n}} -n {{ns}}
```

## Change the image version

```sh @confirm
kubectl set image {{deploy}} {{container}}={{image}} -n {{ns}}
```

## Delete a pod so it is recreated

```sh @confirm
kubectl delete {{pod}} -n {{ns}}
```

## Apply a manifest

```sh @confirm
kubectl apply -f {{file}}
```

## Preview what applying a manifest would change

Run this first in production.

```sh
kubectl diff -f {{file}}
```

## Dump a resource's full YAML

```sh
kubectl get {{deploy}} -n {{ns}} -o yaml
```

## Show a ConfigMap's contents

```sh
kubectl get configmap {{name}} -n {{ns}} -o jsonpath='{.data}'
```

## Decode a Secret

A Secret is base64 encoded, not encrypted; anyone with access can read it.

```sh @confirm
kubectl get secret {{name}} -n {{ns}} -o go-template='{{range $k,$v := .data}}{{$k}}={{$v|base64decode}}{{"\n"}}{{end}}'
```

## Change the default namespace

Saves writing -n on every command.

```sh
kubectl config set-context --current --namespace={{ns}}
```

## Show the current context

```sh
kubectl config current-context
```

## Switch cluster context

```sh @confirm
kubectl config use-context {{context}}
```

## Start a throwaway debug pod

Invaluable for network debugging inside a cluster.

```sh @tags=debug
kubectl run tmp-debug --rm -it --image=nicolaka/netshoot -n {{ns}} -- bash
```
