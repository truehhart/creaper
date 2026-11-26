resource "github_repository_environment" "this" {
  for_each = var.name == "_repository" ? toset([]) : toset([var.name])

  repository  = var.repository
  environment = each.value
}

resource "github_actions_secret" "this" {
  for_each = var.name == "_repository" ? local.secret_names : toset([])

  repository  = var.repository
  secret_name = each.value
  value       = var.secrets[each.value]
}

resource "github_actions_environment_secret" "this" {
  for_each = var.name == "_repository" ? toset([]) : local.secret_names

  repository  = var.repository
  environment = github_repository_environment.this[var.name].environment
  secret_name = each.value
  value       = var.secrets[each.value]
}
