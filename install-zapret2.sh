#!/bin/sh
#
# zapret2 — установщик пакетов для OpenWrt
#
# Использование:
#   sh install-zapret2.sh [версия] [опции]
#
# Примеры:
#   sh install-zapret2.sh
#   sh install-zapret2.sh 1.0.0-r1
#   sh install-zapret2.sh v1.0.0-r1 --start
#   sh install-zapret2.sh --no-luci --yes
#   sh install-zapret2.sh --lang-ru
#
# Запуск с роутера:
#   wget -qO- https://raw.githubusercontent.com/alex0310/zapret2-openwrt/main/install-zapret2.sh | sh
#   wget -qO- .../install-zapret2.sh | sh -s -- --start
#
# Переменные окружения:
#   ZAPRET2_VERSION=1.0.0-r1
#   ZAPRET2_YES=1
#   ZAPRET2_LANG=ru
#

REPO="alex0310/zapret2-openwrt"
PKG_LOG="${ZAPRET2_PKG_LOG:-/tmp/zapret2-pkg.log}"

VERSION="${ZAPRET2_VERSION:-}"
WITH_LUCI=""
START=0
FORCE=0
NO_UPDATE=0
ALLOW_UNSUPPORTED=0
DRY_RUN=0
ASSUME_YES=0
LANG_RU=""
START_FAILED=0

PKG_MGR=""
PKG_FMT=""
OWRT=""
OWRT_USE=""
ARCH=""
DISTRIB_ARCH=""
ASSET=""
ASSET_URL=""
WORKDIR=""
EXTRACT_DIR=""
ZIP_PATH=""
ZAPRET_PKG=""
LUCI_PKG=""
INSTALL_LUCI=0
PKG_INDEX_READY=0
DL=""
UNZIP=""

case "${ZAPRET2_YES:-0}" in
	1|yes|YES|true|TRUE) ASSUME_YES=1 ;;
	*) ASSUME_YES=0 ;;
esac

case "${ZAPRET2_LANG:-}" in
	ru|RU|1|yes|YES|true|TRUE) LANG_RU=1 ;;
	0|no|NO|false|FALSE|none|NONE) LANG_RU=0 ;;
	'') LANG_RU="" ;;
	*) LANG_RU="" ;;
esac

log() { printf '%s\n' "$*"; }
warn() { printf '%s\n' "ПРЕДУПРЕЖДЕНИЕ: $*" >&2; }
die() {
	printf '%s\n' "ОШИБКА: $*" >&2
	exit 1
}

have() { command -v "$1" >/dev/null 2>&1; }

usage() {
	cat <<'EOF'
zapret2 — установщик для OpenWrt

Использование:
  sh install-zapret2.sh [версия] [опции]

Аргументы:
  версия                 Версия пакетов, например 1.0.0-r1 или v1.0.0-r1.
                         По умолчанию — последний релиз GitHub.

Опции:
  --no-luci              Установить только zapret2, без LuCI.
  --with-luci            Установить LuCI, даже если luci-base не найден.
  --start                Включить и сразу запустить сервис zapret2.
  --lang-ru              Установить русский язык LuCI без вопроса.
  --no-lang-ru           Не предлагать и не устанавливать русский язык.
  --yes, -y              Не задавать вопросов. Для языка используется ответ
                         по умолчанию — «нет»; для явной установки добавьте
                         --lang-ru.
  --force                Переустановить выбранные пакеты.
  --no-update            Не выполнять opkg update / apk update.
  --allow-unsupported    Не спрашивать подтверждение для близкой версии
                         OpenWrt 24.10.x / 25.12.x (с предупреждением).
  --dry-run              Показать план установки, ничего не меняя.
  -h, --help             Показать эту справку.

Переменные окружения:
  ZAPRET2_VERSION        Версия пакетов.
  ZAPRET2_YES=1          Не задавать вопросов.
  ZAPRET2_LANG=ru        Установить русский язык LuCI.
  ZAPRET2_PKG_LOG        Путь к логу пакетного менеджера.

Поддерживаемые комбинации:
  24.10.5  mipsel_24kc          IPK
  24.10.5  mips_24kc            IPK
  24.10.5  aarch64_cortex-a53   IPK
  25.12.6  mipsel_24kc          APK
  25.12.6  mips_24kc            APK
  25.12.6  aarch64_cortex-a53   APK
EOF
}

print_supported() {
	cat <<'EOF'
Поддерживаемые комбинации:
  24.10.5  mipsel_24kc          IPK
  24.10.5  mips_24kc            IPK
  24.10.5  aarch64_cortex-a53   IPK
  25.12.6  mipsel_24kc          APK
  25.12.6  mips_24kc            APK
  25.12.6  aarch64_cortex-a53   APK
EOF
}

cleanup() {
	[ -n "$WORKDIR" ] && [ -d "$WORKDIR" ] && rm -rf "$WORKDIR"
}

validate_version() {
	case "$1" in
		''|*[!0-9A-Za-z._-]*) return 1 ;;
		[0-9]*) ;;
		*) return 1 ;;
	esac
	case "$1" in
		*..*|*.) return 1 ;;
	esac
	return 0
}

parse_args() {
	while [ $# -gt 0 ]; do
		case "$1" in
			-h|--help)
				usage
				exit 0
				;;
			--no-luci) WITH_LUCI=0 ;;
			--with-luci) WITH_LUCI=1 ;;
			--start) START=1 ;;
			--lang-ru) LANG_RU=1 ;;
			--no-lang-ru) LANG_RU=0 ;;
			--yes|-y) ASSUME_YES=1 ;;
			--force) FORCE=1 ;;
			--no-update) NO_UPDATE=1 ;;
			--allow-unsupported) ALLOW_UNSUPPORTED=1 ;;
			--dry-run) DRY_RUN=1 ;;
			--)
				shift
				break
				;;
			-*)
				die "Неизвестная опция: $1"
				;;
			*)
				if [ -n "$VERSION" ]; then
					die "Версия указана дважды: $VERSION и $1"
				fi
				VERSION="$1"
				;;
		esac
		shift
	done

	if [ $# -gt 0 ]; then
		if [ -n "$VERSION" ]; then
			die "Лишние аргументы: $*"
		fi
		VERSION="$1"
		shift
	fi
	[ $# -eq 0 ] || die "Лишние аргументы: $*"

	VERSION="${VERSION#v}"
	if [ -n "$VERSION" ]; then
		validate_version "$VERSION" || die "Некорректная версия: $VERSION"
	fi
}

ask_yes_no() {
	prompt="$1"
	default="$2"
	answer=""

	if [ "$ASSUME_YES" = 1 ]; then
		[ "$default" = yes ]
		return $?
	fi

	if [ ! -r /dev/tty ]; then
		return 1
	fi

	printf '%s ' "$prompt" >/dev/tty
	IFS= read -r answer < /dev/tty || answer=""
	[ -n "$answer" ] || answer="$default"

	case "$answer" in
		y|Y|yes|YES|Yes|д|Д|да|ДА|Да) return 0 ;;
		*) return 1 ;;
	esac
}

require_openwrt() {
	[ -f /etc/openwrt_release ] || die "Это не OpenWrt: не найден /etc/openwrt_release"
	. /etc/openwrt_release
	OWRT="${DISTRIB_RELEASE:-}"
	DISTRIB_ARCH="${DISTRIB_ARCH:-}"
	[ -n "$OWRT" ] || die "Не удалось определить версию OpenWrt"
}

detect_pkg_mgr() {
	if have apk && [ -f /etc/apk/arch ]; then
		PKG_MGR=apk
		PKG_FMT=apk
	elif have opkg; then
		PKG_MGR=opkg
		PKG_FMT=ipk
	else
		die "Не найден apk или opkg"
	fi
}

detect_arch() {
	ARCH=""
	if [ "$PKG_MGR" = apk ] && [ -r /etc/apk/arch ]; then
		ARCH=$(cat /etc/apk/arch 2>/dev/null | tr -d '\r\n')
	fi
	if [ -z "$ARCH" ] && [ -n "$DISTRIB_ARCH" ]; then
		ARCH="$DISTRIB_ARCH"
	fi
	if [ -z "$ARCH" ] && [ "$PKG_MGR" = apk ]; then
		ARCH=$(apk --print-arch 2>/dev/null)
	fi
	if [ -z "$ARCH" ] && [ "$PKG_MGR" = opkg ]; then
		ARCH=$(opkg print-architecture 2>/dev/null | awk '$1 == "arch" && $2 != "all" && $2 != "noarch" { print $2; exit }')
	fi
	[ -n "$ARCH" ] || die "Не удалось определить архитектуру пакетов"
}

resolve_owrt() {
	OWRT_USE="$OWRT"
	case "$OWRT" in
		24.10.5|25.12.6)
			return 0
			;;
	esac

	case "$OWRT" in
		24.10|24.10.*) OWRT_USE=24.10.5 ;;
		25.12|25.12.*) OWRT_USE=25.12.6 ;;
		*) die "Нет пакетов для OpenWrt $OWRT. Поддерживаются только 24.10.5 и 25.12.6." ;;
	esac

	warn "Версия OpenWrt $OWRT устарела; использую пакеты для $OWRT_USE."
	warn "Рекомендуется обновить OpenWrt до $OWRT_USE."

	[ "$ALLOW_UNSUPPORTED" = 1 ] && return 0

	if [ "$DRY_RUN" = 1 ]; then
		warn "Режим --dry-run: продолжаю без подтверждения."
		return 0
	fi

	if [ "$ASSUME_YES" = 1 ]; then
		die "Для продолжения без вопросов укажите --allow-unsupported"
	fi

	if ! ( : < /dev/tty ) 2>/dev/null || ! ( : > /dev/tty ) 2>/dev/null; then
		die "Нет терминала для подтверждения. Укажите --allow-unsupported."
	fi

	printf 'Нажмите Enter, чтобы продолжить, или Ctrl+C для отмены: ' >/dev/tty
	IFS= read -r answer </dev/tty || die "Установка отменена"
	printf '\n' >/dev/tty 2>/dev/null || true
	return 0
}

asset_name() {
	case "$2:$3:$4" in
		24.10.5:mipsel_24kc:ipk)
			printf '%s\n' "zapret2-openwrt-$1-24.10.5.zip"
			;;
		24.10.5:mips_24kc:ipk)
			printf '%s\n' "zapret2-openwrt-$1-mips_24kc-24.10.5.zip"
			;;
		24.10.5:aarch64_cortex-a53:ipk)
			printf '%s\n' "zapret2-openwrt-$1-aarch64_cortex-a53-24.10.5.zip"
			;;
		25.12.6:mipsel_24kc:apk)
			printf '%s\n' "zapret2-openwrt-$1-25.12.6-apk.zip"
			;;
		25.12.6:mips_24kc:apk)
			printf '%s\n' "zapret2-openwrt-$1-mips_24kc-25.12.6-apk.zip"
			;;
		25.12.6:aarch64_cortex-a53:apk)
			printf '%s\n' "zapret2-openwrt-$1-aarch64_cortex-a53-25.12.6-apk.zip"
			;;
		*)
			return 1
			;;
	esac
}

detect_downloader() {
	if have curl; then
		DL=curl
	elif have wget; then
		DL=wget
	elif have uclient-fetch; then
		DL=uclient-fetch
	else
		DL=""
	fi
}

fetch_url() {
	case "$DL" in
		curl) curl -fsSL --connect-timeout 15 --max-time 30 "$1" ;;
		wget) wget -q -T 30 -O - "$1" ;;
		uclient-fetch) uclient-fetch -q -T 30 -O - "$1" ;;
		*) return 1 ;;
	esac
}

download_file() {
	case "$DL" in
		curl) curl -fsSL --connect-timeout 15 --max-time 180 -o "$2" "$1" ;;
		wget) wget -q -T 30 -O "$2" "$1" ;;
		uclient-fetch) uclient-fetch -q -T 180 -O "$2" "$1" ;;
		*) return 1 ;;
	esac
}

latest_version() {
	json=""
	atom=""
	tag=""
	location=""

	json=$(fetch_url "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null) || json=""
	if [ -n "$json" ]; then
		tag=$(printf '%s\n' "$json" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)
		tag="${tag#v}"
		if validate_version "$tag"; then
			printf '%s\n' "$tag"
			return 0
		fi
	fi

	atom=$(fetch_url "https://github.com/$REPO/releases.atom" 2>/dev/null) || atom=""
	if [ -n "$atom" ]; then
		tag=$(printf '%s\n' "$atom" | sed -n 's:.*<title>\(v*[0-9][0-9A-Za-z._-]*\)</title>.*:\1:p' | head -n 1)
		tag="${tag#v}"
		if validate_version "$tag"; then
			printf '%s\n' "$tag"
			return 0
		fi
	fi

	if [ "$DL" = curl ]; then
		location=$(curl -fsSI "https://github.com/$REPO/releases/latest" 2>/dev/null | \
			sed -n 's/^[Ll]ocation:[[:space:]]*.*\/tag\/v*\([0-9][0-9A-Za-z._-]*\).*/\1/p' | \
			tr -d '\r' | head -n 1)
		if validate_version "$location"; then
			printf '%s\n' "$location"
			return 0
		fi
	fi

	return 1
}

resolve_version() {
	if [ -n "$VERSION" ]; then
		validate_version "$VERSION" || die "Некорректная версия: $VERSION"
		return 0
	fi
	log "Определение последней версии..."
	VERSION=$(latest_version) || die "Не удалось определить версию. Укажите её явно: sh install-zapret2.sh 1.0.0-r1"
	validate_version "$VERSION" || die "Некорректная версия: $VERSION"
	log "Последняя версия: $VERSION"
}

pkg_update() {
	if [ "$PKG_MGR" = opkg ]; then
		opkg update >>"$PKG_LOG" 2>&1
	else
		apk update >>"$PKG_LOG" 2>&1
	fi
}

ensure_pkg_index() {
	[ "$NO_UPDATE" = 1 ] && return 0
	[ "$PKG_INDEX_READY" = 1 ] && return 0
	log "Обновление списка пакетов..."
	if pkg_update; then
		PKG_INDEX_READY=1
		return 0
	fi
	warn "Не удалось обновить список пакетов. Продолжаю."
	return 1
}

pkg_add_feed() {
	if [ "$PKG_MGR" = opkg ]; then
		opkg install "$@" >>"$PKG_LOG" 2>&1
	else
		apk add "$@" >>"$PKG_LOG" 2>&1
	fi
}

pkg_add_local() {
	mode="$1"
	shift
	if [ "$PKG_MGR" = opkg ]; then
		if [ "$mode" = reinstall ]; then
			opkg install --force-reinstall "$@" >>"$PKG_LOG" 2>&1
		else
			opkg install "$@" >>"$PKG_LOG" 2>&1
		fi
	else
		if [ "$mode" = reinstall ]; then
			apk add --allow-untrusted --force-non-repository --force-reinstall "$@" >>"$PKG_LOG" 2>&1
		else
			apk add --allow-untrusted --force-non-repository "$@" >>"$PKG_LOG" 2>&1
		fi
	fi
}

pkg_add_local_overwrite() {
	if [ "$PKG_MGR" = opkg ]; then
		opkg install --force-reinstall --force-overwrite "$@" >>"$PKG_LOG" 2>&1
	else
		apk add --allow-untrusted --force-non-repository --force-reinstall --force-overwrite "$@" >>"$PKG_LOG" 2>&1
	fi
}

pkg_installed() {
	if [ "$PKG_MGR" = opkg ]; then
		opkg list-installed 2>/dev/null | \
			awk -v p="$1" '$1 == p { found = 1 } END { exit !found }'
	else
		apk list --installed -q "$1" 2>/dev/null | grep -qx "$1"
	fi
}

pkg_available() {
	if [ "$PKG_MGR" = opkg ]; then
		opkg info "$1" >/dev/null 2>&1
	else
		apk list -q "$1" 2>/dev/null | grep -qx "$1"
	fi
}

pkg_version() {
	if [ "$PKG_MGR" = opkg ]; then
		opkg list-installed 2>/dev/null | \
			awk -v p="$1" '$1 == p { print $3; exit }'
	else
		apk list --installed "$1" 2>/dev/null | \
			sed -n "s/^$1-\([^ ]*\).*/\1/p" | head -n 1
	fi
}

list_installed_names() {
	if [ "$PKG_MGR" = opkg ]; then
		opkg list-installed 2>/dev/null | awk '{ print $1 }'
	else
		apk list --installed -q 2>/dev/null
	fi
}

ensure_downloader() {
	detect_downloader
	[ -n "$DL" ] && return 0

	log "Не найден curl, wget или uclient-fetch."
	if ask_yes_no "Установить пакет curl? [Y/n]" yes; then
		ensure_pkg_index || true
		if pkg_add_feed curl; then
			detect_downloader
			[ -n "$DL" ] && return 0
		fi
	fi
	die "Для скачивания нужен curl, wget или uclient-fetch"
}

detect_unzip() {
	if have unzip; then
		UNZIP=unzip
		return 0
	fi
	if have busybox && busybox --list 2>/dev/null | grep -qx unzip; then
		UNZIP=busybox
		return 0
	fi
	if have python3; then
		UNZIP=python3
		return 0
	fi
	return 1
}

ensure_unzip() {
	detect_unzip && return 0

	log "Не найден unzip, busybox unzip или python3."
	if ask_yes_no "Установить пакет unzip? [Y/n]" yes; then
		ensure_pkg_index || true
		if pkg_add_feed unzip; then
			detect_unzip && return 0
		fi
	fi
	die "Для распаковки архива нужен unzip"
}

check_space() {
	root_avail=$(df -k / 2>/dev/null | awk 'NR == 2 { print $4 }')
	tmp_avail=$(df -k /tmp 2>/dev/null | awk 'NR == 2 { print $4 }')
	mem_avail=$(awk '/^MemAvailable:/ { print $2 }' /proc/meminfo 2>/dev/null)
	[ -n "$mem_avail" ] || mem_avail=$(awk '/^MemFree:/ { print $2 }' /proc/meminfo 2>/dev/null)

	case "$root_avail" in
		''|*[!0-9]*) ;;
		*) [ "$root_avail" -lt 10240 ] && warn "Мало свободного места на /: ${root_avail} КБ" ;;
	esac
	case "$tmp_avail" in
		''|*[!0-9]*) ;;
		*) [ "$tmp_avail" -lt 5120 ] && warn "Мало свободного места в /tmp: ${tmp_avail} КБ" ;;
	esac
	case "$mem_avail" in
		''|*[!0-9]*) ;;
		*) [ "$mem_avail" -lt 16384 ] && warn "Мало свободной памяти: ${mem_avail} КБ" ;;
	esac
}

determine_luci() {
	if [ "$WITH_LUCI" = 1 ]; then
		INSTALL_LUCI=1
	elif [ "$WITH_LUCI" = 0 ]; then
		INSTALL_LUCI=0
	elif pkg_installed luci-base; then
		INSTALL_LUCI=1
	else
		INSTALL_LUCI=0
	fi
}

make_workdir() {
	WORKDIR=$(mktemp -d /tmp/zapret2-install.XXXXXX 2>/dev/null || true)
	if [ -z "$WORKDIR" ]; then
		WORKDIR="/tmp/zapret2-install.$$"
		mkdir -p "$WORKDIR" || die "Не удалось создать временный каталог"
	fi
	trap cleanup 0 1 2 15
	EXTRACT_DIR="$WORKDIR/extract"
	ZIP_PATH="$WORKDIR/$ASSET"
	mkdir -p "$EXTRACT_DIR" || die "Не удалось создать каталог распаковки"
}

download_asset() {
	log "Скачивание: $ASSET_URL"
	if ! download_file "$ASSET_URL" "$ZIP_PATH"; then
		rm -f "$ZIP_PATH"
		die "Не удалось скачать архив. Проверьте версию, архитектуру и доступ к GitHub."
	fi
	[ -s "$ZIP_PATH" ] || die "Скачан пустой файл"
	set -- $(wc -c < "$ZIP_PATH" 2>/dev/null)
	log "Скачано: ${1:-?} байт"
}

extract_asset() {
	ensure_unzip
	log "Проверка и распаковка архива..."

	case "$UNZIP" in
		unzip)
			unzip -t "$ZIP_PATH" >/dev/null 2>&1 || die "Архив повреждён"
			unzip -q "$ZIP_PATH" -d "$EXTRACT_DIR" >>"$PKG_LOG" 2>&1 || \
				die "Не удалось распаковать архив"
			;;
		busybox)
			busybox unzip -l "$ZIP_PATH" >/dev/null 2>&1 || die "Архив повреждён"
			busybox unzip -q "$ZIP_PATH" -d "$EXTRACT_DIR" >>"$PKG_LOG" 2>&1 || \
				die "Не удалось распаковать архив"
			;;
		python3)
			python3 - "$ZIP_PATH" "$EXTRACT_DIR" <<'PY'
import sys
import zipfile

try:
    with zipfile.ZipFile(sys.argv[1]) as archive:
        bad = archive.testzip()
        if bad:
            print("bad file: " + bad, file=sys.stderr)
            sys.exit(1)
        archive.extractall(sys.argv[2])
except Exception as exc:
    print(str(exc), file=sys.stderr)
    sys.exit(1)
PY
			[ $? -eq 0 ] || die "Не удалось распаковать архив"
			;;
		*)
			die "Не найден инструмент для распаковки"
			;;
	esac
}

select_packages() {
	ZAPRET_PKG=""
	LUCI_PKG=""

	if [ "$PKG_FMT" = ipk ]; then
		for f in "$EXTRACT_DIR"/zapret2_"$VERSION"_*.ipk; do
			[ -f "$f" ] || continue
			ZAPRET_PKG="$f"
			break
		done

		[ -n "$ZAPRET_PKG" ] || die "В архиве не найден пакет zapret2"
		case "$ZAPRET_PKG" in
			*"_${ARCH}.ipk") ;;
			*) die "В архиве неверная архитектура zapret2: $ZAPRET_PKG" ;;
		esac

		if [ -f "$EXTRACT_DIR/luci-app-zapret2_${VERSION}_all.ipk" ]; then
			LUCI_PKG="$EXTRACT_DIR/luci-app-zapret2_${VERSION}_all.ipk"
		fi
	else
		[ -f "$EXTRACT_DIR/zapret2-${VERSION}.apk" ] && \
			ZAPRET_PKG="$EXTRACT_DIR/zapret2-${VERSION}.apk"
		[ -n "$ZAPRET_PKG" ] || die "В архиве не найден пакет zapret2"

		if [ -f "$EXTRACT_DIR/luci-app-zapret2-${VERSION}.apk" ]; then
			LUCI_PKG="$EXTRACT_DIR/luci-app-zapret2-${VERSION}.apk"
		fi
	fi

	if [ "$INSTALL_LUCI" = 1 ] && [ -z "$LUCI_PKG" ]; then
		die "В архиве не найден пакет luci-app-zapret2"
	fi
}

install_main_packages() {
	need_zapret=0
	need_luci=0
	installed=""

	installed=$(pkg_version zapret2)
	if [ "$FORCE" = 1 ] || [ "$installed" != "$VERSION" ]; then
		need_zapret=1
	else
		log "zapret2 $VERSION уже установлен."
	fi

	if [ "$INSTALL_LUCI" = 1 ]; then
		installed=$(pkg_version luci-app-zapret2)
		if [ "$FORCE" = 1 ] || [ "$installed" != "$VERSION" ]; then
			need_luci=1
		else
			log "luci-app-zapret2 $VERSION уже установлен."
		fi
	fi

	if [ "$need_zapret" = 0 ] && [ "$need_luci" = 0 ]; then
		return 0
	fi

	ensure_pkg_index || true
	set --
	[ "$need_zapret" = 1 ] && set -- "$ZAPRET_PKG"
	[ "$need_luci" = 1 ] && set -- "$@" "$LUCI_PKG"

	mode=normal
	[ "$FORCE" = 1 ] && mode=reinstall

	log "Установка пакетов..."
	if pkg_add_local "$mode" "$@"; then
		log "Пакеты установлены."
		return 0
	fi

	warn "Установка не удалась. Повтор с --force-reinstall --force-overwrite..."
	if pkg_add_local_overwrite "$@"; then
		log "Пакеты установлены (force-reinstall/force-overwrite)."
		return 0
	fi

	warn "Не удалось установить пакеты. Последние строки лога:"
	tail -n 40 "$PKG_LOG" >&2 2>/dev/null || true
	die "Установка пакетов не удалась"
}

ru_installed() {
	pkg_installed luci-i18n-base-ru || [ -f /usr/lib/lua/luci/i18n/base.ru.lmo ]
}

luci_present() {
	[ "$INSTALL_LUCI" = 1 ] || pkg_installed luci-base
}

set_luci_lang_ru() {
	if ! have uci; then
		warn "Не найден uci: не удалось переключить язык LuCI"
		return 1
	fi

	if [ -f /etc/uci-defaults/luci-i18n-base-ru ]; then
		sh /etc/uci-defaults/luci-i18n-base-ru >/dev/null 2>&1 || true
	fi

	uci set luci.main.lang='ru' >/dev/null 2>&1 || true
	uci set luci.languages.ru='Русский' >/dev/null 2>&1 || true
	if uci commit luci && [ "$(uci get luci.main.lang 2>/dev/null)" = "ru" ]; then
		/etc/init.d/rpcd reload >/dev/null 2>&1 || true
		log "Язык LuCI переключён на русский."
		return 0
	fi

	warn "Не удалось сохранить настройку языка LuCI"
	return 1
}

print_ru_manual() {
	if [ "$PKG_FMT" = ipk ]; then
		log "Установить русский язык позже:"
		log "  opkg update"
		log "  opkg install luci-i18n-base-ru luci-i18n-firewall-ru"
		log "  opkg list | grep '^luci-i18n-.*-ru'"
	else
		log "Установить русский язык позже:"
		log "  apk update"
		log "  apk add luci-i18n-base-ru luci-i18n-firewall-ru"
		log "  apk list -q | grep '^luci-i18n-.*-ru'"
	fi
	log "  uci set luci.main.lang='ru'"
	log "  uci set luci.languages.ru='Русский'"
	log "  uci commit luci"
	log "  /etc/init.d/rpcd reload"
}

install_russian_language() {
	if ! luci_present; then
		log "LuCI не установлен, русский язык LuCI не требуется."
		return 0
	fi

	if ru_installed; then
		if [ "$LANG_RU" = 1 ]; then
			set_luci_lang_ru || true
		fi
		return 0
	fi

	if [ "$LANG_RU" = 0 ]; then
		log "Установка русского языка отключена."
		return 0
	fi

	if [ "$LANG_RU" != 1 ]; then
		if ask_yes_no "Русский язык интерфейса LuCI не установлен. Установить его? [y/N]" no; then
			LANG_RU=1
		else
			LANG_RU=0
			log "Пропускаю установку русского языка."
			print_ru_manual
			return 0
		fi
	fi

	ensure_pkg_index || true

	pkgs="luci-i18n-base-ru"
	for p in $(list_installed_names); do
		case "$p" in
			luci-app-*|luci-mod-*|luci-proto-*)
				name=${p#luci-app-}
				name=${name#luci-mod-}
				name=${name#luci-proto-}
				cand="luci-i18n-$name-ru"
				case " $pkgs " in
					*" $cand "*) continue ;;
				esac
				if pkg_available "$cand"; then
					pkgs="$pkgs $cand"
				fi
				;;
		esac
	done

	log "Установка русского языка LuCI: $pkgs"
	set -- $pkgs
	if pkg_add_feed "$@"; then
		set_luci_lang_ru || true
		return 0
	fi

	warn "Не удалось установить все пакеты русского языка. Пробую только базовый пакет."
	if pkg_add_feed luci-i18n-base-ru; then
		set_luci_lang_ru || true
		return 0
	fi

	warn "Русский язык не установлен."
	print_ru_manual
	return 1
}

run_uci_defaults() {
	if [ -f /etc/uci-defaults/99-zapret2 ]; then
		log "Применение uci-defaults для zapret2..."
		sh /etc/uci-defaults/99-zapret2 >/dev/null 2>&1 || \
			warn "Не удалось применить /etc/uci-defaults/99-zapret2"
	fi
}

reload_rpcd() {
	# LuCI caches menu and module indexes in /tmp; drop them so the new
	# application appears without waiting for a reboot.
	rm -f /tmp/luci-indexcache /tmp/luci-indexcache.* \
		/tmp/luci-modulecache /tmp/luci-modulecache.* 2>/dev/null || true
	[ -x /etc/init.d/rpcd ] && /etc/init.d/rpcd reload >/dev/null 2>&1 || true
}

start_zapret2() {
	if [ ! -x /etc/init.d/zapret2 ]; then
		warn "Не найден /etc/init.d/zapret2"
		return 1
	fi

	log "Включение и запуск zapret2..."
	/etc/init.d/zapret2 enable >/dev/null 2>&1 || true

	if /etc/init.d/zapret2 start >/dev/null 2>&1; then
		attempt=0
		while [ "$attempt" -lt 10 ]; do
			status=$(/etc/init.d/zapret2 status 2>/dev/null || true)
			if [ "$status" = "running" ]; then
				log "Статус zapret2: запущен"
				return 0
			fi
			attempt=$((attempt + 1))
			sleep 1
		done
		if [ "$status" = "inactive" ]; then
			log "Статус zapret2: не запущен"
		else
			log "Статус zapret2: ${status:-неизвестно}"
		fi
	fi

	warn "Сервис zapret2 не запустился. Последние записи logread:"
	logread -e zapret2 2>/dev/null | tail -n 30 >&2 || true
	return 1
}

verify_install() {
	if have zapret2; then
		zapret_version=$(zapret2 --version 2>/dev/null | head -n 1)
		[ -n "$zapret_version" ] && log "zapret2: $zapret_version" || \
			warn "Не удалось получить версию zapret2"
	else
		warn "Команда zapret2 не найдена"
	fi

	[ -x /opt/zapret2/nfq2/nfqws2 ] || warn "Не найден /opt/zapret2/nfq2/nfqws2"

	if [ "$INSTALL_LUCI" = 1 ]; then
		[ -f /usr/share/luci/menu.d/luci-app-zapret2.json ] || \
			warn "Не найден файл меню LuCI для zapret2"
	fi

	if [ "$LANG_RU" = 1 ]; then
		if ru_installed; then
			log "Русский язык LuCI установлен."
		else
			warn "Русский язык LuCI не найден"
		fi
	fi
}

print_plan() {
	log "OpenWrt: $OWRT (пакеты: $OWRT_USE)"
	log "Архитектура: $ARCH"
	log "Формат пакетов: $PKG_FMT"
	log "Версия: $VERSION"
	log "Архив: $ASSET"
	if [ "$INSTALL_LUCI" = 1 ]; then
		log "LuCI: установить luci-app-zapret2"
	else
		log "LuCI: не устанавливать"
	fi
	if ! luci_present; then
		log "Русский язык LuCI: не требуется"
	elif ru_installed; then
		log "Русский язык LuCI: уже установлен"
	elif [ "$LANG_RU" = 0 ]; then
		log "Русский язык LuCI: установка отключена"
	elif [ "$LANG_RU" = 1 ]; then
		log "Русский язык LuCI: установить"
	elif [ "$ASSUME_YES" = 1 ] || [ ! -r /dev/tty ]; then
		log "Русский язык LuCI: не установлен (пропуск)"
	else
		log "Русский язык LuCI: будет предложен"
	fi
}

print_summary() {
	log ""
	log "Установка завершена."
	log "  Версия: $VERSION"
	if [ "$OWRT" = "$OWRT_USE" ]; then
		log "  OpenWrt: $OWRT"
	else
		log "  OpenWrt: $OWRT (пакеты: $OWRT_USE)"
	fi
	log "  Архитектура: $ARCH"
	log "  Формат: $PKG_FMT"
	if [ "$INSTALL_LUCI" = 1 ]; then
		log "  LuCI: установлен"
	else
		log "  LuCI: не устанавливался"
	fi
	if ! luci_present; then
		log "  Русский язык LuCI: не требуется"
	elif ru_installed; then
		log "  Русский язык LuCI: установлен"
	else
		log "  Русский язык LuCI: не установлен"
	fi

	if [ "$START" = 1 ]; then
		if [ "$START_FAILED" = 1 ]; then
			log "  Сервис: не запущен"
		else
			log "  Сервис: запущен"
		fi
	else
		log "  Сервис: не запускался"
		log "Для включения и запуска:"
		log "  /etc/init.d/zapret2 enable"
		log "  /etc/init.d/zapret2 start"
	fi

	if [ "$INSTALL_LUCI" = 1 ]; then
		log "LuCI: http://<IP-роутера>/cgi-bin/luci/admin/services/zapret2"
	fi
}

init_log() {
	: >"$PKG_LOG" 2>/dev/null || PKG_LOG="/dev/null"
}

main() {
	parse_args "$@"
	init_log

	[ "$(id -u)" = 0 ] || die "Запустите скрипт от root"

	require_openwrt
	detect_pkg_mgr
	detect_arch
	resolve_owrt

	if [ "$DRY_RUN" = 1 ]; then
		detect_downloader
		if [ -z "$VERSION" ] && [ -z "$DL" ]; then
			die "Для --dry-run без указания версии нужен curl, wget или uclient-fetch"
		fi
	else
		ensure_downloader
	fi

	resolve_version

	ASSET=$(asset_name "$VERSION" "$OWRT_USE" "$ARCH" "$PKG_FMT") || {
		print_supported
		die "Нет пакетов для OpenWrt $OWRT_USE, архитектуры $ARCH и формата $PKG_FMT"
	}
	ASSET_URL="https://github.com/$REPO/releases/download/v$VERSION/$ASSET"

	determine_luci
	check_space
	print_plan

	if [ "$DRY_RUN" = 1 ]; then
		log "Режим --dry-run: изменения не выполнялись."
		exit 0
	fi

	make_workdir
	download_asset
	extract_asset
	select_packages
	install_main_packages
	install_russian_language || true
	run_uci_defaults
	reload_rpcd

	if [ "$START" = 1 ]; then
		start_zapret2 || START_FAILED=1
	fi

	verify_install
	print_summary

	[ "$START_FAILED" = 1 ] && exit 1
	exit 0
}

main "$@"
