#!/bin/sh

set -eu

GH_API="https://api.github.com/repos/urlynn/PSLinkB/releases/latest"
POST_INSTALL_URL="https://raw.githubusercontent.com/urlynn/PSLinkB/main/openwrt/pslinkb/post-install"

# ── 输出 ───────────────────────────────────

info() { printf '\033[1;34m[INFO]\033[0m  %s\n' "$*"; }
warn() { printf '\033[1;33m[WARN]\033[0m  %s\n' "$*"; }
err()  { printf '\033[1;31m[ERR]\033[0m  %s\n' "$*" >&2; }
ok()   { printf '\033[1;32m[OK]\033[0m    %s\n' "$*"; }
die()  { err "$*"; exit 1; }

# ── 检测 ───────────────────────────────────

detect_arch() {
	distrib_arch=$(grep '^DISTRIB_ARCH=' /etc/openwrt_release 2>/dev/null | cut -d"'" -f2)
	[ -z "$distrib_arch" ] && die "无法读取 DISTRIB_ARCH (非 OpenWRT?)"
	case "$distrib_arch" in
		x86_64)
			ARCH="x86_64"; METHOD="pkg" ;;
		aarch64_generic)
			ARCH="aarch64"; METHOD="pkg" ;;
		aarch64_cortex-a53|aarch64_cortex-a72|aarch64_cortex-a76)
			ARCH="aarch64"; METHOD="targz" ;;
		*)
			die "不支持的架构: $distrib_arch (仅支持 x86_64 / aarch64_generic / aarch64_cortex-a53/a72/a76)" ;;
	esac
	DISTRIB_ARCH_VAL="$distrib_arch"
}

detect_pkg_manager() {
	if command -v apk >/dev/null 2>&1; then echo apk
	elif command -v opkg >/dev/null 2>&1; then echo opkg
	else die "未找到 apk/opkg"; fi
}

detect_web_variant() {
	if [ "$(uci -q get nginx.global.uci_enable)" = "true" ]; then
		echo nginx
	else
		echo uhttpd
	fi
}

# ── GitHub API ────────────────────────────

fetch_release_json() {
	wget -q -O- --timeout=30 \
		--header="Accept: application/vnd.github+json" \
		"$GH_API" 2>/dev/null || die "GitHub API 不可达"
}

_json_compact() {
	tr -d '\n' | sed 's/" *: *"/":"/g'
}

parse_tag() {
	echo "$1" | _json_compact | grep -o '"tag_name":"[^"]*"' | head -1 | sed 's/"tag_name":"//;s/"$//'
}

parse_asset_urls() {
	echo "$1" | _json_compact | grep -o '"browser_download_url":"[^"]*"' | sed 's/"browser_download_url":"//;s/"$//'
}

# ── 下载 ───────────────────────────────────

download() {
	url="$1"; dst="$2"
	info "下载: $url"
	wget -q -O "$dst" --timeout=120 "$url" || die "下载失败: $url"
}

# ── 安装: pslinkb ──────────────────────────

install_pslinkb_pkg() {
	url="$1"; pm="$2"
	ext=$(echo "$url" | sed 's/.*\.//')
	dl="/tmp/pslinkb-install.$$.${ext}"
	download "$url" "$dl"
	/etc/init.d/pslinkb stop >/dev/null 2>&1 || true
	if [ "$pm" = "apk" ]; then
		out=$(apk add --allow-untrusted "$dl" 2>&1) || { rm -f "$dl"; die "apk 安装 pslinkb 失败:\n$out"; }
	else
		out=$(opkg install "$dl" 2>&1) || { rm -f "$dl"; die "opkg 安装 pslinkb 失败:\n$out"; }
	fi
	rm -f "$dl"
	ok "pslinkb 安装成功"
}

install_pslinkb_targz() {
	url="$1"
	dl="/tmp/pslinkb-install.$$.tar.gz"
	download "$url" "$dl"
	/etc/init.d/pslinkb stop >/dev/null 2>&1 || true
	tar xzf "$dl" -C /
	rm -f "$dl"

	pi="/tmp/pslinkb-post-install.$$"
	download "$POST_INSTALL_URL" "$pi"
	sh "$pi" || warn "post-install 脚本执行失败"
	rm -f "$pi"
	ok "pslinkb 安装成功 (tar.gz)"
}

# ── 安装: luci ─────────────────────────────

install_luci() {
	url="$1"; pm="$2"
	ext=$(echo "$url" | sed 's/.*\.//')
	dl="/tmp/luci-install.$$.${ext}"
	download "$url" "$dl"
	if [ "$pm" = "apk" ]; then
		out=$(apk add --allow-untrusted "$dl" 2>&1) || { rm -f "$dl"; die "luci-app-pslinkb 安装失败:\n$out"; }
	else
		out=$(opkg install "$dl" 2>&1) || {
			warn "opkg install luci 失败, 尝试 --add-arch all:9"
			out=$(opkg --add-arch all:9 install "$dl" 2>&1) || { rm -f "$dl"; die "luci-app-pslinkb 安装失败:\n$out"; }
		}
	fi
	rm -f "$dl"
	ok "luci-app-pslinkb 安装成功"
}

# ── 主流程 ─────────────────────────────────

main() {
	info "PSLinkB 一键安装 (OpenWRT)"

	detect_arch
	pm=$(detect_pkg_manager)
	variant=$(detect_web_variant)

	info "DISTRIB_ARCH=$DISTRIB_ARCH_VAL  架构=$ARCH  安装方式=$METHOD  包管理器=$pm  luci后端=$variant"

	if [ "$pm" = "opkg" ]; then
		info "opkg update..."
		opkg update >/dev/null 2>&1 || warn "opkg update 失败"
	fi

	info "查询 GitHub latest release..."
	json=$(fetch_release_json)
	tag=$(parse_tag "$json")
	ver=${tag#v}
	[ -z "$ver" ] && die "无法解析版本号"
	info "最新版本: $ver (tag: $tag)"

	urls=$(parse_asset_urls "$json")
	[ -z "$urls" ] && die "Release 无 assets"

	if [ "$pm" = "apk" ]; then ext=apk; else ext=ipk; fi

	info "=== 安装 pslinkb ==="
	if [ "$METHOD" = "targz" ]; then
		pslinkb_url=$(echo "$urls" | grep "pslinkb-${ver}-openwrt-aarch64\.tar\.gz$" | head -1)
		[ -z "$pslinkb_url" ] && die "未找到 pslinkb tar.gz: pslinkb-${ver}-openwrt-aarch64.tar.gz"
		install_pslinkb_targz "$pslinkb_url"
	else
		pslinkb_url=$(echo "$urls" | grep "pslinkb-${ver}-openwrt-${ARCH}\.${ext}$" | head -1)
		[ -z "$pslinkb_url" ] && die "未找到 pslinkb 包: pslinkb-${ver}-openwrt-${ARCH}.${ext}"
		install_pslinkb_pkg "$pslinkb_url" "$pm"
	fi

	info "=== 安装 luci-app-pslinkb-${variant} ==="
	luci_url=$(echo "$urls" | grep "luci-app-pslinkb-[0-9][0-9.]*-${variant}\.${ext}$" | head -1)
	[ -z "$luci_url" ] && die "未找到 luci 包: luci-app-pslinkb-*-${variant}.${ext}"
	install_luci "$luci_url" "$pm"

	rm -rf /tmp/luci-* 2>/dev/null || true
	info "重启 web server..."
	if [ "$variant" = "nginx" ]; then
		/etc/init.d/uhttpd restart >/dev/null 2>&1 || true
		/etc/init.d/nginx restart >/dev/null 2>&1 || true
	else
		/etc/init.d/uhttpd restart >/dev/null 2>&1 || true
	fi

	/etc/init.d/pslinkb enable >/dev/null 2>&1 || true
	/etc/init.d/pslinkb start >/dev/null 2>&1 || true

	ok "PSLinkB $ver 安装完成"
	info "访问 LuCI: Services -> PSLinkB"
}

main "$@"
