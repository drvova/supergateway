variable "VERSION" {
  default = "DEV"
}

target "common" {
  context   = "."
  dockerfile = "docker/Dockerfile"
  platforms = ["linux/amd64", "linux/arm64"]
}

target "rust" {
  inherits = ["common"]
  tags = [
    "supercorp/supergateway:latest",
    "supercorp/supergateway:${VERSION}",
    "ghcr.io/supercorp-ai/supergateway:latest",
    "ghcr.io/supercorp-ai/supergateway:${VERSION}"
  ]
}

group "default" {
  targets = ["rust"]
}
