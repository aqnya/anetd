#!/system/bin/sh

MODDIR=${0%/*}
STATE_FILE="$MODDIR/log/dns_off"

if [ ! -f "$STATE_FILE" ]; then
	echo "disable filter"
	kill -9 "$(cat "$MODDIR/log/anetd.pid")" 2>/dev/null
	touch "$STATE_FILE"
elif [ -f "$STATE_FILE" ]; then
	echo "loading filter"
	sh "$MODDIR/post-fs-data.sh"
fi