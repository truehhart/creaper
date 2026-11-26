include "root" {
  path = "${get_repo_root()}/infra/stacks/root.hcl"
}

include "provider" {
  path = find_in_parent_folders("root.hcl")
}

terraform {
  source = "${get_repo_root()}/infra/modules/github/environment"
}

locals {
  secrets = {
    for name, value in yamldecode(sops_decrypt_file("${get_terragrunt_dir()}/secrets.sops.yaml")) :
    name => value if value != null
  }
}

inputs = {
  repository = "creaper"
  name       = "_repository"
  secrets    = local.secrets
}
