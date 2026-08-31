#!/bin/sh
# Copyright (C) 2026 Red Hat, Inc.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
# http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#
# SPDX-License-Identifier: Apache-2.0
#
# Build and export the root filesystem for the build microVM.
# Requires Podman. The resulting rootfs has buildah installed and is
# pre-configured to use the vfs storage driver (required on virtiofs).
#
# Usage: ./make-rootfs.sh [OUTPUT_DIR]
#   OUTPUT_DIR  where to write the rootfs  (default: ./vm-rootfs)
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOTFS="${1:-./vm-rootfs}"
IMAGE="openshell-vm-rootfs"

if ! command -v podman >/dev/null 2>&1; then
    echo "error: podman is required to build the rootfs" >&2
    exit 1
fi

echo "Building rootfs image for linux/arm64..."
podman build \
    --platform linux/arm64 \
    --tag "$IMAGE" \
    --file "$SCRIPT_DIR/Containerfile" \
    "$SCRIPT_DIR"

echo "Exporting rootfs to $ROOTFS..."
mkdir -p "$ROOTFS"
CONTAINER="$(podman create --platform linux/arm64 "$IMAGE")"
podman export "$CONTAINER" | tar -C "$ROOTFS" -x
podman rm "$CONTAINER" >/dev/null

# podman create may mount the host's /etc/resolv.conf into the container,
# overwriting the one baked into the image. Write it explicitly after export.
# use-vc forces TCP for DNS queries — TSI routes TCP but may not route UDP.
printf 'nameserver 1.1.1.1\noptions use-vc\n' > "$ROOTFS/etc/resolv.conf"

echo ""
echo "Rootfs ready at: $ROOTFS"
echo ""
echo "Build an image with it:"
echo "  openshell-image-builder --runtime vm --vm-rootfs $ROOTFS myimage:latest"
