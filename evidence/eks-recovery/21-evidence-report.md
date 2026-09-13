# FleetForge evidence report — run-20260913T141547Z

**Run:** EKS Bottlerocket + Brupop demonstration  
**Cluster:** `1c2cdb4c-c1b2-4cd5-bb54-1132876ab118`  
**Window:** 2026-09-13T14:15:47.607196+00:00 → 2026-09-13T14:46:39.260208+00:00  
**Recorded:** 3908 log entries, 75 distinct snapshots

Every line below cites a `seq` from the event log. Check any of them with:

```bash
jq 'select(.seq == N)' events.jsonl
```

## Prediction versus actual

1 of 5 tested prediction(s) exact, 3 conservative (over-predicted), 1 under-predicted, 0 missed a disrupted workload entirely.

| Predicted at | Nodes | Predicted evictions | Observed | Δ | Verdict |
| --- | --- | --- | --- | --- | --- |
| 14:15:50 | ip-10-42-100-186.us-east-2.compute.internal | 16 | 0 | -16 | untested — no disruption occurred in this window |
| 14:16:20 | ip-10-42-100-186.us-east-2.compute.internal | 16 | 0 | -16 | untested — no disruption occurred in this window |
| 14:17:34 | ip-10-42-100-186.us-east-2.compute.internal | 0 | 0 | +0 | exact |
| 14:18:41 | ip-10-42-100-186.us-east-2.compute.internal | 16 | 0 | -16 | conservative — predicted more disruption than occurred |
| 14:29:21 | ip-10-42-101-194.us-east-2.compute.internal | 5 | 0 | -5 | conservative — predicted more disruption than occurred |
| 14:34:50 | ip-10-42-101-194.us-east-2.compute.internal | 5 | 8 | +3 | under-predicted — more pods were evicted than predicted |
| 14:45:11 | ip-10-42-100-186.us-east-2.compute.internal | 16 | 0 | -16 | conservative — predicted more disruption than occurred |

### What FleetForge got wrong

No workload was disrupted without being predicted.

### Over-prediction

- Prediction at 14:18:41: predicted but not disrupted — brupop-bottlerocket-aws/brupop-agent, brupop-bottlerocket-aws/brupop-apiserver, brupop-bottlerocket-aws/brupop-controller-deployment, cert-manager/cert-manager, cert-manager/cert-manager-cainjector, cert-manager/cert-manager-webhook, demo/api, demo/traffic, demo/web, kube-system/aws-node, kube-system/coredns, kube-system/eks-pod-identity-agent, kube-system/kube-proxy
- Prediction at 14:29:21: predicted but not disrupted — brupop-bottlerocket-aws/brupop-agent, demo/web, kube-system/aws-node, kube-system/eks-pod-identity-agent, kube-system/kube-proxy
- Prediction at 14:45:11: predicted but not disrupted — brupop-bottlerocket-aws/brupop-agent, brupop-bottlerocket-aws/brupop-apiserver, brupop-bottlerocket-aws/brupop-controller-deployment, cert-manager/cert-manager, cert-manager/cert-manager-cainjector, cert-manager/cert-manager-webhook, demo/api, demo/traffic, demo/web, kube-system/aws-node, kube-system/coredns, kube-system/eks-pod-identity-agent, kube-system/kube-proxy

## Timeline

| seq | time | kind | detail |
| --- | --- | --- | --- |
| 1 | 14:15:47 | run | run started: EKS Bottlerocket + Brupop demonstration |
| 31 | 14:15:48 | node | node ip-10-42-101-194.us-east-2.compute.internal: ready=true cordoned=true |
| 32 | 14:15:48 | node | node ip-10-42-101-90.us-east-2.compute.internal: ready=true cordoned=true |
| 68 | 14:15:48 | warning | DaemonSet/brupop-agent: FailedDaemonPod — Found failed daemon pod brupop-bottlerocket-aws/brupop-agent-lg942 on node ip-10-42-100-186.us-east-2.compute.internal, will try to kill it |
| 69 | 14:15:48 | warning | DaemonSet/brupop-agent: FailedDaemonPod — Found failed daemon pod brupop-bottlerocket-aws/brupop-agent-shbfh on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 77 | 14:15:48 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-hvrm9 on node ip-10-42-100-186.us-east-2.compute.internal, will try to kill it |
| 78 | 14:15:48 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-pckkf on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 79 | 14:15:48 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-rmdmg on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 80 | 14:15:48 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-mbtwj on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 81 | 14:15:48 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-lq2hs on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 82 | 14:15:48 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-nblm8 on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 102 | 14:15:48 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-bs9m8 on node ip-10-42-100-186.us-east-2.compute.internal, will try to kill it |
| 103 | 14:15:48 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-5hdct on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 104 | 14:15:48 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-82lwr on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 105 | 14:15:48 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-htd2j on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 106 | 14:15:48 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-m9g9s on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 107 | 14:15:48 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-h8gks on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 108 | 14:15:48 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-c5l9d on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 130 | 14:15:48 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-slv2h on node ip-10-42-100-186.us-east-2.compute.internal, will try to kill it |
| 131 | 14:15:48 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-kbgt8 on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 132 | 14:15:48 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-xmnf4 on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 133 | 14:15:48 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-c7c9c on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 134 | 14:15:48 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-tqjzg on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 135 | 14:15:48 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-w4h6f on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 136 | 14:15:48 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-tnsw7 on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 170 | 14:15:48 | warning | Issuer/brupop-root-certificate-issuer: ErrGetKeyPair — Error getting keypair for CA issuer: secrets "brupop-root-ca-secret" not found |
| 171 | 14:15:48 | warning | Issuer/brupop-root-certificate-issuer: ErrInitIssuer — Error initializing issuer: secrets "brupop-root-ca-secret" not found |
| 185 | 14:15:48 | warning | Node/ip-10-42-100-186.us-east-2.compute.internal: InvalidDiskCapacity — invalid capacity 0 on image filesystem |
| 199 | 14:15:48 | warning | Node/ip-10-42-100-186.us-east-2.compute.internal: Rebooted — Node ip-10-42-100-186.us-east-2.compute.internal has been rebooted, boot id: 6436acb8-a1be-4339-8b6e-b6d04fbfa82c |
| 208 | 14:15:48 | warning | Node/ip-10-42-101-194.us-east-2.compute.internal: InvalidDiskCapacity — invalid capacity 0 on image filesystem |
| 220 | 14:15:48 | warning | Node/ip-10-42-101-90.us-east-2.compute.internal: InvalidDiskCapacity — invalid capacity 0 on image filesystem |
| 221 | 14:15:48 | warning | Node/ip-10-42-101-90.us-east-2.compute.internal: InvalidDiskCapacity — invalid capacity 0 on image filesystem |
| 235 | 14:15:48 | warning | Node/ip-10-42-101-90.us-east-2.compute.internal: Rebooted — Node ip-10-42-101-90.us-east-2.compute.internal has been rebooted, boot id: edad6a4f-2f72-4646-a370-f8ddebc2914f |
| 244 | 14:15:48 | warning | Pod/brupop-agent-9j5vd: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-client-certificate" not found |
| 249 | 14:15:48 | warning | Pod/brupop-agent-lg942: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-client-certificate" not found |
| 255 | 14:15:48 | warning | Pod/brupop-agent-shbfh: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-client-certificate" not found |
| 277 | 14:15:48 | warning | Pod/brupop-apiserver-75bd4784c4-9cc8s: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-certificate" not found |
| 293 | 14:15:48 | warning | Pod/brupop-apiserver-75bd4784c4-rpn4n: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-certificate" not found |
| 299 | 14:15:48 | warning | Pod/brupop-apiserver-75bd4784c4-rpn4n: Unhealthy — Readiness probe failed: Get "https://10.42.101.122:8443/ping": dial tcp 10.42.101.122:8443: connect: connection refused |
| 301 | 14:15:48 | warning | Pod/brupop-apiserver-75bd4784c4-twmsq: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-certificate" not found |
| 380 | 14:15:48 | warning | Pod/web-8cb9878f-6j82j: FailedScheduling — 0/3 nodes are available: 1 node(s) were unschedulable, 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 381 | 14:15:48 | warning | Pod/web-8cb9878f-6j82j: FailedScheduling — 0/3 nodes are available: 1 node(s) didn't match pod topology spread constraints, 2 node(s) were unschedulable. no new claims to deallocate, preemption: 0/3 nodes are available: 1 No preemption victims found for incoming pod, 2 Preemption is not helpful for scheduling. |
| 400 | 14:15:48 | warning | Pod/web-8cb9878f-zh67z: FailedScheduling — 0/3 nodes are available: 1 node(s) were unschedulable, 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 401 | 14:15:48 | warning | Pod/web-8cb9878f-zh67z: FailedScheduling — 0/3 nodes are available: 1 node(s) had untolerated taint(s), 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 447 | 14:15:48 | warning | Pod/aws-node-h7pv5: Unhealthy — Readiness probe errored and resulted in unknown state: rpc error: code = Unknown desc = failed to exec in container: container is in CONTAINER_EXITED state |
| 463 | 14:15:48 | warning | Pod/aws-node-lq2hs: NodeShutdown — Pod was rejected as the node is shutting down. |
| 465 | 14:15:48 | warning | Pod/aws-node-mbtwj: NodeShutdown — Pod was rejected as the node is shutting down. |
| 482 | 14:15:48 | warning | Pod/aws-node-nblm8: NodeShutdown — Pod was rejected as the node is shutting down. |
| 484 | 14:15:48 | warning | Pod/aws-node-pckkf: NodeShutdown — Pod was rejected as the node is shutting down. |
| 491 | 14:15:48 | warning | Pod/aws-node-pskf9: NodeShutdown — Pod was rejected as the node is shutting down. |
| 499 | 14:15:48 | warning | Pod/aws-node-rmdmg: NodeShutdown — Pod was rejected as the node is shutting down. |
| 515 | 14:15:48 | warning | Pod/coredns-5f8c6645d8-g5nr5: FailedCreatePodSandBox — Failed to create pod sandbox: rpc error: code = Unknown desc = failed to setup network for sandbox "a70b50879bc5520e36978115a8f9755aff95f33a2ebfa8f8642fccf4dd903c7c": plugin type="aws-cni" name="aws-cni" failed (add): add cmd: Error received from AddNetwork gRPC call: rpc error: code = Unavailable desc = connection error: desc = "transport: Error while dialing: dial tcp 127.0.0.1:50051: connect: connection refused" |
| 520 | 14:15:48 | warning | Pod/coredns-5f8c6645d8-g5nr5: Unhealthy — Readiness probe failed: HTTP probe failed with statuscode: 503 |
| 521 | 14:15:48 | warning | Pod/coredns-5f8c6645d8-g5nr5: Unhealthy — Readiness probe failed: Get "http://10.42.100.33:8181/ready": dial tcp 10.42.100.33:8181: connect: connection refused |
| 528 | 14:15:48 | warning | Pod/coredns-5f8c6645d8-nchxv: Unhealthy — Readiness probe failed: Get "http://10.42.101.208:8181/ready": dial tcp 10.42.101.208:8181: connect: connection refused |
| 533 | 14:15:48 | warning | Pod/coredns-5f8c6645d8-qdvn8: Unhealthy — Readiness probe failed: Get "http://10.42.100.79:8181/ready": dial tcp 10.42.100.79:8181: connect: connection refused |
| 540 | 14:15:48 | warning | Pod/coredns-6cd89df5d6-4p222: Unhealthy — Readiness probe failed: Get "http://10.42.100.160:8181/ready": dial tcp 10.42.100.160:8181: connect: connection refused |
| 547 | 14:15:48 | warning | Pod/coredns-6cd89df5d6-6qtqm: Unhealthy — Readiness probe failed: Get "http://10.42.101.142:8181/ready": dial tcp 10.42.101.142:8181: connect: connection refused |
| 549 | 14:15:48 | warning | Pod/coredns-6cd89df5d6-vf4r2: FailedScheduling — 0/3 nodes are available: 3 node(s) had untolerated taint(s). no new claims to deallocate, preemption: 0/3 nodes are available: 3 Preemption is not helpful for scheduling. |
| 555 | 14:15:48 | warning | Pod/coredns-6cd89df5d6-vf4r2: Unhealthy — Readiness probe failed: Get "http://10.42.101.81:8181/ready": dial tcp 10.42.101.81:8181: connect: connection refused |
| 557 | 14:15:48 | warning | Pod/coredns-6cd89df5d6-w87j8: FailedScheduling — 0/3 nodes are available: 3 node(s) had untolerated taint(s). no new claims to deallocate, preemption: 0/3 nodes are available: 3 Preemption is not helpful for scheduling. |
| 563 | 14:15:48 | warning | Pod/coredns-6cd89df5d6-w87j8: Unhealthy — Readiness probe failed: Get "http://10.42.101.253:8181/ready": dial tcp 10.42.101.253:8181: connect: connection refused |
| 583 | 14:15:48 | warning | Pod/eks-pod-identity-agent-5hdct: NodeShutdown — Pod was rejected as the node is shutting down. |
| 604 | 14:15:48 | warning | Pod/eks-pod-identity-agent-82lwr: NodeShutdown — Pod was rejected as the node is shutting down. |
| 606 | 14:15:48 | warning | Pod/eks-pod-identity-agent-bs9m8: NodeShutdown — Pod was rejected as the node is shutting down. |
| 608 | 14:15:48 | warning | Pod/eks-pod-identity-agent-c5l9d: NodeShutdown — Pod was rejected as the node is shutting down. |
| 620 | 14:15:48 | warning | Pod/eks-pod-identity-agent-h8gks: NodeShutdown — Pod was rejected as the node is shutting down. |
| 622 | 14:15:48 | warning | Pod/eks-pod-identity-agent-htd2j: NodeShutdown — Pod was rejected as the node is shutting down. |
| 624 | 14:15:48 | warning | Pod/eks-pod-identity-agent-m9g9s: NodeShutdown — Pod was rejected as the node is shutting down. |
| 666 | 14:15:48 | warning | Pod/kube-proxy-c7c9c: NodeShutdown — Pod was rejected as the node is shutting down. |
| 668 | 14:15:48 | warning | Pod/kube-proxy-kbgt8: NodeShutdown — Pod was rejected as the node is shutting down. |
| 681 | 14:15:48 | warning | Pod/kube-proxy-tnsw7: NodeShutdown — Pod was rejected as the node is shutting down. |
| 683 | 14:15:48 | warning | Pod/kube-proxy-tqjzg: NodeShutdown — Pod was rejected as the node is shutting down. |
| 685 | 14:15:48 | warning | Pod/kube-proxy-w4h6f: NodeShutdown — Pod was rejected as the node is shutting down. |
| 687 | 14:15:48 | warning | Pod/kube-proxy-xmnf4: NodeShutdown — Pod was rejected as the node is shutting down. |
| 703 | 14:15:48 | warning | ReplicaSet/brupop-controller-deployment-584c75f8c8: FailedCreate — Error creating: pods "brupop-controller-deployment-584c75f8c8-" is forbidden: no PriorityClass with name brupop-controller-high-priority was found |
| 732 | 14:15:50 | preflight | preflight on [ip-10-42-100-186.us-east-2.compute.internal] at concurrency 1 → Blocked (16 pods predicted evicted) |
| 734 | 14:16:20 | preflight | preflight on [ip-10-42-100-186.us-east-2.compute.internal] at concurrency 1 → Blocked (16 pods predicted evicted) |
| 736 | 14:17:34 | run | run started: EKS Bottlerocket + Brupop demonstration |
| 738 | 14:17:34 | preflight | preflight on [ip-10-42-100-186.us-east-2.compute.internal] at concurrency 1 → Safe (0 pods predicted evicted) |
| 801 | 14:17:35 | warning | DaemonSet/brupop-agent: FailedDaemonPod — Found failed daemon pod brupop-bottlerocket-aws/brupop-agent-lg942 on node ip-10-42-100-186.us-east-2.compute.internal, will try to kill it |
| 802 | 14:17:35 | warning | DaemonSet/brupop-agent: FailedDaemonPod — Found failed daemon pod brupop-bottlerocket-aws/brupop-agent-shbfh on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 810 | 14:17:35 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-hvrm9 on node ip-10-42-100-186.us-east-2.compute.internal, will try to kill it |
| 811 | 14:17:35 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-pckkf on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 812 | 14:17:35 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-rmdmg on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 813 | 14:17:35 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-mbtwj on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 814 | 14:17:35 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-lq2hs on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 815 | 14:17:35 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-nblm8 on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 835 | 14:17:35 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-bs9m8 on node ip-10-42-100-186.us-east-2.compute.internal, will try to kill it |
| 836 | 14:17:35 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-5hdct on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 837 | 14:17:35 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-82lwr on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 838 | 14:17:35 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-htd2j on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 839 | 14:17:35 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-m9g9s on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 840 | 14:17:35 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-h8gks on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 841 | 14:17:35 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-c5l9d on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 863 | 14:17:35 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-slv2h on node ip-10-42-100-186.us-east-2.compute.internal, will try to kill it |
| 864 | 14:17:35 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-kbgt8 on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 865 | 14:17:35 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-xmnf4 on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 866 | 14:17:35 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-c7c9c on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 867 | 14:17:35 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-tqjzg on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 868 | 14:17:35 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-w4h6f on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 869 | 14:17:35 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-tnsw7 on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 903 | 14:17:35 | warning | Issuer/brupop-root-certificate-issuer: ErrGetKeyPair — Error getting keypair for CA issuer: secrets "brupop-root-ca-secret" not found |
| 904 | 14:17:35 | warning | Issuer/brupop-root-certificate-issuer: ErrInitIssuer — Error initializing issuer: secrets "brupop-root-ca-secret" not found |
| 918 | 14:17:35 | warning | Node/ip-10-42-100-186.us-east-2.compute.internal: InvalidDiskCapacity — invalid capacity 0 on image filesystem |
| 932 | 14:17:35 | warning | Node/ip-10-42-100-186.us-east-2.compute.internal: Rebooted — Node ip-10-42-100-186.us-east-2.compute.internal has been rebooted, boot id: 6436acb8-a1be-4339-8b6e-b6d04fbfa82c |
| 941 | 14:17:35 | warning | Node/ip-10-42-101-194.us-east-2.compute.internal: InvalidDiskCapacity — invalid capacity 0 on image filesystem |
| 953 | 14:17:35 | warning | Node/ip-10-42-101-90.us-east-2.compute.internal: InvalidDiskCapacity — invalid capacity 0 on image filesystem |
| 954 | 14:17:35 | warning | Node/ip-10-42-101-90.us-east-2.compute.internal: InvalidDiskCapacity — invalid capacity 0 on image filesystem |
| 968 | 14:17:35 | warning | Node/ip-10-42-101-90.us-east-2.compute.internal: Rebooted — Node ip-10-42-101-90.us-east-2.compute.internal has been rebooted, boot id: edad6a4f-2f72-4646-a370-f8ddebc2914f |
| 977 | 14:17:35 | warning | Pod/brupop-agent-9j5vd: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-client-certificate" not found |
| 982 | 14:17:35 | warning | Pod/brupop-agent-lg942: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-client-certificate" not found |
| 988 | 14:17:35 | warning | Pod/brupop-agent-shbfh: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-client-certificate" not found |
| 1010 | 14:17:35 | warning | Pod/brupop-apiserver-75bd4784c4-9cc8s: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-certificate" not found |
| 1026 | 14:17:35 | warning | Pod/brupop-apiserver-75bd4784c4-rpn4n: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-certificate" not found |
| 1032 | 14:17:35 | warning | Pod/brupop-apiserver-75bd4784c4-rpn4n: Unhealthy — Readiness probe failed: Get "https://10.42.101.122:8443/ping": dial tcp 10.42.101.122:8443: connect: connection refused |
| 1034 | 14:17:35 | warning | Pod/brupop-apiserver-75bd4784c4-twmsq: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-certificate" not found |
| 1113 | 14:17:35 | warning | Pod/web-8cb9878f-6j82j: FailedScheduling — 0/3 nodes are available: 1 node(s) were unschedulable, 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 1114 | 14:17:35 | warning | Pod/web-8cb9878f-6j82j: FailedScheduling — 0/3 nodes are available: 1 node(s) didn't match pod topology spread constraints, 2 node(s) were unschedulable. no new claims to deallocate, preemption: 0/3 nodes are available: 1 No preemption victims found for incoming pod, 2 Preemption is not helpful for scheduling. |
| 1133 | 14:17:35 | warning | Pod/web-8cb9878f-zh67z: FailedScheduling — 0/3 nodes are available: 1 node(s) were unschedulable, 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 1134 | 14:17:35 | warning | Pod/web-8cb9878f-zh67z: FailedScheduling — 0/3 nodes are available: 1 node(s) had untolerated taint(s), 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 1180 | 14:17:35 | warning | Pod/aws-node-h7pv5: Unhealthy — Readiness probe errored and resulted in unknown state: rpc error: code = Unknown desc = failed to exec in container: container is in CONTAINER_EXITED state |
| 1196 | 14:17:35 | warning | Pod/aws-node-lq2hs: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1198 | 14:17:35 | warning | Pod/aws-node-mbtwj: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1215 | 14:17:35 | warning | Pod/aws-node-nblm8: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1217 | 14:17:35 | warning | Pod/aws-node-pckkf: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1224 | 14:17:35 | warning | Pod/aws-node-pskf9: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1232 | 14:17:35 | warning | Pod/aws-node-rmdmg: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1248 | 14:17:35 | warning | Pod/coredns-5f8c6645d8-g5nr5: FailedCreatePodSandBox — Failed to create pod sandbox: rpc error: code = Unknown desc = failed to setup network for sandbox "a70b50879bc5520e36978115a8f9755aff95f33a2ebfa8f8642fccf4dd903c7c": plugin type="aws-cni" name="aws-cni" failed (add): add cmd: Error received from AddNetwork gRPC call: rpc error: code = Unavailable desc = connection error: desc = "transport: Error while dialing: dial tcp 127.0.0.1:50051: connect: connection refused" |
| 1253 | 14:17:35 | warning | Pod/coredns-5f8c6645d8-g5nr5: Unhealthy — Readiness probe failed: Get "http://10.42.100.33:8181/ready": dial tcp 10.42.100.33:8181: connect: connection refused |
| 1254 | 14:17:35 | warning | Pod/coredns-5f8c6645d8-g5nr5: Unhealthy — Readiness probe failed: HTTP probe failed with statuscode: 503 |
| 1261 | 14:17:35 | warning | Pod/coredns-5f8c6645d8-nchxv: Unhealthy — Readiness probe failed: Get "http://10.42.101.208:8181/ready": dial tcp 10.42.101.208:8181: connect: connection refused |
| 1266 | 14:17:35 | warning | Pod/coredns-5f8c6645d8-qdvn8: Unhealthy — Readiness probe failed: Get "http://10.42.100.79:8181/ready": dial tcp 10.42.100.79:8181: connect: connection refused |
| 1273 | 14:17:35 | warning | Pod/coredns-6cd89df5d6-4p222: Unhealthy — Readiness probe failed: Get "http://10.42.100.160:8181/ready": dial tcp 10.42.100.160:8181: connect: connection refused |
| 1280 | 14:17:35 | warning | Pod/coredns-6cd89df5d6-6qtqm: Unhealthy — Readiness probe failed: Get "http://10.42.101.142:8181/ready": dial tcp 10.42.101.142:8181: connect: connection refused |
| 1282 | 14:17:35 | warning | Pod/coredns-6cd89df5d6-vf4r2: FailedScheduling — 0/3 nodes are available: 3 node(s) had untolerated taint(s). no new claims to deallocate, preemption: 0/3 nodes are available: 3 Preemption is not helpful for scheduling. |
| 1288 | 14:17:35 | warning | Pod/coredns-6cd89df5d6-vf4r2: Unhealthy — Readiness probe failed: Get "http://10.42.101.81:8181/ready": dial tcp 10.42.101.81:8181: connect: connection refused |
| 1290 | 14:17:35 | warning | Pod/coredns-6cd89df5d6-w87j8: FailedScheduling — 0/3 nodes are available: 3 node(s) had untolerated taint(s). no new claims to deallocate, preemption: 0/3 nodes are available: 3 Preemption is not helpful for scheduling. |
| 1296 | 14:17:35 | warning | Pod/coredns-6cd89df5d6-w87j8: Unhealthy — Readiness probe failed: Get "http://10.42.101.253:8181/ready": dial tcp 10.42.101.253:8181: connect: connection refused |
| 1316 | 14:17:35 | warning | Pod/eks-pod-identity-agent-5hdct: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1337 | 14:17:35 | warning | Pod/eks-pod-identity-agent-82lwr: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1339 | 14:17:35 | warning | Pod/eks-pod-identity-agent-bs9m8: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1341 | 14:17:35 | warning | Pod/eks-pod-identity-agent-c5l9d: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1353 | 14:17:35 | warning | Pod/eks-pod-identity-agent-h8gks: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1355 | 14:17:35 | warning | Pod/eks-pod-identity-agent-htd2j: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1357 | 14:17:35 | warning | Pod/eks-pod-identity-agent-m9g9s: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1399 | 14:17:35 | warning | Pod/kube-proxy-c7c9c: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1401 | 14:17:35 | warning | Pod/kube-proxy-kbgt8: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1414 | 14:17:35 | warning | Pod/kube-proxy-tnsw7: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1416 | 14:17:35 | warning | Pod/kube-proxy-tqjzg: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1418 | 14:17:35 | warning | Pod/kube-proxy-w4h6f: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1420 | 14:17:35 | warning | Pod/kube-proxy-xmnf4: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1436 | 14:17:35 | warning | ReplicaSet/brupop-controller-deployment-584c75f8c8: FailedCreate — Error creating: pods "brupop-controller-deployment-584c75f8c8-" is forbidden: no PriorityClass with name brupop-controller-high-priority was found |
| 1465 | 14:18:41 | preflight | preflight on [ip-10-42-100-186.us-east-2.compute.internal] at concurrency 1 → Blocked (16 pods predicted evicted) |
| 1468 | 14:20:41 | warning | Pod/web-8cb9878f-6j82j: FailedScheduling — 0/3 nodes are available: 1 node(s) didn't match pod topology spread constraints, 2 node(s) were unschedulable. no new claims to deallocate, preemption: 0/3 nodes are available: 1 No preemption victims found for incoming pod, 2 Preemption is not helpful for scheduling. |
| 1474 | 14:25:41 | warning | Pod/web-8cb9878f-6j82j: FailedScheduling — 0/3 nodes are available: 1 node(s) didn't match pod topology spread constraints, 2 node(s) were unschedulable. no new claims to deallocate, preemption: 0/3 nodes are available: 1 No preemption victims found for incoming pod, 2 Preemption is not helpful for scheduling. |
| 1477 | 14:28:34 | run | run started: EKS Bottlerocket + Brupop demonstration |
| 1540 | 14:28:34 | warning | DaemonSet/brupop-agent: FailedDaemonPod — Found failed daemon pod brupop-bottlerocket-aws/brupop-agent-lg942 on node ip-10-42-100-186.us-east-2.compute.internal, will try to kill it |
| 1541 | 14:28:34 | warning | DaemonSet/brupop-agent: FailedDaemonPod — Found failed daemon pod brupop-bottlerocket-aws/brupop-agent-shbfh on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 1549 | 14:28:34 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-hvrm9 on node ip-10-42-100-186.us-east-2.compute.internal, will try to kill it |
| 1550 | 14:28:34 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-pckkf on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 1551 | 14:28:34 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-rmdmg on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 1552 | 14:28:34 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-mbtwj on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 1553 | 14:28:34 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-lq2hs on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 1554 | 14:28:34 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-nblm8 on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 1574 | 14:28:34 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-bs9m8 on node ip-10-42-100-186.us-east-2.compute.internal, will try to kill it |
| 1575 | 14:28:34 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-5hdct on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 1576 | 14:28:34 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-82lwr on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 1577 | 14:28:34 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-htd2j on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 1578 | 14:28:34 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-m9g9s on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 1579 | 14:28:34 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-h8gks on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 1580 | 14:28:34 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-c5l9d on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 1602 | 14:28:34 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-slv2h on node ip-10-42-100-186.us-east-2.compute.internal, will try to kill it |
| 1603 | 14:28:34 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-kbgt8 on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 1604 | 14:28:34 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-xmnf4 on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 1605 | 14:28:34 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-c7c9c on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 1606 | 14:28:34 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-tqjzg on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 1607 | 14:28:34 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-w4h6f on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 1608 | 14:28:34 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-tnsw7 on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 1642 | 14:28:34 | warning | Issuer/brupop-root-certificate-issuer: ErrGetKeyPair — Error getting keypair for CA issuer: secrets "brupop-root-ca-secret" not found |
| 1643 | 14:28:34 | warning | Issuer/brupop-root-certificate-issuer: ErrInitIssuer — Error initializing issuer: secrets "brupop-root-ca-secret" not found |
| 1651 | 14:28:34 | warning | Node/ip-10-42-100-186.us-east-2.compute.internal: InvalidDiskCapacity — invalid capacity 0 on image filesystem |
| 1665 | 14:28:34 | warning | Node/ip-10-42-100-186.us-east-2.compute.internal: Rebooted — Node ip-10-42-100-186.us-east-2.compute.internal has been rebooted, boot id: 6436acb8-a1be-4339-8b6e-b6d04fbfa82c |
| 1674 | 14:28:34 | warning | Node/ip-10-42-101-194.us-east-2.compute.internal: InvalidDiskCapacity — invalid capacity 0 on image filesystem |
| 1686 | 14:28:34 | warning | Node/ip-10-42-101-90.us-east-2.compute.internal: InvalidDiskCapacity — invalid capacity 0 on image filesystem |
| 1687 | 14:28:34 | warning | Node/ip-10-42-101-90.us-east-2.compute.internal: InvalidDiskCapacity — invalid capacity 0 on image filesystem |
| 1701 | 14:28:34 | warning | Node/ip-10-42-101-90.us-east-2.compute.internal: Rebooted — Node ip-10-42-101-90.us-east-2.compute.internal has been rebooted, boot id: edad6a4f-2f72-4646-a370-f8ddebc2914f |
| 1710 | 14:28:34 | warning | Pod/brupop-agent-9j5vd: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-client-certificate" not found |
| 1715 | 14:28:34 | warning | Pod/brupop-agent-lg942: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-client-certificate" not found |
| 1721 | 14:28:34 | warning | Pod/brupop-agent-shbfh: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-client-certificate" not found |
| 1743 | 14:28:34 | warning | Pod/brupop-apiserver-75bd4784c4-9cc8s: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-certificate" not found |
| 1759 | 14:28:34 | warning | Pod/brupop-apiserver-75bd4784c4-rpn4n: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-certificate" not found |
| 1765 | 14:28:34 | warning | Pod/brupop-apiserver-75bd4784c4-rpn4n: Unhealthy — Readiness probe failed: Get "https://10.42.101.122:8443/ping": dial tcp 10.42.101.122:8443: connect: connection refused |
| 1767 | 14:28:34 | warning | Pod/brupop-apiserver-75bd4784c4-twmsq: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-certificate" not found |
| 1846 | 14:28:34 | warning | Pod/web-8cb9878f-6j82j: FailedScheduling — 0/3 nodes are available: 1 node(s) were unschedulable, 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 1847 | 14:28:34 | warning | Pod/web-8cb9878f-6j82j: FailedScheduling — 0/3 nodes are available: 1 node(s) didn't match pod topology spread constraints, 2 node(s) were unschedulable. no new claims to deallocate, preemption: 0/3 nodes are available: 1 No preemption victims found for incoming pod, 2 Preemption is not helpful for scheduling. |
| 1866 | 14:28:34 | warning | Pod/web-8cb9878f-zh67z: FailedScheduling — 0/3 nodes are available: 1 node(s) were unschedulable, 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 1867 | 14:28:34 | warning | Pod/web-8cb9878f-zh67z: FailedScheduling — 0/3 nodes are available: 1 node(s) had untolerated taint(s), 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 1913 | 14:28:34 | warning | Pod/aws-node-h7pv5: Unhealthy — Readiness probe errored and resulted in unknown state: rpc error: code = Unknown desc = failed to exec in container: container is in CONTAINER_EXITED state |
| 1929 | 14:28:34 | warning | Pod/aws-node-lq2hs: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1931 | 14:28:34 | warning | Pod/aws-node-mbtwj: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1948 | 14:28:34 | warning | Pod/aws-node-nblm8: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1950 | 14:28:34 | warning | Pod/aws-node-pckkf: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1957 | 14:28:34 | warning | Pod/aws-node-pskf9: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1965 | 14:28:34 | warning | Pod/aws-node-rmdmg: NodeShutdown — Pod was rejected as the node is shutting down. |
| 1981 | 14:28:34 | warning | Pod/coredns-5f8c6645d8-g5nr5: FailedCreatePodSandBox — Failed to create pod sandbox: rpc error: code = Unknown desc = failed to setup network for sandbox "a70b50879bc5520e36978115a8f9755aff95f33a2ebfa8f8642fccf4dd903c7c": plugin type="aws-cni" name="aws-cni" failed (add): add cmd: Error received from AddNetwork gRPC call: rpc error: code = Unavailable desc = connection error: desc = "transport: Error while dialing: dial tcp 127.0.0.1:50051: connect: connection refused" |
| 1986 | 14:28:34 | warning | Pod/coredns-5f8c6645d8-g5nr5: Unhealthy — Readiness probe failed: HTTP probe failed with statuscode: 503 |
| 1987 | 14:28:34 | warning | Pod/coredns-5f8c6645d8-g5nr5: Unhealthy — Readiness probe failed: Get "http://10.42.100.33:8181/ready": dial tcp 10.42.100.33:8181: connect: connection refused |
| 1994 | 14:28:34 | warning | Pod/coredns-5f8c6645d8-nchxv: Unhealthy — Readiness probe failed: Get "http://10.42.101.208:8181/ready": dial tcp 10.42.101.208:8181: connect: connection refused |
| 1999 | 14:28:34 | warning | Pod/coredns-5f8c6645d8-qdvn8: Unhealthy — Readiness probe failed: Get "http://10.42.100.79:8181/ready": dial tcp 10.42.100.79:8181: connect: connection refused |
| 2006 | 14:28:34 | warning | Pod/coredns-6cd89df5d6-4p222: Unhealthy — Readiness probe failed: Get "http://10.42.100.160:8181/ready": dial tcp 10.42.100.160:8181: connect: connection refused |
| 2013 | 14:28:34 | warning | Pod/coredns-6cd89df5d6-6qtqm: Unhealthy — Readiness probe failed: Get "http://10.42.101.142:8181/ready": dial tcp 10.42.101.142:8181: connect: connection refused |
| 2015 | 14:28:34 | warning | Pod/coredns-6cd89df5d6-vf4r2: FailedScheduling — 0/3 nodes are available: 3 node(s) had untolerated taint(s). no new claims to deallocate, preemption: 0/3 nodes are available: 3 Preemption is not helpful for scheduling. |
| 2021 | 14:28:34 | warning | Pod/coredns-6cd89df5d6-vf4r2: Unhealthy — Readiness probe failed: Get "http://10.42.101.81:8181/ready": dial tcp 10.42.101.81:8181: connect: connection refused |
| 2023 | 14:28:34 | warning | Pod/coredns-6cd89df5d6-w87j8: FailedScheduling — 0/3 nodes are available: 3 node(s) had untolerated taint(s). no new claims to deallocate, preemption: 0/3 nodes are available: 3 Preemption is not helpful for scheduling. |
| 2029 | 14:28:34 | warning | Pod/coredns-6cd89df5d6-w87j8: Unhealthy — Readiness probe failed: Get "http://10.42.101.253:8181/ready": dial tcp 10.42.101.253:8181: connect: connection refused |
| 2049 | 14:28:34 | warning | Pod/eks-pod-identity-agent-5hdct: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2070 | 14:28:34 | warning | Pod/eks-pod-identity-agent-82lwr: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2072 | 14:28:34 | warning | Pod/eks-pod-identity-agent-bs9m8: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2074 | 14:28:34 | warning | Pod/eks-pod-identity-agent-c5l9d: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2086 | 14:28:34 | warning | Pod/eks-pod-identity-agent-h8gks: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2088 | 14:28:34 | warning | Pod/eks-pod-identity-agent-htd2j: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2090 | 14:28:34 | warning | Pod/eks-pod-identity-agent-m9g9s: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2132 | 14:28:34 | warning | Pod/kube-proxy-c7c9c: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2134 | 14:28:34 | warning | Pod/kube-proxy-kbgt8: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2147 | 14:28:34 | warning | Pod/kube-proxy-tnsw7: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2149 | 14:28:34 | warning | Pod/kube-proxy-tqjzg: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2151 | 14:28:34 | warning | Pod/kube-proxy-w4h6f: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2153 | 14:28:34 | warning | Pod/kube-proxy-xmnf4: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2169 | 14:28:34 | warning | ReplicaSet/brupop-controller-deployment-584c75f8c8: FailedCreate — Error creating: pods "brupop-controller-deployment-584c75f8c8-" is forbidden: no PriorityClass with name brupop-controller-high-priority was found |
| 2198 | 14:29:21 | preflight | preflight on [ip-10-42-101-194.us-east-2.compute.internal] at concurrency 1 → Blocked (5 pods predicted evicted) |
| 2201 | 14:30:42 | warning | Pod/web-8cb9878f-6j82j: FailedScheduling — 0/3 nodes are available: 1 node(s) didn't match pod topology spread constraints, 2 node(s) were unschedulable. no new claims to deallocate, preemption: 0/3 nodes are available: 1 No preemption victims found for incoming pod, 2 Preemption is not helpful for scheduling. |
| 2202 | 14:30:59 | run | run started: EKS Bottlerocket + Brupop demonstration |
| 2231 | 14:31:00 | brupop | brupop: brs-ip-10-42-100-186.us-east-2.compute.internal → Idle (target 1.64.0) |
| 2232 | 14:31:00 | brupop | brupop: brs-ip-10-42-101-194.us-east-2.compute.internal → StagedAndPerformedUpdate (target 1.64.0) |
| 2233 | 14:31:00 | brupop | brupop: brs-ip-10-42-101-90.us-east-2.compute.internal → Idle (target 1.64.0) |
| 2269 | 14:31:00 | warning | DaemonSet/brupop-agent: FailedDaemonPod — Found failed daemon pod brupop-bottlerocket-aws/brupop-agent-lg942 on node ip-10-42-100-186.us-east-2.compute.internal, will try to kill it |
| 2270 | 14:31:00 | warning | DaemonSet/brupop-agent: FailedDaemonPod — Found failed daemon pod brupop-bottlerocket-aws/brupop-agent-shbfh on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 2278 | 14:31:00 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-hvrm9 on node ip-10-42-100-186.us-east-2.compute.internal, will try to kill it |
| 2279 | 14:31:00 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-pckkf on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 2280 | 14:31:00 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-rmdmg on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 2281 | 14:31:00 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-mbtwj on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 2282 | 14:31:00 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-lq2hs on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 2283 | 14:31:00 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-nblm8 on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 2303 | 14:31:00 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-bs9m8 on node ip-10-42-100-186.us-east-2.compute.internal, will try to kill it |
| 2304 | 14:31:00 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-5hdct on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 2305 | 14:31:00 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-82lwr on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 2306 | 14:31:00 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-htd2j on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 2307 | 14:31:00 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-m9g9s on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 2308 | 14:31:00 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-h8gks on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 2309 | 14:31:00 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-c5l9d on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 2331 | 14:31:00 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-slv2h on node ip-10-42-100-186.us-east-2.compute.internal, will try to kill it |
| 2332 | 14:31:00 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-kbgt8 on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 2333 | 14:31:00 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-xmnf4 on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 2334 | 14:31:00 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-c7c9c on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 2335 | 14:31:00 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-tqjzg on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 2336 | 14:31:00 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-w4h6f on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 2337 | 14:31:00 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-tnsw7 on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 2371 | 14:31:00 | warning | Issuer/brupop-root-certificate-issuer: ErrGetKeyPair — Error getting keypair for CA issuer: secrets "brupop-root-ca-secret" not found |
| 2372 | 14:31:00 | warning | Issuer/brupop-root-certificate-issuer: ErrInitIssuer — Error initializing issuer: secrets "brupop-root-ca-secret" not found |
| 2380 | 14:31:00 | warning | Node/ip-10-42-100-186.us-east-2.compute.internal: InvalidDiskCapacity — invalid capacity 0 on image filesystem |
| 2394 | 14:31:00 | warning | Node/ip-10-42-100-186.us-east-2.compute.internal: Rebooted — Node ip-10-42-100-186.us-east-2.compute.internal has been rebooted, boot id: 6436acb8-a1be-4339-8b6e-b6d04fbfa82c |
| 2403 | 14:31:00 | warning | Node/ip-10-42-101-194.us-east-2.compute.internal: InvalidDiskCapacity — invalid capacity 0 on image filesystem |
| 2415 | 14:31:00 | warning | Node/ip-10-42-101-90.us-east-2.compute.internal: InvalidDiskCapacity — invalid capacity 0 on image filesystem |
| 2416 | 14:31:00 | warning | Node/ip-10-42-101-90.us-east-2.compute.internal: InvalidDiskCapacity — invalid capacity 0 on image filesystem |
| 2430 | 14:31:00 | warning | Node/ip-10-42-101-90.us-east-2.compute.internal: Rebooted — Node ip-10-42-101-90.us-east-2.compute.internal has been rebooted, boot id: edad6a4f-2f72-4646-a370-f8ddebc2914f |
| 2439 | 14:31:00 | warning | Pod/brupop-agent-9j5vd: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-client-certificate" not found |
| 2444 | 14:31:00 | warning | Pod/brupop-agent-lg942: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-client-certificate" not found |
| 2450 | 14:31:00 | warning | Pod/brupop-agent-shbfh: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-client-certificate" not found |
| 2472 | 14:31:00 | warning | Pod/brupop-apiserver-75bd4784c4-9cc8s: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-certificate" not found |
| 2488 | 14:31:00 | warning | Pod/brupop-apiserver-75bd4784c4-rpn4n: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-certificate" not found |
| 2494 | 14:31:00 | warning | Pod/brupop-apiserver-75bd4784c4-rpn4n: Unhealthy — Readiness probe failed: Get "https://10.42.101.122:8443/ping": dial tcp 10.42.101.122:8443: connect: connection refused |
| 2496 | 14:31:00 | warning | Pod/brupop-apiserver-75bd4784c4-twmsq: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-certificate" not found |
| 2575 | 14:31:00 | warning | Pod/web-8cb9878f-6j82j: FailedScheduling — 0/3 nodes are available: 1 node(s) were unschedulable, 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 2576 | 14:31:00 | warning | Pod/web-8cb9878f-6j82j: FailedScheduling — 0/3 nodes are available: 1 node(s) didn't match pod topology spread constraints, 2 node(s) were unschedulable. no new claims to deallocate, preemption: 0/3 nodes are available: 1 No preemption victims found for incoming pod, 2 Preemption is not helpful for scheduling. |
| 2595 | 14:31:00 | warning | Pod/web-8cb9878f-zh67z: FailedScheduling — 0/3 nodes are available: 1 node(s) were unschedulable, 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 2596 | 14:31:00 | warning | Pod/web-8cb9878f-zh67z: FailedScheduling — 0/3 nodes are available: 1 node(s) had untolerated taint(s), 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 2642 | 14:31:00 | warning | Pod/aws-node-h7pv5: Unhealthy — Readiness probe errored and resulted in unknown state: rpc error: code = Unknown desc = failed to exec in container: container is in CONTAINER_EXITED state |
| 2658 | 14:31:00 | warning | Pod/aws-node-lq2hs: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2660 | 14:31:00 | warning | Pod/aws-node-mbtwj: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2677 | 14:31:00 | warning | Pod/aws-node-nblm8: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2679 | 14:31:00 | warning | Pod/aws-node-pckkf: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2686 | 14:31:00 | warning | Pod/aws-node-pskf9: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2694 | 14:31:00 | warning | Pod/aws-node-rmdmg: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2710 | 14:31:00 | warning | Pod/coredns-5f8c6645d8-g5nr5: FailedCreatePodSandBox — Failed to create pod sandbox: rpc error: code = Unknown desc = failed to setup network for sandbox "a70b50879bc5520e36978115a8f9755aff95f33a2ebfa8f8642fccf4dd903c7c": plugin type="aws-cni" name="aws-cni" failed (add): add cmd: Error received from AddNetwork gRPC call: rpc error: code = Unavailable desc = connection error: desc = "transport: Error while dialing: dial tcp 127.0.0.1:50051: connect: connection refused" |
| 2715 | 14:31:00 | warning | Pod/coredns-5f8c6645d8-g5nr5: Unhealthy — Readiness probe failed: HTTP probe failed with statuscode: 503 |
| 2716 | 14:31:00 | warning | Pod/coredns-5f8c6645d8-g5nr5: Unhealthy — Readiness probe failed: Get "http://10.42.100.33:8181/ready": dial tcp 10.42.100.33:8181: connect: connection refused |
| 2723 | 14:31:00 | warning | Pod/coredns-5f8c6645d8-nchxv: Unhealthy — Readiness probe failed: Get "http://10.42.101.208:8181/ready": dial tcp 10.42.101.208:8181: connect: connection refused |
| 2728 | 14:31:00 | warning | Pod/coredns-5f8c6645d8-qdvn8: Unhealthy — Readiness probe failed: Get "http://10.42.100.79:8181/ready": dial tcp 10.42.100.79:8181: connect: connection refused |
| 2735 | 14:31:00 | warning | Pod/coredns-6cd89df5d6-4p222: Unhealthy — Readiness probe failed: Get "http://10.42.100.160:8181/ready": dial tcp 10.42.100.160:8181: connect: connection refused |
| 2742 | 14:31:00 | warning | Pod/coredns-6cd89df5d6-6qtqm: Unhealthy — Readiness probe failed: Get "http://10.42.101.142:8181/ready": dial tcp 10.42.101.142:8181: connect: connection refused |
| 2744 | 14:31:00 | warning | Pod/coredns-6cd89df5d6-vf4r2: FailedScheduling — 0/3 nodes are available: 3 node(s) had untolerated taint(s). no new claims to deallocate, preemption: 0/3 nodes are available: 3 Preemption is not helpful for scheduling. |
| 2750 | 14:31:00 | warning | Pod/coredns-6cd89df5d6-vf4r2: Unhealthy — Readiness probe failed: Get "http://10.42.101.81:8181/ready": dial tcp 10.42.101.81:8181: connect: connection refused |
| 2752 | 14:31:00 | warning | Pod/coredns-6cd89df5d6-w87j8: FailedScheduling — 0/3 nodes are available: 3 node(s) had untolerated taint(s). no new claims to deallocate, preemption: 0/3 nodes are available: 3 Preemption is not helpful for scheduling. |
| 2758 | 14:31:00 | warning | Pod/coredns-6cd89df5d6-w87j8: Unhealthy — Readiness probe failed: Get "http://10.42.101.253:8181/ready": dial tcp 10.42.101.253:8181: connect: connection refused |
| 2778 | 14:31:00 | warning | Pod/eks-pod-identity-agent-5hdct: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2799 | 14:31:00 | warning | Pod/eks-pod-identity-agent-82lwr: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2801 | 14:31:00 | warning | Pod/eks-pod-identity-agent-bs9m8: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2803 | 14:31:00 | warning | Pod/eks-pod-identity-agent-c5l9d: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2815 | 14:31:00 | warning | Pod/eks-pod-identity-agent-h8gks: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2817 | 14:31:00 | warning | Pod/eks-pod-identity-agent-htd2j: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2819 | 14:31:00 | warning | Pod/eks-pod-identity-agent-m9g9s: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2861 | 14:31:00 | warning | Pod/kube-proxy-c7c9c: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2863 | 14:31:00 | warning | Pod/kube-proxy-kbgt8: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2876 | 14:31:00 | warning | Pod/kube-proxy-tnsw7: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2878 | 14:31:00 | warning | Pod/kube-proxy-tqjzg: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2880 | 14:31:00 | warning | Pod/kube-proxy-w4h6f: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2882 | 14:31:00 | warning | Pod/kube-proxy-xmnf4: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2898 | 14:31:00 | warning | ReplicaSet/brupop-controller-deployment-584c75f8c8: FailedCreate — Error creating: pods "brupop-controller-deployment-584c75f8c8-" is forbidden: no PriorityClass with name brupop-controller-high-priority was found |
| 2929 | 14:34:50 | preflight | preflight on [ip-10-42-101-194.us-east-2.compute.internal] at concurrency 1 → Blocked (5 pods predicted evicted) |
| 2934 | 14:35:11 | warning | Pod/web-8cb9878f-6j82j: FailedScheduling — 0/3 nodes are available: 1 node(s) didn't match pod topology spread constraints, 1 node(s) had untolerated taint(s), 1 node(s) were unschedulable. no new claims to deallocate, preemption: 0/3 nodes are available: 1 No preemption victims found for incoming pod, 2 Preemption is not helpful for scheduling. |
| 2943 | 14:35:13 | warning | Pod/web-8cb9878f-hjtw9: FailedScheduling — 0/3 nodes are available: 1 node(s) were unschedulable, 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 2948 | 14:35:13 | warning | Pod/web-8cb9878f-hjtw9: FailedScheduling — 0/3 nodes are available: 1 node(s) were unschedulable, 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 2950 | 14:35:14 | eviction | pod demo/web-8cb9878f-n28s6 left ip-10-42-101-194.us-east-2.compute.internal |
| 2954 | 14:35:18 | node | node ip-10-42-101-194.us-east-2.compute.internal: ready=false cordoned=true |
| 2958 | 14:35:18 | warning | Pod/web-8cb9878f-hjtw9: FailedScheduling — 0/3 nodes are available: 1 node(s) were unschedulable, 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 2960 | 14:35:22 | warning | Pod/web-8cb9878f-hjtw9: FailedScheduling — 0/3 nodes are available: 1 node(s) were unschedulable, 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 2962 | 14:35:28 | brupop | brupop: brs-ip-10-42-101-194.us-east-2.compute.internal → RebootedIntoUpdate (target 1.64.0) |
| 2964 | 14:35:30 | brupop | brupop: brs-ip-10-42-101-194.us-east-2.compute.internal → RebootedIntoUpdate (target 1.64.0) |
| 2966 | 14:35:48 | brupop | brupop: brs-ip-10-42-101-194.us-east-2.compute.internal → MonitoringUpdate (target 1.64.0) |
| 2968 | 14:35:48 | eviction | pod brupop-bottlerocket-aws/brupop-agent-9j5vd left ip-10-42-101-194.us-east-2.compute.internal |
| 2969 | 14:35:48 | warning | DaemonSet/brupop-agent: FailedDaemonPod — Found failed daemon pod brupop-bottlerocket-aws/brupop-agent-9j5vd on node ip-10-42-101-194.us-east-2.compute.internal, will try to kill it |
| 2972 | 14:35:49 | eviction | pod kube-system/eks-pod-identity-agent-5g57p left ip-10-42-101-194.us-east-2.compute.internal |
| 2976 | 14:35:49 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-lmstm on node ip-10-42-101-194.us-east-2.compute.internal, will try to kill it |
| 2982 | 14:35:49 | eviction | pod kube-system/aws-node-ck4l5 left ip-10-42-101-194.us-east-2.compute.internal |
| 2983 | 14:35:49 | eviction | pod kube-system/kube-proxy-lmstm left ip-10-42-101-194.us-east-2.compute.internal |
| 2987 | 14:35:49 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-ck4l5 on node ip-10-42-101-194.us-east-2.compute.internal, will try to kill it |
| 2990 | 14:35:49 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-psg8p on node ip-10-42-101-194.us-east-2.compute.internal, will try to kill it |
| 2994 | 14:35:49 | warning | Pod/aws-node-ldfvv: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2996 | 14:35:49 | warning | Pod/eks-pod-identity-agent-l9g7t: NodeShutdown — Pod was rejected as the node is shutting down. |
| 2998 | 14:35:49 | warning | Pod/eks-pod-identity-agent-psg8p: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3000 | 14:35:49 | warning | Pod/kube-proxy-dg5fv: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3003 | 14:35:50 | brupop | brupop: brs-ip-10-42-101-194.us-east-2.compute.internal → MonitoringUpdate (target 1.64.0) |
| 3005 | 14:35:50 | eviction | pod kube-system/aws-node-ldfvv left ip-10-42-101-194.us-east-2.compute.internal |
| 3006 | 14:35:50 | eviction | pod kube-system/eks-pod-identity-agent-l9g7t left ip-10-42-101-194.us-east-2.compute.internal |
| 3007 | 14:35:50 | eviction | pod kube-system/kube-proxy-dg5fv left ip-10-42-101-194.us-east-2.compute.internal |
| 3011 | 14:35:50 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-ldfvv on node ip-10-42-101-194.us-east-2.compute.internal, will try to kill it |
| 3014 | 14:35:50 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-l9g7t on node ip-10-42-101-194.us-east-2.compute.internal, will try to kill it |
| 3015 | 14:35:50 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-dg5fv on node ip-10-42-101-194.us-east-2.compute.internal, will try to kill it |
| 3022 | 14:36:06 | node | node ip-10-42-101-194.us-east-2.compute.internal: ready=false cordoned=true |
| 3023 | 14:36:06 | warning | Node/ip-10-42-101-194.us-east-2.compute.internal: InvalidDiskCapacity — invalid capacity 0 on image filesystem |
| 3029 | 14:36:06 | warning | Node/ip-10-42-101-194.us-east-2.compute.internal: Rebooted — Node ip-10-42-101-194.us-east-2.compute.internal has been rebooted, boot id: 6d868fcc-7686-460d-b665-6d2516d891b9 |
| 3064 | 14:36:14 | node | node ip-10-42-101-194.us-east-2.compute.internal: ready=true cordoned=true |
| 3068 | 14:36:14 | warning | Pod/web-8cb9878f-hjtw9: FailedScheduling — 0/3 nodes are available: 1 node(s) were unschedulable, 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 3074 | 14:36:17 | warning | Pod/web-8cb9878f-hjtw9: FailedScheduling — 0/3 nodes are available: 1 node(s) were unschedulable, 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 3079 | 14:36:33 | brupop | brupop: brs-ip-10-42-101-194.us-east-2.compute.internal → Idle (target 1.64.0) |
| 3090 | 14:41:41 | warning | Pod/web-8cb9878f-hjtw9: FailedScheduling — 0/3 nodes are available: 1 node(s) were unschedulable, 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 3093 | 14:45:11 | preflight | preflight on [ip-10-42-100-186.us-east-2.compute.internal] at concurrency 1 → Blocked (16 pods predicted evicted) |
| 3095 | 14:46:12 | run | run started: EKS Bottlerocket + Brupop demonstration |
| 3125 | 14:46:12 | node | node ip-10-42-101-194.us-east-2.compute.internal: ready=true cordoned=true |
| 3161 | 14:46:12 | warning | DaemonSet/brupop-agent: FailedDaemonPod — Found failed daemon pod brupop-bottlerocket-aws/brupop-agent-lg942 on node ip-10-42-100-186.us-east-2.compute.internal, will try to kill it |
| 3162 | 14:46:12 | warning | DaemonSet/brupop-agent: FailedDaemonPod — Found failed daemon pod brupop-bottlerocket-aws/brupop-agent-shbfh on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 3163 | 14:46:12 | warning | DaemonSet/brupop-agent: FailedDaemonPod — Found failed daemon pod brupop-bottlerocket-aws/brupop-agent-9j5vd on node ip-10-42-101-194.us-east-2.compute.internal, will try to kill it |
| 3173 | 14:46:12 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-hvrm9 on node ip-10-42-100-186.us-east-2.compute.internal, will try to kill it |
| 3174 | 14:46:12 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-pckkf on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 3175 | 14:46:12 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-rmdmg on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 3176 | 14:46:12 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-mbtwj on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 3177 | 14:46:12 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-lq2hs on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 3178 | 14:46:12 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-nblm8 on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 3179 | 14:46:12 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-ck4l5 on node ip-10-42-101-194.us-east-2.compute.internal, will try to kill it |
| 3180 | 14:46:12 | warning | DaemonSet/aws-node: FailedDaemonPod — Found failed daemon pod kube-system/aws-node-ldfvv on node ip-10-42-101-194.us-east-2.compute.internal, will try to kill it |
| 3204 | 14:46:12 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-bs9m8 on node ip-10-42-100-186.us-east-2.compute.internal, will try to kill it |
| 3205 | 14:46:12 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-5hdct on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 3206 | 14:46:12 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-82lwr on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 3207 | 14:46:12 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-htd2j on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 3208 | 14:46:12 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-m9g9s on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 3209 | 14:46:12 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-h8gks on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 3210 | 14:46:12 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-c5l9d on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 3211 | 14:46:12 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-psg8p on node ip-10-42-101-194.us-east-2.compute.internal, will try to kill it |
| 3212 | 14:46:12 | warning | DaemonSet/eks-pod-identity-agent: FailedDaemonPod — Found failed daemon pod kube-system/eks-pod-identity-agent-l9g7t on node ip-10-42-101-194.us-east-2.compute.internal, will try to kill it |
| 3238 | 14:46:12 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-slv2h on node ip-10-42-100-186.us-east-2.compute.internal, will try to kill it |
| 3239 | 14:46:12 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-kbgt8 on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 3240 | 14:46:12 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-xmnf4 on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 3241 | 14:46:12 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-c7c9c on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 3242 | 14:46:12 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-tqjzg on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 3243 | 14:46:12 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-w4h6f on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 3244 | 14:46:12 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-tnsw7 on node ip-10-42-101-90.us-east-2.compute.internal, will try to kill it |
| 3245 | 14:46:12 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-lmstm on node ip-10-42-101-194.us-east-2.compute.internal, will try to kill it |
| 3246 | 14:46:12 | warning | DaemonSet/kube-proxy: FailedDaemonPod — Found failed daemon pod kube-system/kube-proxy-dg5fv on node ip-10-42-101-194.us-east-2.compute.internal, will try to kill it |
| 3284 | 14:46:12 | warning | Issuer/brupop-root-certificate-issuer: ErrGetKeyPair — Error getting keypair for CA issuer: secrets "brupop-root-ca-secret" not found |
| 3285 | 14:46:12 | warning | Issuer/brupop-root-certificate-issuer: ErrInitIssuer — Error initializing issuer: secrets "brupop-root-ca-secret" not found |
| 3302 | 14:46:12 | warning | Node/ip-10-42-100-186.us-east-2.compute.internal: Rebooted — Node ip-10-42-100-186.us-east-2.compute.internal has been rebooted, boot id: 6436acb8-a1be-4339-8b6e-b6d04fbfa82c |
| 3308 | 14:46:12 | warning | Node/ip-10-42-101-194.us-east-2.compute.internal: InvalidDiskCapacity — invalid capacity 0 on image filesystem |
| 3318 | 14:46:12 | warning | Node/ip-10-42-101-194.us-east-2.compute.internal: Rebooted — Node ip-10-42-101-194.us-east-2.compute.internal has been rebooted, boot id: 6d868fcc-7686-460d-b665-6d2516d891b9 |
| 3324 | 14:46:12 | warning | Node/ip-10-42-101-90.us-east-2.compute.internal: InvalidDiskCapacity — invalid capacity 0 on image filesystem |
| 3335 | 14:46:12 | warning | Node/ip-10-42-101-90.us-east-2.compute.internal: Rebooted — Node ip-10-42-101-90.us-east-2.compute.internal has been rebooted, boot id: edad6a4f-2f72-4646-a370-f8ddebc2914f |
| 3341 | 14:46:12 | warning | Pod/brupop-agent-9j5vd: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-client-certificate" not found |
| 3351 | 14:46:12 | warning | Pod/brupop-agent-lg942: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-client-certificate" not found |
| 3357 | 14:46:12 | warning | Pod/brupop-agent-shbfh: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-client-certificate" not found |
| 3379 | 14:46:12 | warning | Pod/brupop-apiserver-75bd4784c4-9cc8s: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-certificate" not found |
| 3395 | 14:46:12 | warning | Pod/brupop-apiserver-75bd4784c4-rpn4n: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-certificate" not found |
| 3401 | 14:46:12 | warning | Pod/brupop-apiserver-75bd4784c4-rpn4n: Unhealthy — Readiness probe failed: Get "https://10.42.101.122:8443/ping": dial tcp 10.42.101.122:8443: connect: connection refused |
| 3403 | 14:46:12 | warning | Pod/brupop-apiserver-75bd4784c4-twmsq: FailedMount — MountVolume.SetUp failed for volume "bottlerocket-tls-keys" : secret "brupop-apiserver-certificate" not found |
| 3483 | 14:46:12 | warning | Pod/web-8cb9878f-6j82j: FailedScheduling — 0/3 nodes are available: 1 node(s) were unschedulable, 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 3484 | 14:46:12 | warning | Pod/web-8cb9878f-6j82j: FailedScheduling — 0/3 nodes are available: 1 node(s) didn't match pod topology spread constraints, 2 node(s) were unschedulable. no new claims to deallocate, preemption: 0/3 nodes are available: 1 No preemption victims found for incoming pod, 2 Preemption is not helpful for scheduling. |
| 3485 | 14:46:12 | warning | Pod/web-8cb9878f-6j82j: FailedScheduling — 0/3 nodes are available: 1 node(s) didn't match pod topology spread constraints, 1 node(s) had untolerated taint(s), 1 node(s) were unschedulable. no new claims to deallocate, preemption: 0/3 nodes are available: 1 No preemption victims found for incoming pod, 2 Preemption is not helpful for scheduling. |
| 3501 | 14:46:12 | warning | Pod/web-8cb9878f-hjtw9: FailedScheduling — 0/3 nodes are available: 1 node(s) were unschedulable, 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 3509 | 14:46:12 | warning | Pod/web-8cb9878f-zh67z: FailedScheduling — 0/3 nodes are available: 1 node(s) were unschedulable, 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 3510 | 14:46:12 | warning | Pod/web-8cb9878f-zh67z: FailedScheduling — 0/3 nodes are available: 1 node(s) had untolerated taint(s), 2 node(s) didn't match pod topology spread constraints. no new claims to deallocate, preemption: 0/3 nodes are available: 1 Preemption is not helpful for scheduling, 2 No preemption victims found for incoming pod. |
| 3568 | 14:46:12 | warning | Pod/aws-node-h7pv5: Unhealthy — Readiness probe errored and resulted in unknown state: rpc error: code = Unknown desc = failed to exec in container: container is in CONTAINER_EXITED state |
| 3584 | 14:46:12 | warning | Pod/aws-node-ldfvv: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3586 | 14:46:12 | warning | Pod/aws-node-lq2hs: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3588 | 14:46:12 | warning | Pod/aws-node-mbtwj: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3605 | 14:46:12 | warning | Pod/aws-node-nblm8: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3607 | 14:46:12 | warning | Pod/aws-node-pckkf: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3614 | 14:46:12 | warning | Pod/aws-node-pskf9: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3622 | 14:46:12 | warning | Pod/aws-node-rmdmg: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3638 | 14:46:12 | warning | Pod/coredns-5f8c6645d8-g5nr5: FailedCreatePodSandBox — Failed to create pod sandbox: rpc error: code = Unknown desc = failed to setup network for sandbox "a70b50879bc5520e36978115a8f9755aff95f33a2ebfa8f8642fccf4dd903c7c": plugin type="aws-cni" name="aws-cni" failed (add): add cmd: Error received from AddNetwork gRPC call: rpc error: code = Unavailable desc = connection error: desc = "transport: Error while dialing: dial tcp 127.0.0.1:50051: connect: connection refused" |
| 3643 | 14:46:12 | warning | Pod/coredns-5f8c6645d8-g5nr5: Unhealthy — Readiness probe failed: Get "http://10.42.100.33:8181/ready": dial tcp 10.42.100.33:8181: connect: connection refused |
| 3644 | 14:46:12 | warning | Pod/coredns-5f8c6645d8-g5nr5: Unhealthy — Readiness probe failed: HTTP probe failed with statuscode: 503 |
| 3651 | 14:46:12 | warning | Pod/coredns-5f8c6645d8-nchxv: Unhealthy — Readiness probe failed: Get "http://10.42.101.208:8181/ready": dial tcp 10.42.101.208:8181: connect: connection refused |
| 3656 | 14:46:12 | warning | Pod/coredns-5f8c6645d8-qdvn8: Unhealthy — Readiness probe failed: Get "http://10.42.100.79:8181/ready": dial tcp 10.42.100.79:8181: connect: connection refused |
| 3663 | 14:46:12 | warning | Pod/coredns-6cd89df5d6-4p222: Unhealthy — Readiness probe failed: Get "http://10.42.100.160:8181/ready": dial tcp 10.42.100.160:8181: connect: connection refused |
| 3670 | 14:46:12 | warning | Pod/coredns-6cd89df5d6-6qtqm: Unhealthy — Readiness probe failed: Get "http://10.42.101.142:8181/ready": dial tcp 10.42.101.142:8181: connect: connection refused |
| 3672 | 14:46:12 | warning | Pod/coredns-6cd89df5d6-vf4r2: FailedScheduling — 0/3 nodes are available: 3 node(s) had untolerated taint(s). no new claims to deallocate, preemption: 0/3 nodes are available: 3 Preemption is not helpful for scheduling. |
| 3678 | 14:46:12 | warning | Pod/coredns-6cd89df5d6-vf4r2: Unhealthy — Readiness probe failed: Get "http://10.42.101.81:8181/ready": dial tcp 10.42.101.81:8181: connect: connection refused |
| 3680 | 14:46:12 | warning | Pod/coredns-6cd89df5d6-w87j8: FailedScheduling — 0/3 nodes are available: 3 node(s) had untolerated taint(s). no new claims to deallocate, preemption: 0/3 nodes are available: 3 Preemption is not helpful for scheduling. |
| 3686 | 14:46:12 | warning | Pod/coredns-6cd89df5d6-w87j8: Unhealthy — Readiness probe failed: Get "http://10.42.101.253:8181/ready": dial tcp 10.42.101.253:8181: connect: connection refused |
| 3707 | 14:46:12 | warning | Pod/eks-pod-identity-agent-5hdct: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3728 | 14:46:12 | warning | Pod/eks-pod-identity-agent-82lwr: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3730 | 14:46:12 | warning | Pod/eks-pod-identity-agent-bs9m8: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3732 | 14:46:12 | warning | Pod/eks-pod-identity-agent-c5l9d: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3744 | 14:46:12 | warning | Pod/eks-pod-identity-agent-h8gks: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3746 | 14:46:12 | warning | Pod/eks-pod-identity-agent-htd2j: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3748 | 14:46:12 | warning | Pod/eks-pod-identity-agent-l9g7t: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3750 | 14:46:12 | warning | Pod/eks-pod-identity-agent-m9g9s: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3761 | 14:46:12 | warning | Pod/eks-pod-identity-agent-psg8p: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3803 | 14:46:12 | warning | Pod/kube-proxy-c7c9c: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3805 | 14:46:12 | warning | Pod/kube-proxy-dg5fv: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3807 | 14:46:12 | warning | Pod/kube-proxy-kbgt8: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3821 | 14:46:12 | warning | Pod/kube-proxy-tnsw7: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3823 | 14:46:12 | warning | Pod/kube-proxy-tqjzg: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3829 | 14:46:12 | warning | Pod/kube-proxy-w4h6f: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3831 | 14:46:12 | warning | Pod/kube-proxy-xmnf4: NodeShutdown — Pod was rejected as the node is shutting down. |
| 3847 | 14:46:12 | warning | ReplicaSet/brupop-controller-deployment-584c75f8c8: FailedCreate — Error creating: pods "brupop-controller-deployment-584c75f8c8-" is forbidden: no PriorityClass with name brupop-controller-high-priority was found |
| 3877 | 14:46:38 | run | run started: EKS Bottlerocket + Brupop demonstration |
| 3907 | 14:46:39 | node | node ip-10-42-101-194.us-east-2.compute.internal: ready=true cordoned=true |

## Caveats

None recorded.

_FleetForge observed this run. It did not perform any of the changes described: it holds no mutating Kubernetes client (ADR-0012)._
