# Cost guardrails for the FleetForge demonstration account.
#
# A SEPARATE stack from the cluster, on purpose. These must outlive
# `terraform destroy` of the demonstration environment — a budget alarm that is
# torn down along with the thing it was watching protects nothing.
#
# Apply once and leave it. It costs nothing: AWS Budgets is free for the first
# two budgets per account.

terraform {
  required_version = "~> 1.16"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 6.0"
    }
  }
}

provider "aws" {
  region  = var.region
  profile = var.aws_profile

  default_tags {
    tags = {
      Project     = "FleetForge"
      Environment = "dev"
      ManagedBy   = "Terraform"
      Owner       = "Param"
    }
  }
}

variable "aws_profile" {
  description = "Named AWS CLI profile."
  type        = string
}

variable "region" {
  description = "AWS region."
  type        = string
  default     = "us-east-2"
}

variable "alert_email" {
  description = "Address to send budget alerts to."
  type        = string
}

variable "credit_balance_usd" {
  description = "Promotional credits available. Thresholds are derived from this."
  type        = number
  default     = 120
}

locals {
  # Thresholds as a fraction of the credit balance. Deliberately starting low:
  # on a free-plan account the useful alert is the one that arrives while there
  # is still time to act, not the one at 90%.
  thresholds = {
    "early warning"   = 0.10 # ~$12 — roughly two days of the lean profile
    "quarter spent"   = 0.25 # ~$30 — about four days
    "half spent"      = 0.50 # ~$60 — decide whether to tear down
    "nearly depleted" = 0.80 # ~$96 — tear down now
  }
}

resource "aws_budgets_budget" "monthly" {
  name         = "fleetforge-monthly"
  budget_type  = "COST"
  limit_amount = tostring(var.credit_balance_usd)
  limit_unit   = "USD"
  time_unit    = "MONTHLY"

  # THE important setting on a credit-funded account.
  #
  # By default a cost budget reports spend NET of credits. With $120 of credits
  # covering everything, that reads $0.00 — so the budget never fires, right up
  # until the credits run out and account access ends. Excluding credits makes
  # the budget track gross usage, which is the number that actually predicts
  # when the credits will be gone.
  cost_types {
    include_credit             = false
    include_refund             = false
    include_discount           = true
    include_subscription       = true
    include_support            = true
    include_tax                = true
    include_upfront            = true
    include_recurring          = true
    include_other_subscription = true
    use_amortized              = false
    use_blended                = false
  }

  dynamic "notification" {
    for_each = local.thresholds
    content {
      comparison_operator        = "GREATER_THAN"
      threshold                  = notification.value * 100
      threshold_type             = "PERCENTAGE"
      notification_type          = "ACTUAL"
      subscriber_email_addresses = [var.alert_email]
    }
  }

  # Forecast catches a cluster left running: actual spend lags, but the forecast
  # sees the burn rate immediately.
  notification {
    comparison_operator        = "GREATER_THAN"
    threshold                  = 60
    threshold_type             = "PERCENTAGE"
    notification_type          = "FORECASTED"
    subscriber_email_addresses = [var.alert_email]
  }
}

# A daily budget is the one that actually catches a forgotten cluster. The
# monthly budget takes days to cross a threshold; this fires the morning after.
resource "aws_budgets_budget" "daily" {
  name         = "fleetforge-daily"
  budget_type  = "COST"
  limit_amount = "8"
  limit_unit   = "USD"
  time_unit    = "DAILY"

  cost_types {
    include_credit = false
  }

  notification {
    comparison_operator        = "GREATER_THAN"
    threshold                  = 100
    threshold_type             = "PERCENTAGE"
    notification_type          = "ACTUAL"
    subscriber_email_addresses = [var.alert_email]
  }
}

output "budgets" {
  description = "What was created, and what it will and will not do."
  value = {
    monthly = aws_budgets_budget.monthly.name
    daily   = aws_budgets_budget.daily.name
    note = join(" ", [
      "Budgets ALERT. They do not stop anything.",
      "Nothing here caps spend, terminates instances, or protects the account:",
      "the only thing that stops the meter is ./scripts/eks-down.sh.",
      "Tags do not stop resources either.",
    ])
  }
}
