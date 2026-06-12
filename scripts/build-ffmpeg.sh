#!/bin/bash
# FFmpeg 跨平台编译脚本
#
# 使用方法:
#   ./scripts/build-ffmpeg.sh <platform> <arch>
#
# 示例:
#   ./scripts/build-ffmpeg.sh macos aarch64
#   ./scripts/build-ffmpeg.sh linux x86_64
#   ./scripts/build-ffmpeg.sh windows x86_64

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'
log_info()  { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

if [ -z "$1" ] || [ -z "$2" ]; then
    log_error "Usage: $0 <platform> <arch>"
    echo ""
    echo "Supported:"
    echo "  macos    (aarch64, x86_64)"
    echo "  linux    (x86_64, aarch64)"
    echo "  windows  (x86_64)"
    exit 1
fi

PLATFORM=$1
ARCH=$2
PLATFORM_ID="${PLATFORM}-${ARCH}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
FFBUILD_DIR="${PROJECT_ROOT}/ffbuild/${PLATFORM_ID}"
FFMPEG_SRC="${PROJECT_ROOT}/ffmpeg"

log_info "Building FFmpeg for ${PLATFORM_ID}"

# ── 加载工具链配置 ──
ENV_SH="${PROJECT_ROOT}/ffbuild/env.sh"
if [ -f "${ENV_SH}" ]; then
    source "${ENV_SH}" "${PLATFORM}" "${ARCH}"
else
    log_info "env.sh not found, using defaults (gcc, linux)"
fi

# ── 变量默认值（env.sh 未设置时生效）──
CC="${CC:-gcc}"
CXX="${CXX:-g++}"
TARGET_OS="${TARGET_OS:-linux}"
EXTRA_CFLAGS="${EXTRA_CFLAGS:--O2}"
EXTRA_LDFLAGS="${EXTRA_LDFLAGS:-}"
CROSS_FLAG="${CROSS_FLAG:-}"
AR="${AR:-ar}"
NM="${NM:-nm}"
RANLIB="${RANLIB:-ranlib}"
STRIP="${STRIP:-strip}"
OBJCOPY="${OBJCOPY:-objcopy}"

# ── 获取/更新 FFmpeg 源码 ──
FFMPEG_BRANCH="n8.0"

if [ "${PS_SKIP_FFMPEG:-0}" = "1" ]; then
    log_info "PS_SKIP_FFMPEG=1 — skipping FFmpeg (Rust-only build)"
    exit 0
fi

if [ ! -d "${FFMPEG_SRC}" ]; then
    log_info "Cloning FFmpeg (${FFMPEG_BRANCH})..."
    git clone --branch "${FFMPEG_BRANCH}" --depth 1 https://github.com/FFmpeg/FFmpeg.git "${FFMPEG_SRC}"
else
    git config --global --add safe.directory "${FFMPEG_SRC}" 2>/dev/null || true
    log_info "Syncing FFmpeg (${FFMPEG_BRANCH})..."
    cd "${FFMPEG_SRC}"
    git fetch origin "refs/tags/${FFMPEG_BRANCH}:refs/tags/${FFMPEG_BRANCH}" --depth 1
    git checkout "tags/${FFMPEG_BRANCH}"

fi

# ── 平台特定 configure flags ──
CONFIGURE_FLAGS=(
  "--prefix=${FFBUILD_DIR}"
  "--cc=${CC}"
  "--cxx=${CXX}"
  "--ar=${AR}"
  "--ranlib=${RANLIB}"
  "--strip=${STRIP}"
  "--nm=${NM}"
  "--target-os=${TARGET_OS}"
  "--arch=${ARCH}"
  "--extra-cflags=${EXTRA_CFLAGS}"
  "--extra-ldflags=${EXTRA_LDFLAGS}"
  ${CROSS_FLAG:+"${CROSS_FLAG}"}
  "--enable-static"
  "--disable-shared"
  "--disable-everything"
  "--enable-small"
  "--enable-ffmpeg"
  # 实际需要的组件
  "--enable-protocol=tcp,rtmp"
  "--enable-demuxer=flv"
  "--enable-muxer=flv"
  "--enable-parser=h264"
  "--enable-network"
  # 禁用无用组件
  "--disable-filters"
  "--disable-swscale"
  "--disable-bsfs"
  "--disable-doc"
  "--disable-debug"
  "--disable-ffplay"
  "--disable-ffprobe"
  "--disable-iconv"
  "--disable-lzma"
  "--disable-bzlib"
  "--disable-zlib"
  "--disable-runtime-cpudetect"
)

# ── 构建 ──
mkdir -p "${FFBUILD_DIR}/lib" "${FFBUILD_DIR}/include"
cd "${FFMPEG_SRC}"

log_info "Configuring..."
echo "./configure ${CONFIGURE_FLAGS[*]}" | tr -s ' '
./configure "${CONFIGURE_FLAGS[@]}"

CPU_COUNT=$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)
log_info "Compiling (${CPU_COUNT} jobs)..."
make -j${CPU_COUNT}

log_info "Installing to ${FFBUILD_DIR}..."
make install

# ── 编译 pslinkb-stream ──
log_info "Building pslinkb-stream..."
"${CC}" -O2 -flto -march=x86-64-v3 \
    -I"${FFBUILD_DIR}/include" \
    -L"${FFBUILD_DIR}/lib" \
    "${PROJECT_ROOT}/src/ffmpeg/stream_copy.c" \
    -lavformat -lavcodec -lavutil \
    ${EXTRA_LDFLAGS} \
    -Wl,--gc-sections \
    -o "${FFBUILD_DIR}/bin/pslinkb-stream"

# ── 验证 ──
for lib in libavcodec libavformat libavutil; do
    if [ ! -f "${FFBUILD_DIR}/lib/${lib}.a" ]; then
        log_error "Missing: ${lib}.a"
        exit 1
    fi
done

if [ ! -f "${FFBUILD_DIR}/bin/pslinkb-stream" ]; then
    log_error "Missing: pslinkb-stream"
    exit 1
fi

log_info "Done: ${FFBUILD_DIR}"
echo "  ── pslinkb-stream: $(wc -c < "${FFBUILD_DIR}/bin/pslinkb-stream") bytes"
echo "  ── ffmpeg CLI: $(wc -c < "${FFBUILD_DIR}/bin/ffmpeg" 2>/dev/null || echo 'not built')"

log_info "Done: ${FFBUILD_DIR}"
echo "  export FFMPEG_PREFIX=${FFBUILD_DIR}"
