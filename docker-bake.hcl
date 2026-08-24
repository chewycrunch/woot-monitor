# Build definition for `docker buildx bake`, driven by the Taskfile.
#
# This is deliberately not a compose file: nothing here describes a runnable stack.
# The images are built and pushed from this repo and run elsewhere, so `services`,
# ports and environment have no meaning — only build inputs and output tags do.

variable "REGISTRY" {
  default = "ghcr.io/chewycrunch/woot-monitor"
}

# Override to publish a versioned tag, e.g. `TAG=v1.5.0 task push`.
variable "TAG" {
  default = "latest"
}

group "default" {
  targets = ["monitor", "tls-client"]
}

target "monitor" {
  context    = "./monitor"
  dockerfile = "Dockerfile"
  tags       = ["${REGISTRY}/monitor:${TAG}"]

  # Rust/musl cannot cross-compile here — each muslrust image only carries its own
  # arch's toolchain — so the non-native arch builds under emulation and is slow.
  # Publish from CI with a native runner per arch if that becomes painful.
  platforms = ["linux/amd64", "linux/arm64"]
}

target "tls-client" {
  context    = "./tls-client"
  dockerfile = "Dockerfile"
  tags       = ["${REGISTRY}/tls-client:${TAG}"]

  # Go cross-compiles, so both arches are cheap to build.
  platforms = ["linux/amd64", "linux/arm64"]
}
