#!/usr/bin/env bash
set -euo pipefail

# ── Defaults ────────────────────────────────────────────────────
IMAGE_NAME="zeaphoo/oximqtt"
IMAGE_TAG="latest"
PLATFORMS="linux/amd64,linux/arm64"
PUSH=false
LOAD=false
OUTPUT_DIR=""
NO_CACHE=false
BUILDER_NAME="oximqtt-builder"

# ── Usage ───────────────────────────────────────────────────────
usage() {
  cat <<'EOF'
Usage: ./build.sh [OPTIONS]

Build multi-architecture OXIMQTT container images via Docker buildx.

Options:
  --image NAME          Image name        (default: zeaphoo/oximqtt)
  --tag TAG             Image tag         (default: latest)
  --platform LIST       Target platforms  (default: linux/amd64,linux/arm64)
  --push                Build and push to registry
  --load                Build and load into local Docker (single platform only)
  --output DIR          Extract compiled binaries to local directory
  --no-cache            Build without Docker layer cache
  -h, --help            Show this help

Examples:
  # Build multi-arch and push
  ./build.sh --tag v0.22.0 --push

  # Build for current platform, load locally
  ./build.sh --load

  # Extract binaries for both architectures
  ./build.sh --output ./dist

  # Build arm64 only, load locally
  ./build.sh --platform linux/arm64 --load
EOF
}

# ── Parse arguments ─────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case $1 in
    --image)    IMAGE_NAME="$2";  shift 2 ;;
    --tag)      IMAGE_TAG="$2";   shift 2 ;;
    --platform) PLATFORMS="$2";   shift 2 ;;
    --push)     PUSH=true;        shift ;;
    --load)     LOAD=true;        shift ;;
    --output)   OUTPUT_DIR="$2";  shift 2 ;;
    --no-cache) NO_CACHE=true;    shift ;;
    -h|--help)  usage; exit 0 ;;
    *) echo "Unknown option: $1"; usage; exit 1 ;;
  esac
done

FULL_TAG="${IMAGE_NAME}:${IMAGE_TAG}"

# ── Validate ────────────────────────────────────────────────────
if $PUSH && $LOAD; then
  echo "Error: --push and --load cannot be combined" >&2
  exit 1
fi

if [[ -n "$OUTPUT_DIR" ]] && ($PUSH || $LOAD); then
  echo "Error: --output cannot be combined with --push or --load" >&2
  exit 1
fi

if $LOAD; then
  IFS=',' read -ra PLAT_ARR <<< "$PLATFORMS"
  if [[ ${#PLAT_ARR[@]} -gt 1 ]]; then
    echo "Error: --load supports only a single platform. Use --platform linux/amd64 or linux/arm64" >&2
    exit 1
  fi
fi

# ── Preflight checks ───────────────────────────────────────────
if ! command -v docker &>/dev/null; then
  echo "Error: docker is not installed" >&2; exit 1
fi

if ! docker buildx version &>/dev/null; then
  echo "Error: docker buildx is not available" >&2; exit 1
fi

# ── Setup buildx builder ───────────────────────────────────────
setup_builder() {
  if ! docker buildx inspect "$BUILDER_NAME" &>/dev/null; then
    echo "→ Creating buildx builder: $BUILDER_NAME"
    docker buildx create --name "$BUILDER_NAME" --driver docker-container --use
  else
    docker buildx use "$BUILDER_NAME"
  fi

  # Bootstrap QEMU emulators for cross-architecture builds
  local current_platform
  current_platform="linux/$(docker info --format '{{.Architecture}}')"
  IFS=',' read -ra PLAT_ARR <<< "$PLATFORMS"
  for plat in "${PLAT_ARR[@]}"; do
    if [[ "$plat" != "$current_platform" ]]; then
      echo "→ Cross-build detected ($current_platform → $plat), ensuring QEMU is available"
      docker run --privileged --rm tonistiigi/binfmt --install "${plat##*/}" 2>/dev/null || true
      break
    fi
  done

  docker buildx inspect --bootstrap &>/dev/null
}

# ── Build common args ───────────────────────────────────────────
build_args=(
  --platform "$PLATFORMS"
  -t "$FULL_TAG"
)

if $NO_CACHE; then
  build_args+=(--no-cache)
fi

# ── Execute ─────────────────────────────────────────────────────
if [[ -n "$OUTPUT_DIR" ]]; then
  # ── Binary extraction mode ──────────────────────────────────
  mkdir -p "$OUTPUT_DIR"
  setup_builder

  echo "→ Building and extracting binaries to $OUTPUT_DIR"
  echo "  Platforms: $PLATFORMS"

  IFS=',' read -ra PLAT_ARR <<< "$PLATFORMS"
  for plat in "${PLAT_ARR[@]}"; do
    arch="${plat##*/}"
    dest="${OUTPUT_DIR}/${arch}"
    mkdir -p "$dest"

    echo "→ Extracting binary for $plat → $dest/"
    docker buildx build \
      "${build_args[@]}" \
      --target binaries \
      --platform "$plat" \
      --output "type=local,dest=$dest" \
      .

    if [[ -f "$dest/oximqttd" ]]; then
      chmod +x "$dest/oximqttd"
      echo "  ✓ $dest/oximqttd ($(du -h "$dest/oximqttd" | cut -f1))"
    else
      echo "  ✗ Binary not found in $dest/" >&2
    fi
  done

  echo "→ Done. Binaries extracted to $OUTPUT_DIR/"

elif $PUSH; then
  # ── Push mode ───────────────────────────────────────────────
  setup_builder

  echo "→ Building and pushing $FULL_TAG"
  echo "  Platforms: $PLATFORMS"

  docker buildx build \
    "${build_args[@]}" \
    --push \
    .

  echo "→ Done. Pushed $FULL_TAG"

elif $LOAD; then
  # ── Load mode (single platform, local docker) ───────────────
  echo "→ Building $FULL_TAG for $PLATFORMS"

  docker buildx build \
    "${build_args[@]}" \
    --load \
    .

  echo "→ Done. Loaded $FULL_TAG"

else
  # ── Default: build multi-arch image (no output) ─────────────
  setup_builder

  echo "→ Building $FULL_TAG"
  echo "  Platforms: $PLATFORMS"

  docker buildx build \
    "${build_args[@]}" \
    .

  echo "→ Done. Built $FULL_TAG (use --push to push, --load to load locally, --output to extract binaries)"
fi
