#!/system/bin/sh

MODDIR=${0%/*}

rm -r $MODDIR/log
mkdir $MODDIR/log

chmod +x $MODDIR/anetd

$MODDIR/anetd -r $MODDIR/rules -s