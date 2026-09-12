# Brupop CRD, for exercising the collection path

## What this is

`crd.yaml` is the **real upstream CustomResourceDefinition**, extracted verbatim from
[bottlerocket-update-operator v1.8.0](https://github.com/bottlerocket-os/bottlerocket-update-operator/releases/tag/v1.8.0)
(`bottlerocket-update-operator-v1.8.0.yaml`). Group `brupop.bottlerocket.aws`, kind
`BottlerocketShadow`, storage version `v2`.

## What this is not

**Brupop is not installed.** Only the CRD is applied. The operator is not running, and could not
do anything useful if it were: this is a `kind` cluster with Debian nodes, and Brupop manages
Bottlerocket hosts.

The `BottlerocketShadow` resources in `shadows.yaml` are **hand-written**. They are real objects
in a real API server — FleetForge collects them over a real watch, with real UIDs and
resourceVersions — but they describe a fiction. No Bottlerocket node exists, and no update is
happening.

## So what does this actually prove?

That FleetForge's Brupop **collection path** works against a real API server: the dynamic watch,
the normalizer, the `BrupopFact` model, the transition recording, and the distinction between
`not installed` and `forbidden`.

It proves **nothing** about observing a real Brupop update. That needs Bottlerocket nodes and a
running operator, which is Milestone 5 on EKS.

That distinction is the entire point of the data-mode discipline, so it would be absurd to blur
it here. Anything sourced from these objects is `LIVE` in the strict sense — it really is the
current state of the API server — and it is still not evidence that Brupop did anything.

## Use

```bash
kubectl apply -f infra/local/brupop/crd.yaml
kubectl apply -f infra/local/brupop/shadows.yaml
kubectl delete -f infra/local/brupop/shadows.yaml   # when finished
```
