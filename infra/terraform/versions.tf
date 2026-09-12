# Pinned so a rebuild six months from now produces the same environment.
#
# Written in standard HCL with no Terraform-specific syntax, so it runs
# unchanged under OpenTofu (`tofu init && tofu plan`). That matters: Terraform
# moved to the Business Source License in 2023 and is no longer open source,
# while FleetForge is Apache-2.0 (ADR-0010). Keeping the configuration
# tool-agnostic costs nothing and leaves the choice open.
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

  # Every resource, every time. Tags are how this environment is found and
  # cleaned up, so they are set at the provider rather than per-resource where
  # one could be forgotten.
  default_tags {
    tags = {
      Project     = "FleetForge"
      Environment = "dev"
      ManagedBy   = "Terraform"
      Owner       = "Param"
    }
  }
}
