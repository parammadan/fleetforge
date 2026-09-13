# Public-release audit

**Date:** 2026-09-13 · **Question:** can `parammadan/fleetforge` be made public as it stands?

**Answer: not as-is.** No secrets are present, but the repository contains
personal and account-identifying metadata that should be removed first. The
work is small — five files, one of which should be deleted from the working tree
entirely — and it is listed below.

**Nothing in this audit has been published or made public.** No repository
visibility was changed.

---

## 1. Secrets — none found

| Check | Result |
| --- | --- |
| `gitleaks detect` over all 39 commits (8.8 MB) | **no leaks found** |
| AWS access keys (`AKIA…` / `ASIA…`) in tree or evidence | none |
| Private keys (`BEGIN … PRIVATE KEY`) | none |
| Kubeconfig material (`client-key-data`, `client-certificate-data`, `certificate-authority-data`) | none |
| Bearer tokens / JWTs | none. The one JWT-shaped string is a deliberately non-JWT test fixture |
| `.env`, `*.kubeconfig`, SSO cache | not tracked; blocked in `.gitignore` |
| Every artifact fetched through the replay API, swept for the above | clean — asserted by `the_real_bundle_serves_no_credential_material` |

The distinction that matters for the rest of this document: **a secret grants
access; identifying metadata does not.** Everything below is the second kind. It
is not exploitable on its own; it narrows an attacker's search and it exposes
personal information.

---

## 2. Identifying metadata — present, and it should go

### 2.1 Personal data — the strongest reason not to publish yet

| Item | Where |
| --- | --- |
| `madan.pa@northeastern.edu` | `docs/aws-account-changes.md:25, 82` |
| Identity Center user id `a1eb75c0-b0b1-70f0-d0ac-8b72c4c7b7fd` | `docs/aws-account-changes.md:82` |

Git commits use `86084060+parammadan@users.noreply.github.com`, so the personal
address is **not otherwise exposed** by this repository. Publishing would newly
expose it, and it is a real, in-use address.

### 2.2 AWS account and organisation identifiers

| Item | Where |
| --- | --- |
| Account `771965334314` | `STATUS.md`, `infra/terraform/README.md`, `docs/aws-account-changes.md`, `evidence/eks-recovery/00-CONCLUSIONS.md` |
| `arn:aws:iam::771965334314:user/pennydata-sink` | `STATUS.md:167`, `infra/terraform/README.md:131` |
| Organization `o-qpabyw2zdv` | `docs/aws-account-changes.md:34` |
| Identity Center `ssoins-66841e1b4af4488c`, `ssoins-6684ec564eebf36b` | `docs/aws-account-changes.md:20, 77` |
| Identity store `d-9a675c7881` | `docs/aws-account-changes.md:20` |

AWS does not classify an account id as a secret — it appears in every ARN you
hand to a partner. But it is the seed for targeted reconnaissance: role-name
enumeration, S3 bucket guessing, and cross-account trust probing all start from
one. Combined with a known IAM user name (`pennydata-sink`) it is materially
more useful to an attacker than either alone.

`docs/aws-account-changes.md` is the single worst file: it is a complete
inventory of the account's identity configuration, written as a cleanup runbook.
It is exactly the document you would want if you were attacking the account, and
it has **no value to a public reader** — it exists so its author can undo what he
set up.

### 2.3 Infrastructure identifiers in the evidence bundle

| Item | Count | Assessment |
| --- | --- | --- |
| Public EC2 IPs `18.216.231.41`, `18.222.119.23`, `3.12.197.77` | 3 | **Low.** The cluster was destroyed on 2026-09-13; AWS has long since reassigned these to other tenants. Publishing them points at strangers' machines, which is the reason to remove them, not the risk to you. |
| `ec2-18-222-119-23.us-east-2.compute.amazonaws.com` and peers | 3 | Same, derived from the above |
| EC2 instance ids `i-…` | 393 occurrences | **Negligible.** Meaningless without the account, and the instances are terminated |
| Private VPC addresses `10.42.x.x` | 28 | **None.** RFC 1918, unroutable, and the VPC no longer exists |
| Cluster name `fleetforge-demo`, region `us-east-2` | — | **None.** Both are already in the public Terraform |

### 2.4 Checked and cleared

Seven further twelve-digit numbers looked like account ids and are not:
`953330819851`, `791953938558`, `632025540692`, `341869455021`, `931997501262`
are the trailing segments of Kubernetes object UUIDs; `240120725504` is an
ephemeral-storage byte count in a fixture. `602401143452` is AWS's own public
ECR registry for EKS add-ons in `us-east-2` and is documented by AWS.

---

## 3. Sanitisation procedure

Three tiers, because they carry different risks.

### Tier 1 — documentation and status (do this; it is safe and reversible)

`docs/aws-account-changes.md` is a private operational runbook. **Remove it from
the working tree** and keep it outside the repository:

```sh
mkdir -p ~/fleetforge-private
git mv docs/aws-account-changes.md ~/fleetforge-private/   # or: git rm --cached
printf 'docs/aws-account-changes.md\n' >> .gitignore
```

Then redact the remaining references, which are prose rather than data:

| File | Change |
| --- | --- |
| `STATUS.md:167` | `arn:aws:iam::771965334314:user/pennydata-sink` → `arn:aws:iam::<account>:user/<s3-scoped-user>` |
| `infra/terraform/README.md:131` | same substitution |
| `evidence/eks-recovery/00-CONCLUSIONS.md:3` | `Account 771965334314, us-east-2` → `Account <redacted>, us-east-2` |

None of these carries a claim that depends on the literal value. The account id
is context, not evidence.

> **`00-CONCLUSIONS.md` is inside the evidence bundle**, so editing it changes
> its SHA-256 — see Tier 3.

### Tier 2 — public IPs in the evidence bundle (optional)

Three addresses in three artifacts. They belong to somebody else now. A
mechanical substitution preserving shape and distinctness:

```sh
# 18.222.119.23 → 203.0.113.11, 18.216.231.41 → 203.0.113.12, 3.12.197.77 → 203.0.113.13
# 203.0.113.0/24 is TEST-NET-3 (RFC 5737): reserved for documentation, never routable.
```

Applied to `06-nodes-before.json`, `22-node-194-before-uncordon.json`,
`25-nodes-final.json`, and the matching `ec2-*.compute.amazonaws.com` names.

Instance ids and private addresses should be **left alone**: they are load-bearing
in the replay (node identity, pod placement) and carry no risk.

### Tier 3 — the provenance problem, stated plainly

The artifact manifest stores the SHA-256 each file had **when captured**, and the
interface shows it so a reader can prove the artifact in front of them is the
one that was recorded. That is the guarantee the whole evidence explorer rests
on.

Editing any file under `evidence/` breaks it. There are two ways out and they
are not equivalent:

**(a) Sanitise, then re-hash.** The bundle is internally consistent and the
interface works. But the hashes no longer match the original capture, so they
prove only that nobody edited the file *after sanitisation*. The chain back to
the cluster is cut, and a reader cannot tell.

**(b) Publish the evidence unmodified.** Provenance is intact end to end. Three
stale public IPs go out with it.

**Recommendation: (b) for the bundle, Tier 1 for everything else.**

The evidence bundle is the only part of this repository whose value depends on
being unedited. Three addresses on terminated instances in a destroyed VPC are
not worth cutting that chain for — and a project whose entire argument is *we do
not edit captured evidence to make it look better* should not edit captured
evidence to make it look better.

If Tier 2 is done anyway, **do both of these or neither**: re-hash the manifest,
and add a `DataCaveat` to the bundle stating that the artifacts were sanitised,
what was changed, and that the hashes are post-sanitisation. A silently re-hashed
bundle is worse than either honest option.

The original bundle stays private regardless:

```sh
cp -R evidence/eks-recovery ~/fleetforge-private/eks-recovery-original
```

---

## 4. Verdict

| | |
| --- | --- |
| **Public as-is?** | **No** |
| **Public after Tier 1?** | **Yes** |
| Effort | Four edits and one file moved out of the tree |
| Blocked on | Nothing technical. It is a decision about the personal email and the account inventory |

Note also that the repository's **existing history** contains all of the above.
Making it public exposes every commit, not the tip. Removing these from history
would require a rewrite, which is explicitly out of scope here — so Tier 1 must
be understood as *stops it getting worse and cleans the current tree*, not *erases
it*. If history-level removal matters, that is a separate, deliberate operation
to decide on before publishing rather than after.

---

## 5. What this audit did not do

- Change repository visibility. Nothing was published.
- Modify any file under `evidence/`.
- Rewrite git history or create tags.
- Apply any of the Tier 1 edits. They are proposed, not made.
