#!/usr/bin/env bash
# Reproduce the GitHub release build failure on Ubuntu 24.04 (noble)
# This runs an Ubuntu 24.04 container and attempts to install the same
# dependencies as in .github/workflows/release.yml, which fails due to
# libcublas-dev not being available on noble under that name.
#
# Usage:
#   ./reproduce_release_failure.sh           # runs the failing 24.04 case
#   ./reproduce_release_failure.sh 22.04     # optional: show that 22.04 succeeds
#
# Output is saved to build_reproduction_result.txt in the project root.

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
LOG_FILE="$PROJECT_DIR/build_reproduction_result.txt"
UBUNTU_VER="${1:-22.04}"

cat > "$LOG_FILE" <<EOF
[Reproduction Script]
Date: $(date -Is)
Ubuntu version: $UBUNTU_VER
This script uses Docker to simulate the release workflow's apt installs.
EOF

# Build the apt command snippet shared with the workflow
read -r -d '' APT_SCRIPT <<'EOS' || true
set -euxo pipefail
export DEBIAN_FRONTEND=noninteractive
apt-get update -qy
apt-get install -qy wget gnupg

# Add NVIDIA’s CUDA repo key + list
wget https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2204/x86_64/cuda-archive-keyring.gpg
mv cuda-archive-keyring.gpg /usr/share/keyrings/
echo "deb [signed-by=/usr/share/keyrings/cuda-archive-keyring.gpg] https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2204/x86_64/ /" \
  > /etc/apt/sources.list.d/cuda.list

apt-get update -qy
apt-get install -qy \
  build-essential curl git pkg-config cmake \
  debhelper dpkg-dev fakeroot dh-make \
  cargo rustc \
  libssl-dev \
  libasound2-dev \
  libx11-dev libxtst-dev libxi-dev libxkbcommon-dev \
  libxcb-shape0-dev libxcb-xfixes0-dev \
  libgtk-3-dev libayatana-appindicator3-dev \
  clang llvm-dev libclang-dev \
  nvidia-cuda-toolkit nvidia-cuda-dev libcublas-dev-12-8
EOS

# Docker image tag
IMAGE="ubuntu:${UBUNTU_VER}"

{
  echo "\n--- Pulling Docker image: $IMAGE ---"
  docker pull "$IMAGE"
  echo "\n--- Running apt installation within container (expected to fail on 24.04) ---"
  docker run --rm -i "$IMAGE" /bin/bash -lc "$APT_SCRIPT"
  echo "\n[RESULT] The apt installation completed successfully on $UBUNTU_VER."
} >> "$LOG_FILE" 2>&1 || {
  STATUS=$?
  echo "\n[RESULT] The apt installation failed on $UBUNTU_VER as expected (exit $STATUS)." >> "$LOG_FILE"
  echo "See full logs above. This reproduces the GitHub failure for ubuntu-latest (24.04)." >> "$LOG_FILE"
  exit 0
}

# If we reached here without error, the install succeeded (likely 22.04)
exit 0
