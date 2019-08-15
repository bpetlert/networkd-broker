#!/usr/bin/env bash

{
    echo "======================================================"
    echo $(date -u +"%Y%m%d-%H%M%SZ")
    echo "======================================================"
    echo "Arguments:"
    echo "$0" "$@"
    echo
    echo "Environments:"
    set | grep "NWD_"
    echo
} >> /tmp/networkd-broker-test.txt
