locals {
  unit_path  = split("/", get_path_from_repo_root())
  provider   = element(local.unit_path, 2)
  env        = element(local.unit_path, 3)
  stack      = join("/", slice(local.unit_path, 4, length(local.unit_path)))
  state_file = "${get_repo_root()}/infra/stacks/state.sops.yaml"
  state      = yamldecode(sops_decrypt_file(local.state_file))
}

remote_state {
  backend = "http"

  generate = {
    path      = "backend.tf"
    if_exists = "overwrite_terragrunt"
  }

  config = {
    address       = "${local.state.state_par_base_url}${local.provider}/${local.env}/${local.stack}.tfstate"
    update_method = "PUT"
  }

  encryption = {
    key_provider = "pbkdf2"
    passphrase   = local.state.state_encryption_passphrase
  }
}
