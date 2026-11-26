generate "provider" {
  path      = "provider.generated.tf"
  if_exists = "overwrite_terragrunt"

  contents = <<-EOF
    provider "github" {
      owner = "truehhart"
    }
  EOF
}
