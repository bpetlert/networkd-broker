#!/usr/bin/env bash

if [[ "$#" -ne 2 ]]; then
    printf 'FAKE-SCRIPT-ERROR: Number of arguments != 2\n' >&2
    exit 51
fi

if [[ "$1" != "routable" ]]; then
    printf 'FAKE-SCRIPT-ERROR: Incorrect 1st argument => %s\n' "$1" >&2
    exit 52
fi

if [[ "$2" != "wlp3s0" ]]; then
    printf 'FAKE-SCRIPT-ERROR: Incorrect 2nd argument => %s\n' "$2" >&2
    exit 53
fi

set | grep --silent --no-messages "NWD_DEVICE_IFACE"
if [[ "$?" -ne 0 ]]; then
    echo "FAKE-SCRIPT-ERROR: 'NWD_DEVICE_IFACE' environment variable does not exist." >&2
    exit 54
fi

set | grep --silent --no-messages "NWD_BROKER_ACTION"
if [[ "$?" -ne 0 ]]; then
    echo "FAKE-SCRIPT-ERROR: 'NWD_BROKER_ACTION' environment variable does not exist." >&2
    exit 55
fi

set | grep --silent --no-messages "NWD_JSON"
if [[ "$?" -ne 0 ]]; then
    echo "FAKE-SCRIPT-ERROR: 'NWD_JSON' environment variable does not exist." >&2
    exit 56
fi

if [[ "SCRIPT_FAILURE" -eq 1 ]]; then
    echo "Simulate script failure..." >&2
    /usr/bin/ls no-such-file
    exit "$?"
fi

if [[ "SCRIPT_FAILURE" -eq 2 ]]; then
    echo "Simulate script timeout..." >&2
    sleep 60
    exit 0
fi

if [[ "SCRIPT_FAILURE" -eq 3 ]]; then
    echo "Simulate script nowait..." >&2
    sleep 2
    exit 0
fi

exit 0
