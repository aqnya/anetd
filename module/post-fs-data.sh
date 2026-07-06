#!/system/bin/sh

MODDIR=${0%/*}

rm -r $MODDIR/log
mkdir $MODDIR/log

$MODDIR/anetd -r $MODDIR/rules -s