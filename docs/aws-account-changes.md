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

## 4. IAM Identity Center organization instance — CREATED (built, not in use)

| | |
| --- | --- |
| Instance | `arn:aws:sso:::instance/ssoins-6684ec564eebf36b` |
| Identity store | `d-9a675c79ed` |
| Portal | `https://d-9a675c79ed.awsapps.com/start` |
| Primary region | `us-east-2` (single region — the auto-created us-west-2 replica was removed) |
| Encryption | CUSTOMER_MANAGED_KEY, a multi-Region key in us-east-2 (`mrk-…`; look it up with the command below) |
| User | `param` / `a1eb75c0-b0b1-70f0-d0ac-8b72c4c7b7fd` / madan.pa@northeastern.edu |
| Permission set | `FleetForgeAdmin` / `ps-b71fbf90878f8b19`, **PT2H**, AdministratorAccess |
| Assignment | `SUCCEEDED` |
| **IAM role created by the assignment** | **`AWSReservedSSO_FleetForgeAdmin_7ab4c86b3b7a9bea`** — IdC-managed, in `aws-reserved/sso.amazonaws.com/us-east-2/`. Not Terraform-managed and not visible in any plan |
| CLI profile | `fleetforge-admin` in `~/.aws/config` (appended; a timestamped backup of the prior file is alongside it) |

**Status: complete but unused.** Sign-in is blocked by the instance's MFA policy —
"if a user does not yet have a registered MFA device: block their sign-in" rather than
"require them to register at sign-in". Changing that one setting in
**IdC → Settings → Authentication → Multi-factor authentication** is all that stands between this
and working. Terraform is running under root instead, by explicit decision.

### us-west-2 replica — REMOVED

IdC replicated itself to us-west-2 automatically, 4.4 seconds after creation. Removed with
`sso-admin remove-region`; converged in ~90s. `remove-region` does **not** clean up the KMS
replica it created — that was scheduled separately, see below.

### KMS

Find the key ID — it is account-specific, so it is not written down here:

```bash
aws sso-admin describe-instance --instance-arn <arn> --region us-east-2 \
  --query 'EncryptionConfigurationDetails.KmsKeyArn'
```

| Region | Key | State |
| --- | --- | --- |
| us-east-2 | `mrk-…` (PRIMARY) | **Enabled — do not delete.** IdC encrypts live data with it |
| us-west-2 | same key ID (REPLICA) | **PendingDeletion, 2026-09-19T20:23:39-04:00** |

Multi-Region key replicas bill independently at ~$1/month each, so this was ~$2/month.
The us-west-2 replica is still billed until the window elapses; cancel with
`aws kms cancel-key-deletion --key-id <key-id> --region us-west-2`.

**Cleanup order at teardown: delete IdC first, then its KMS key.** Deleting the key while IdC
still uses it leaves an instance that can be neither used nor cleanly removed.

## 4b. Original console-only note (kept for the record)

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

## 6. Root access key — CREATED 2026-09-13, **MUST BE DELETED**

| | |
| --- | --- |
| When | 2026-09-13, for the Phase C live EKS run |
| Why | The Identity Center path (`fleetforge-admin`) is blocked by the MFA policy in section 4, and the operator chose a root key over changing that setting |
| Key id | `AKIA3HPFYGMVJFEQ56QU` |
| Profile | `fleetforge-root` in `~/.aws/credentials` |

**This is the highest-priority cleanup item in this file.** A root access key has
no permission boundary and no expiry. It was used for one 1.78-hour cluster run
and has no further purpose.

**Cleanup:**

```bash
# Console: PM (771965334314) -> Security credentials -> Access keys
#          -> AKIA3HPFYGMVJFEQ56QU -> Actions -> Delete
aws configure --profile fleetforge-root set aws_access_key_id ""      # then remove the
aws configure --profile fleetforge-root set aws_secret_access_key ""  # profile by hand
```

The secret was also pasted into a chat transcript on 2026-09-13, so deleting the
key is the only action that actually closes that exposure.

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
| 9 | **Delete the root access key** | console, section 6 above | 1 min |

Step 1 matters more than it looks. A load balancer created by a Kubernetes
Service is made by the cloud controller, not Terraform — Terraform does not know
it exists, will not delete it, and the VPC delete then stalls on
`DependencyViolation` for about twenty minutes before failing with the cluster
half gone.

**Steps 4 onward are optional.** They cost nothing to leave in place. Step 2 is
the only one that stops money being spent.
