#!/usr/bin/env bash
# Reproducible dev environment for DiBillSion on Ubuntu 24.04 (native or WSL2).
# Idempotent: safe to re-run.
#
#   ./scripts/setup-ubuntu.sh              # everything (asks for sudo once for apt)
#   ./scripts/setup-ubuntu.sh --skip-apt   # only user-level tools (no sudo)
set -euo pipefail

NODE_MAJOR=24
NVM_VERSION=v0.40.3
JADX_MCP_VERSION=0.7.1
JADX_MCP_SHA256=6e5eacf500b64292bfb73c49797c1958f6ee44646e43e868039ae7feb573ff75
APKTOOL_VERSION=3.0.3
# Android (M3). SDK licenses accepted by the owner on 2026-10-25.
ANDROID_CMDLINE_TOOLS_BUILD=16111833
ANDROID_PLATFORM=android-37.0
ANDROID_BUILD_TOOLS=37.0.0
ANDROID_NDK=29.0.14206865          # r29
ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"

TOOLS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/dibillsion-tools"
BIN_DIR="$HOME/.local/bin"
SKIP_APT=false
[[ "${1:-}" == "--skip-apt" ]] && SKIP_APT=true

log() { printf '\n\033[1;34m==> %s\033[0m\n' "$*"; }
mkdir -p "$TOOLS_DIR" "$BIN_DIR"

# --- 1. System packages (sudo) ----------------------------------------------
if ! $SKIP_APT; then
  log "apt packages"
  sudo apt-get update
  sudo apt-get install -y --no-install-recommends \
    build-essential pkg-config libssl-dev ca-certificates curl git jq \
    unzip zip procps openjdk-21-jdk-headless
fi

# --- 2. Rust ----------------------------------------------------------------
log "Rust (rustup)"
if ! command -v rustup >/dev/null 2>&1 && [[ ! -x "$HOME/.cargo/bin/rustup" ]]; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
# shellcheck disable=SC1091
source "$HOME/.cargo/env"
rustup update stable
rustup component add rustfmt clippy
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android

log "Cargo tools (prebuilt via cargo-binstall)"
if ! command -v cargo-binstall >/dev/null 2>&1; then
  curl -L --proto '=https' --tlsv1.2 -sSf \
    https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
fi
cargo binstall -y --locked cargo-nextest cargo-deny cargo-llvm-cov cargo-ndk cargo-bloat

# --- 3. Node.js (for REA) ---------------------------------------------------
log "Node.js $NODE_MAJOR (nvm)"
export NVM_DIR="$HOME/.nvm"
if [[ ! -s "$NVM_DIR/nvm.sh" ]]; then
  curl -o- "https://raw.githubusercontent.com/nvm-sh/nvm/$NVM_VERSION/install.sh" | bash
fi
# shellcheck disable=SC1091
source "$NVM_DIR/nvm.sh"
nvm install "$NODE_MAJOR"
nvm alias default "$NODE_MAJOR"

# --- 4. Study tools: jadx-headless-mcp + Apktool + adb -----------------------
log "jadx-headless-mcp $JADX_MCP_VERSION"
JAR="$TOOLS_DIR/jadx-headless-mcp-$JADX_MCP_VERSION-all.jar"
if [[ ! -f "$JAR" ]]; then
  curl -fL -o "$JAR.part" \
    "https://github.com/1013503897/jadx-headless-mcp/releases/download/v$JADX_MCP_VERSION/jadx-headless-mcp-$JADX_MCP_VERSION-all.jar"
  echo "$JADX_MCP_SHA256  $JAR.part" | sha256sum -c -
  mv "$JAR.part" "$JAR"
fi

log "Apktool $APKTOOL_VERSION"
APKJAR="$TOOLS_DIR/apktool_$APKTOOL_VERSION.jar"
if [[ ! -f "$APKJAR" ]]; then
  curl -fL -o "$APKJAR.part" \
    "https://github.com/iBotPeaches/Apktool/releases/download/v$APKTOOL_VERSION/apktool_$APKTOOL_VERSION.jar"
  mv "$APKJAR.part" "$APKJAR"
fi
printf '#!/usr/bin/env bash\nexec java -jar "%s" "$@"\n' "$APKJAR" > "$BIN_DIR/apktool"
chmod +x "$BIN_DIR/apktool"

log "Android platform-tools (adb)"
if [[ ! -x "$TOOLS_DIR/platform-tools/adb" ]] && command -v unzip >/dev/null 2>&1; then
  curl -fL -o "$TOOLS_DIR/pt.zip" https://dl.google.com/android/repository/platform-tools-latest-linux.zip
  unzip -q -o "$TOOLS_DIR/pt.zip" -d "$TOOLS_DIR" && rm "$TOOLS_DIR/pt.zip"
fi
[[ -x "$TOOLS_DIR/platform-tools/adb" ]] && ln -sf "$TOOLS_DIR/platform-tools/adb" "$BIN_DIR/adb"

# --- 4b. Android SDK + NDK (M3) ----------------------------------------------
log "Android SDK (cmdline-tools $ANDROID_CMDLINE_TOOLS_BUILD, $ANDROID_PLATFORM, NDK $ANDROID_NDK)"
SDKMANAGER="$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager"
if [[ ! -x "$SDKMANAGER" ]]; then
  mkdir -p "$ANDROID_HOME/cmdline-tools"
  curl -fL -o "$TOOLS_DIR/cmdline-tools.zip" \
    "https://dl.google.com/android/repository/commandlinetools-linux-${ANDROID_CMDLINE_TOOLS_BUILD}_latest.zip"
  rm -rf "$ANDROID_HOME/cmdline-tools/latest" "$ANDROID_HOME/cmdline-tools/cmdline-tools"
  unzip -q "$TOOLS_DIR/cmdline-tools.zip" -d "$ANDROID_HOME/cmdline-tools"
  mv "$ANDROID_HOME/cmdline-tools/cmdline-tools" "$ANDROID_HOME/cmdline-tools/latest"
  rm "$TOOLS_DIR/cmdline-tools.zip"
fi
# Accepting the SDK licenses is the owner's decision (CLAUDE.md); approved 2026-10-25.
yes | "$SDKMANAGER" --sdk_root="$ANDROID_HOME" --licenses >/dev/null || true
"$SDKMANAGER" --sdk_root="$ANDROID_HOME" --install \
  "platform-tools" "platforms;$ANDROID_PLATFORM" "build-tools;$ANDROID_BUILD_TOOLS" "ndk;$ANDROID_NDK"

# --- 5. Environment ---------------------------------------------------------
log "Shell environment (~/.bashrc)"
MARK="# >>> dibillsion >>>"
if ! grep -q "$MARK" "$HOME/.bashrc"; then
  cat >> "$HOME/.bashrc" <<EOF

$MARK
export PATH="\$HOME/.local/bin:\$PATH"
export REA_JADX_MCP_JAR="$JAR"
export REA_APKTOOL_COMMAND="$BIN_DIR/apktool"
export REA_JADX_OPERATION_TIMEOUT_MS=600000
export REA_JADX_HEAP_MIB=4096
# <<< dibillsion <<<
EOF
fi
ANDROID_MARK="# >>> dibillsion android >>>"
if ! grep -q "$ANDROID_MARK" "$HOME/.bashrc"; then
  cat >> "$HOME/.bashrc" <<EOF

$ANDROID_MARK
export ANDROID_HOME="$ANDROID_HOME"
export ANDROID_NDK_HOME="\$ANDROID_HOME/ndk/$ANDROID_NDK"
export PATH="\$ANDROID_HOME/cmdline-tools/latest/bin:\$PATH"
# <<< dibillsion android <<<
EOF
fi

log "Versions"
rustc --version; cargo --version; node --version
java -version 2>&1 | head -1
command -v javac >/dev/null && javac -version || echo "javac missing: run without --skip-apt"
command -v adb >/dev/null && adb --version | head -1 || true

cat <<'EOF'

Done. Open a new shell (or `source ~/.bashrc`).
Next (REA):   npx rea-agents setup     # registers the MCP server for Claude Code
Android:      ANDROID_HOME and ANDROID_NDK_HOME are set in ~/.bashrc.
EOF
