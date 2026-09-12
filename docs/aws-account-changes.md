# AWS account-level changes

Every change made to AWS account **771965334314** outside the FleetForge
Terraform stacks, with its cleanup procedure.

Terraform-managed resources are not listed here — `terraform destroy` handles
those. This file covers the account-level bootstrap, which Terraform does not
own and which therefore has no automatic cleanup.

Nothing in this file costs money. AWS Organizations, IAM Identity Center, and
AWS Budgets (first two) are all free.

---

## 1. IAM Identity Center account instance — CREATED, then DELETED

| | |
| --- | --- |
| When | 2026-09-12 |
| What | `arn:aws:sso:::instance/ssoins-66841e1b4af4488c`, identity store `d-9a675c7881` |
| Why | An attempt to get federated credentials without an AWS Organization |
| Outcome | **Removed.** Account instances do not support permission sets — `CreatePermissionSet` returns `ValidationException: This operation is not supported for account instances`. They exist for application integrations, not AWS account access |
| Cleanup | Already done. Verified `list-instances` returns 0 |

An identity-store user (`param`, `madan.pa@northeastern.edu`, id
`01ab7550-90c1-7043-cf43-619bce751950`) was created alongside it and deleted
with it. Dependency check before deletion: 0 applications, 0 groups, 1 user.

## 2. AWS Organization — CREATED

| | |
| --- | --- |
| When | 2026-09-12 |
| What | `o-qpabyw2zdv`, feature set **ALL**, management account 771965334314 |
| Root | `r-k193`, **no policies attached** |
| Accounts | Exactly 1 — the existing account. None invited, none created |
| Why | An organization instance of IAM Identity Center is the only way to get permission sets, and permission sets are the only way to get federated temporary credentials for AWS account access |

**Cleanup:**

```bash
# Only possible when the organization contains just the management account.
aws organizations delete-organization --profile <profile>
```

Deleting an organization is irreversible in the sense that the organization ID
is not reissued, but it has no effect on the account itself: the account remains
standalone, as it was before. There are no member accounts to remove first, and
no service control policies to detach.

**Caveats worth knowing before deleting:**

- Anything relying on organization-level features stops working. Nothing here
  does, beyond IdC.
- IAM Identity Center must be deleted first, or the organization delete fails.

## 3. IdC trusted service access — ENABLED

| | |
| --- | --- |
| What | `sso.amazonaws.com` enabled as a trusted service on the organization |
| Why | Documented prerequisite for IAM Identity Center |

**Cleanup:**

```bash
aws organizations disable-aws-service-access \
  --service-principal sso.amazonaws.com --profile <profile>
```

Do this after deleting the IdC instance, not before.

## 4. IAM Identity Center organization instance — PENDING, console only

Enabling an organization instance has **no public API**. `sso-admin
create-instance` exists but is for standalone account instances and explicitly
refuses a management account:

```
ValidationException: Organization management account is not allowed to
                     perform the operation.
```

It must be enabled once from the console. Everything after it is scriptable.

**Cleanup, when the demonstration is finished:**

```bash
# 1. Remove the account assignment
aws sso-admin delete-account-assignment --instance-arn <arn> \
  --target-id 771965334314 --target-type AWS_ACCOUNT \
  --permission-set-arn <ps-arn> --principal-type USER --principal-id <user-id>

# 2. Delete the permission set
aws sso-admin delete-permission-set --instance-arn <arn> \
  --permission-set-arn <ps-arn>

# 3. Delete the identity-store user
aws identitystore delete-user --identity-store-id <store-id> --user-id <user-id>

# 4. Disable IdC itself — console only:
#    IAM Identity Center -> Settings -> Management -> Delete
```

Deleting IdC also removes the `AWSReservedSSO_*` IAM roles it created in the
account. Those roles are not Terraform-managed and are not visible in any plan.

## 5. AWS Budgets — PENDING

`infra/terraform/guardrails/` creates two budgets. Terraform-managed, but kept
in a **separate stack** deliberately so they outlive `terraform destroy` of the
cluster — a budget alarm torn down with the thing it watches protects nothing.

**Cleanup:**

```bash
terraform -chdir=infra/terraform/guardrails destroy
```

Do this last, after the cluster is gone and the final bill has settled.

---

## Full teardown order

Reverse of creation. Each step assumes the previous one completed.

| # | Step | Command | ~Time |
| --- | --- | --- | --- |
| 1 | Delete any Kubernetes Services of type LoadBalancer | `kubectl delete svc -A --field-selector spec.type=LoadBalancer` | 1 min |
| 2 | Destroy the cluster | `./scripts/eks-down.sh` | **15–20 min** |
| 3 | Verify nothing survived | queries printed by that script | 2 min |
| 4 | Destroy the budgets | `terraform -chdir=infra/terraform/guardrails destroy` | 1 min |
| 5 | Remove IdC assignment, permission set, user | commands in section 4 | 2 min |
| 6 | Disable IdC | console | 2 min |
| 7 | Disable IdC trusted access | `aws organizations disable-aws-service-access ...` | 1 min |
| 8 | Delete the organization | `aws organizations delete-organization` | 1 min |

Step 1 matters more than it looks. A load balancer created by a Kubernetes
Service is made by the cloud controller, not Terraform — Terraform does not know
it exists, will not delete it, and the VPC delete then stalls on
`DependencyViolation` for about twenty minutes before failing with the cluster
half gone.

**Steps 4 onward are optional.** They cost nothing to leave in place. Step 2 is
the only one that stops money being spent.
