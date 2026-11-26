variable "repository" {
  description = "The GitHub repository name."
  type        = string

  validation {
    condition     = trimspace(var.repository) != ""
    error_message = "The repository name must not be empty."
  }
}

variable "name" {
  description = "The GitHub environment name."
  type        = string

  validation {
    condition     = trimspace(var.name) != ""
    error_message = "The environment name must not be empty."
  }
}

variable "secrets" {
  description = "The GitHub Actions secrets by name."
  type        = map(string)
  default     = {}
  sensitive   = true
}

locals {
  secret_names = nonsensitive(toset(keys(var.secrets)))
}
