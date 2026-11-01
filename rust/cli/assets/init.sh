#!/bin/sh /etc/rc.common

USE_PROCD=1
# after network
START=21

my_extra_command() {
	local cmd="$1"
	local help="$2"

	local extra="$(printf "%-16s%s" "${cmd}" "${help}")"
	EXTRA_HELP="${EXTRA_HELP}	${extra}
"
	EXTRA_COMMANDS="${EXTRA_COMMANDS} ${cmd}"
}
my_extra_command stop_fw "Stop zapret firewall (noop in iptables+fw3 case)"
my_extra_command start_fw "Start zapret firewall (noop in iptables+fw3 case)"
my_extra_command restart_fw "Restart zapret firewall (noop in iptables+fw3 case)"
my_extra_command reload_ifsets "Reload interface lists (nftables only)"
my_extra_command list_ifsets "Display interface lists (nftables only)"
my_extra_command list_table "Display zapret nftable (nftables only)"
my_extra_command stop_daemons "Stop zapret daemons only (=stop in iptables+fw3 case)"
my_extra_command start_daemons "Start zapret daemons only (=start in iptables+fw3 case)"
my_extra_command restart_daemons "Restart zapret firewall only (=restart in iptables+fw3 case)"

SCRIPT=$(readlink /etc/init.d/zapret2)
if [ -n "$SCRIPT" ]; then
 EXEDIR=$(dirname "$SCRIPT")
 ZAPRET_BASE=$(readlink -f "$EXEDIR/../..")
else
 ZAPRET_BASE=/opt/zapret2
fi

. "$ZAPRET_BASE/init.d/openwrt/functions"


# !!!!! in old openwrt 21.x- with iptables firewall rules are configured separately
# !!!!! in new openwrt >21.x with nftables firewall is configured here

PIDDIR=/var/run

USEROPT="--user=$WS_USER"
NFQWS2="${NFQWS2:-$ZAPRET_BASE/nfq2/nfqws2}"
LUAOPT="--lua-init=@$ZAPRET_BASE/lua/zapret-lib.lua --lua-init=@$ZAPRET_BASE/lua/zapret-antidpi.lua --lua-init=@$ZAPRET_BASE/lua/zapret-auto.lua"
NFQWS2_OPT_BASE="$USEROPT --fwmark=$DESYNC_MARK $LUAOPT"

DAEMON_CFGNAME="main"

run_daemon()
{
	# $1 - daemon string id or number. can use 1,2,3,...
	# $2 - daemon
	# $3 - daemon args
	# use $PIDDIR/$DAEMONBASE$1.pid as pidfile
	local DAEMONBASE="$(basename "$2")"
	local DAEMON_NAME="$DAEMONBASE"
	local DAEMON_IDNUM=$1
	local DAEMON_PATH="$2"
	local DAEMON_ARGS="$3"
	local DAEMON_LOG=

	echo "Starting daemon $1: $2 $3"

	if [ -n "$DAEMON_LOG_FILE" ]; then
		DAEMON_LOG="$DAEMON_LOG_FILE"
		DAEMON_LOG="${DAEMON_LOG//<DAEMON_NAME>/$DAEMON_NAME}"
		DAEMON_LOG="${DAEMON_LOG//<DAEMON_IDNUM>/$DAEMON_IDNUM}"
		DAEMON_LOG="${DAEMON_LOG//<DAEMON_CFGNAME>/$DAEMON_CFGNAME}"

		mkdir -p "$(dirname "$DAEMON_LOG")"

		[ -f "$DAEMON_LOG" ] && rm -f "$DAEMON_LOG"

		if [ "$DAEMON_LOG_ENABLE" = "1" ]; then
			DAEMON_ARGS="--debug=@$DAEMON_LOG $DAEMON_ARGS"
		fi
	fi

	procd_open_instance
	procd_set_param command $DAEMON_PATH $DAEMON_ARGS
	procd_set_param pidfile $PIDDIR/${DAEMONBASE}_$1.pid
	procd_close_instance
}

run_nfqws()
{
	# $1 - daemon id
	# $2 - nfqws options
	run_daemon $1 "$NFQWS2" "$NFQWS2_OPT_BASE $2"
}
do_nfqws()
{
	[ "$1" = 0 ] || { shift; run_nfqws "$@"; }
}

start_daemons_procd()
{
	standard_mode_daemons 1
	custom_runner zapret_custom_daemons 1

	return 0
}
start_daemons()
{
	rc_procd start_daemons_procd "$@"
}
stop_daemons()
{
	local svc="$(basename ${basescript:-$initscript})"
	procd_running "$svc" "$1" && procd_kill "$svc" "$1"
}
restart_daemons()
{
	stop_daemons
	start_daemons
}

start_fw()
{
	zapret_apply_firewall
}
stop_fw()
{
	zapret_unapply_firewall
}
restart_fw()
{
	stop_fw
	start_fw
}
reload_ifsets()
{
	zapret_reload_ifsets
}
list_ifsets()
{
	zapret_list_ifsets
}
list_table()
{
	zapret_list_table
}

start_service()
{
	start_daemons_procd
	[ "$INIT_APPLY_FW" != "1" ] || {
		linux_fwtype
		openwrt_fw3_integration || start_fw
	}
}

stop_service()
{
	# this procedure is called from stop()
	# stop() already stop daemons
	[ "$INIT_APPLY_FW" != "1" ] || {
		linux_fwtype
		openwrt_fw3_integration || stop_fw
	}
}
custom_runner()
{
	# $1 - function name
	# $2+ - params

	[ "$DISABLE_CUSTOM" = 1 ] && return 0

	local n script FUNC=$1

	shift

	[ -d "$CUSTOM_DIR/custom.d" ] && {
		dir_is_not_empty "$CUSTOM_DIR/custom.d" && {
			for script in "$CUSTOM_DIR/custom.d/"*; do
				[ -f "$script" ] || continue

				DAEMON_CFGNAME_SAVED="$DAEMON_CFGNAME"
				unset DAEMON_CFGNAME

				unset -f $FUNC
				. "$script"

				if [ -z "$DAEMON_CFGNAME" ]; then
					DAEMON_CFGNAME="$(basename "$script")"
					DAEMON_CFGNAME="${DAEMON_CFGNAME%%.*}"
				fi

				existf $FUNC && $FUNC "$@"

				DAEMON_CFGNAME="$DAEMON_CFGNAME_SAVED"
			done
		}
	}
}

alloc_qnum()
{
	# $1 - target var name
	alloc_num NUMPOOL_QNUM $1 65300 65399
}
alloc_dnum()
{
	# alloc daemon number
	# $1 - target var name
	alloc_num NUMPOOL_DNUM $1 2000 2999
}
