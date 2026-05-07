variable "GCP_PROJECT_ID" {
  default = "YOUR_PROJECT_ID"
}

variable "AR_REGION" {
  default = "us"
}

variable "AR_REPO" {
  default = "fullstack-template"
}

variable "TAG" {
  default = "latest"
}

variable "PLATFORMS" {
  default = ["linux/amd64", "linux/arm64"]
}

function "registry_prefix" {
  params = []
  result = "${var.AR_REGION}-docker.pkg.dev/${var.GCP_PROJECT_ID}/${var.AR_REPO}"
}

target "backend" {
  context    = "."
  dockerfile = "docker/Dockerfile.backend"
  platforms  = var.PLATFORMS
  tags = [
    "${registry_prefix()}/backend:${var.TAG}",
  ]
  cache-from = ["type=gha,scope=backend"]
  cache-to   = ["type=gha,mode=max,scope=backend"]
}

target "frontend" {
  context    = "."
  dockerfile = "docker/Dockerfile.frontend"
  platforms  = var.PLATFORMS
  tags = [
    "${registry_prefix()}/frontend:${var.TAG}",
  ]
  cache-from = ["type=gha,scope=frontend"]
  cache-to   = ["type=gha,mode=max,scope=frontend"]
}

group "default" {
  targets = ["backend", "frontend"]
}