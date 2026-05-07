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

target "backend" {
  context    = "."
  dockerfile = "docker/Dockerfile.backend"
  platforms  = PLATFORMS
  tags = [
    "${AR_REGION}-docker.pkg.dev/${GCP_PROJECT_ID}/${AR_REPO}/backend:${TAG}",
  ]
  cache-from = ["type=gha,scope=backend"]
  cache-to   = ["type=gha,mode=max,scope=backend"]
}

target "frontend" {
  context    = "."
  dockerfile = "docker/Dockerfile.frontend"
  platforms  = PLATFORMS
  tags = [
    "${AR_REGION}-docker.pkg.dev/${GCP_PROJECT_ID}/${AR_REPO}/frontend:${TAG}",
  ]
  cache-from = ["type=gha,scope=frontend"]
  cache-to   = ["type=gha,mode=max,scope=frontend"]
}

group "default" {
  targets = ["backend", "frontend"]
}