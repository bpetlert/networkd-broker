#!/usr/bin/env bash

if [[ "$#" -ne 2 ]]; then
    printf 'ERROR: Incorrect number of arguments => %d\n' "$#" >&2
    exit 1
fi

if [[ "$1" != "routable" ]]; then
    printf 'ERROR: Incorrect 1st argument => %s\n' "$1" >&2
    exit 1
fi

if [[ "$2" != "wlp3s0" ]]; then
    printf 'ERROR: Incorrect 2nd argument => %s\n' "$2" >&2
    exit 1
fi

set | grep --silent --no-messages "NWD_DEVICE_IFACE"
if [[ "$?" -ne 0 ]]; then
    echo "ERROR: 'NWD_DEVICE_IFACE' environment variable does not exist." >&2
    exit 1
fi

set | grep --silent --no-messages "NWD_BROKER_ACTION"
if [[ "$?" -ne 0 ]]; then
    echo "ERROR: 'NWD_BROKER_ACTION' environment variable does not exist." >&2
    exit 1
fi

set | grep --silent --no-messages "NWD_JSON"
if [[ "$?" -ne 0 ]]; then
    echo "ERROR: 'NWD_JSON' environment variable does not exist." >&2
    exit 1
fi

if [[ "SCRIPT_FAILURE" -eq 1 ]]; then
    echo "Simulate script failure..." >&2
    /usr/bin/ls no-such-file
    exit "$?"
fi

exit 0
