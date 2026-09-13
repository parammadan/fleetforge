# Validating the networking hypothesis

The one claim in the replay that is neither observed nor derived, and what it
would take to settle it.

## What is actually known

| | |
| --- | --- |
| **Observed** | A post-recovery traffic check failed: 42 of 139 requests succeeded (30.2%), 14:53:24 → 14:58:58. Source: `evidence/eks-recovery/31-traffic-post-recovery.txt` |
| **Observed** | At the time of that check all three `web` pods were Running and Ready, all three Service endpoints were Ready, and `kube-proxy` and the VPC CNI were healthy on every node with zero restarts. Sources: `32-kube-system-final.txt`, `33-kube-proxy.log`, `34-aws-node.log` |
| **Observed** | The VPC CNI add-on was installed *after* the managed node group had already joined, because EKS module v21 installs no add-ons by default. |
| **Unverified** | That the second fact causes the first. |

## The hypothesis

A success rate near one-in-three against a three-endpoint Service is what you
would see if only the endpoint on the *same node as the client* were reachable
and the two cross-node endpoints were not. Pods that started before the CNI was
configured can end up with networking that works locally and not across nodes.

It fits the number and it fits the install order. It has never been tested.

## Why it was not tested

The cluster was destroyed immediately after the capture, under the standing rule
that no AWS resource outlives its demonstration. That was the right call for
cost and blast radius and it is the reason this section exists.

## What would settle it

Four experiments, cheapest first. The first two would probably be decisive.

### 1. Per-endpoint probe — decisive, costs nothing but a cluster

Bypass the Service and address each pod IP directly from a known client pod:

```sh
CLIENT=$(kubectl -n demo get pod -l app=traffic -o name | head -1)
for ip in $(kubectl -n demo get pod -l app=web -o jsonpath='{.items[*].status.podIP}'); do
  NODE=$(kubectl -n demo get pod -l app=web -o json \
        | jq -r --arg ip "$ip" '.items[]|select(.status.podIP==$ip)|.spec.nodeName')
  echo -n "$ip ($NODE): "
  kubectl -n demo exec "$CLIENT" -- wget -q -T 2 -t 1 -O /dev/null "http://$ip" \
    && echo reachable || echo UNREACHABLE
done
```

**Predicts:** same-node endpoint reachable, the other two not. If all three are
reachable the hypothesis is dead and the 30.2% needs another explanation.

Record which node the client is on. The prediction is *specifically* that
reachability tracks co-location, not that two random endpoints fail.

### 2. Controlled comparison — decisive about causation

Stand up two clusters from the same Terraform, differing only in add-on
ordering:

* **A:** `addons = { vpc-cni = { before_compute = true }, … }` — the correction
  already written in `infra/terraform/`.
* **B:** add-ons installed after the node group, reproducing the original.

Run the bounded sampler (`-T 2 -t 1`, ~2,000 requests) against both.

**Predicts:** A passes, B lands near 33%. Anything else — both pass, both fail,
B passes — refutes it.

This is the experiment that would let the Terraform correction be described as
*verified* rather than *written*. Cost: roughly $5.50/day per cluster at the
sizing in `infra/terraform/README.md`; both can be torn down within the hour.

### 3. Packet capture — explains the mechanism

On a node with an unreachable endpoint, while the probe from (1) runs:

```sh
kubectl debug node/<node> -it --image=nicolaka/netshoot -- \
  tcpdump -ni any "host <pod-ip> and port 80"
```

Also dump the conntrack and iptables state:

```sh
iptables-save -t nat | grep -A5 KUBE-SVC
conntrack -L | grep <pod-ip>
```

**Distinguishes:** packets leaving and not arriving (routing / ENI), packets
arriving and not answered (pod-side), no packets at all (kube-proxy rules never
programmed).

### 4. Rebuild-in-place — cheapest, weakest

On cluster B, delete and recreate the `web` pods *after* the CNI is healthy. If
they become reachable without touching anything else, the problem was pods
started before the CNI was configured — which is the hypothesis, narrowed.

Weakest because it changes two things at once (pod age and pod identity), so it
supports rather than proves.

## Until then

The interface calls this `UNVERIFIED` in the claim list, in the limitations
panel, and in the demo script's answer to the question. The Terraform correction
is described as "implemented in Terraform and not verified on a fresh cluster",
which is exactly what it is.

**This plan requires creating AWS resources and has not been run.** Doing so
needs explicit approval in-session, per the standing rules.
