#!/system/bin/sh

MODDIR=${0%/*}
STATE_FILE="$MODDIR/log/dns_off"

if [ ! -f "$STATE_FILE" ]; then
	kill -2 "$(cat "$MODDIR/log/anetd.pid")" 2>/dev/null
	touch "$STATE_FILE"
elif [ -f "$STATE_FILE" ]; then
	sh "$MODDIR/post-fs-data.sh"
fi