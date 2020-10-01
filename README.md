Networkd-broker
===============

[![Release](https://img.shields.io/github/v/tag/bpetlert/networkd-broker?include_prereleases&label=release&style=flat-square)](https://github.com/bpetlert/networkd-broker/releases/latest)
[![AUR:
networkd-broker](https://img.shields.io/aur/version/networkd-broker?style=flat-square)](https://aur.archlinux.org/packages/networkd-broker/)
[![License:
MIT](https://img.shields.io/github/license/bpetlert/networkd-broker?style=flat-square)](./LICENSE)

The networkd-broker is an event broker daemon for systemd-networkd. It
will execute scripts in the `/etc/networkd/broker.d/<STATE>.d` directory
in alphabetical order in response to network events.

This work is based on
[networkd-dispatcher](https://gitlab.com/craftyguy/networkd-dispatcher),
written in Rust, for the purpose of reducing runtime dependencies. This
also helps reduce memory footprint (\~30MB ⟶ \~8MB) and improve startup
time (\~30secs ⟶ \~1sec for spinning hard disk drive).

Installation
------------

### Arch Linux

It is available on AUR as
[networkd-broker](https://aur.archlinux.org/packages/networkd-broker/).
To build and install arch package from GIT source:

    $ git clone https://github.com/bpetlert/networkd-broker.git
    ...
    $ cd networkd-broker
    $ makepkg -p PKGBUILD.local
    ...
    $ pacman -U networkd-broker-xxxx-1-x86_64.pkg.tar

Then enable/start networkd-broker.service

    $ systemctl enable networkd-broker.service
    ...
    $ systemctl start networkd-broker.service

Configuration
-------------

To change the options of networkd-broker service, run:

    $ systemctl edit networkd-broker.service`
    ...

    /etc/systemd/system/networkd-broker.service.d/override.conf
    -------------------------------------------------------------------------

    [Service]
    Environment='NETWORKD_BROKER_ARGS=-vv --json'

Supported options are:

-   `-j`, `--json` Pass JSON encoding of event and link status to
    script.
-   `-T`, `--run-startup-triggers` Generate events reflecting
    preexisting state and behavior on startup.
-   `-v`, `--verbose` Increment verbosity level once per call. Default
    is showing error.
    -   `-v`: warn
    -   `-vv`: info
    -   `-vvv`: debug
    -   `-vvvv`: trace
-   `-S`, `--script-dir <script_dir>` Location under which to look for
    scripts. The default location is `/etc/networkd/broker.d`.
-   `-t`, `--timeout <timeout>` Script execution timeout in seconds.
    Default is 20 seconds.

Usage
-----

The scripts for any network event need to be putted (or symlink) in its
corresponding directory as shown below. Each script must be a regular
executable file owned by root. The default execution timeout of each
script is 20 seconds. It can be overridden by `--timeout` option in
service configuration. Any of the scripts which end with '-nowait' is
run immediately, without waitting for the termination of previous
scripts.

    /etc/networkd
    └── broker.d
        ├── carrier.d
        ├── configured.d
        ├── configuring.d
        ├── degraded.d
        ├── dormant.d
        ├── linger.d
        ├── no-carrier.d
        ├── off.d
        ├── routable.d
        └── unmanaged.d

The scripts are run in alphabetical order, one at a time with two
arguments and a set of environment variables being passed. Each script
run asynchronously from `networkd-broker` process.

| Argument | Description                                                                                                                                                                                                       |
|----------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `STATE`  | Current operational status is one of the following: `carrier`, `configured`, `configuring`, `degraded`, `dormant`, `linger`, `no-carrier`, `off`, `routable`, `unmanaged`; see `man networkctl` for more details. |
| `IFACE`  | Link name that operation just happened on                                                                                                                                                                         |

The following environment variables are being passed to each script:

| Variable                   | Description                                                                                                           |
|----------------------------|-----------------------------------------------------------------------------------------------------------------------|
| `NWD_DEVICE_IFACE`         | Link name that operation just happened on, same value as `IFACE`                                                      |
| `NWD_BROKER_ACTION`        | Current operational status, same value as `STATE`                                                                     |
| `NWD_ESSID`                | SSID of access point if link is wireless                                                                              |
| `NWD_STATION`              | MAC address of associated access point                                                                                |
| `NWD_IP4_ADDRESS`          | Current IPv4 address                                                                                                  |
| `NWD_IP6_ADDRESS`          | Current IPv6 address                                                                                                  |
| `NWD_ADMINISTRATIVE_STATE` | Current operation; see `man networkctl` for more details.                                                             |
| `NWD_OPERATIONAL_STATE`    | Current operation; see `man networkctl` for more details.                                                             |
| `NWD_JSON`                 | All the environment variables above are encoded in JSON format if `--json` option is setted in service configuration. |

### Example script

The script below activate/deactivate
[Chrony](https://wiki.archlinux.org/index.php/Chrony) correspond to
operation state of wlan0 link. This script must be putted (or symlink)
in `/etc/networkd/broker.d/configured.d` and
`/etc/networkd/broker.d/dormant.d`.

    #!/usr/bin/env bash

    STATE=$1
    IFACE=$2

    if [[ $IFACE != "wlan0" ]]; then
        exit 0
    fi

    if [[ $STATE == "configured" ]]; then
        chronyc online > /dev/null
        echo "Activate chrony"
    elif [[ $STATE == "dormant" ]]; then
        chronyc offline > /dev/null
        echo "Deactivate chrony"
    fi

Design (roughly)
----------------

![Sequence Diagram](docs/assets/networkd-broker.png)

License
-------

[networkd-notify](https://github.com/wavexx/networkd-notify):  
Copyright (c) 2016 [Yuri D'Elia](wavexx@thregr.org)

[networkd-dispatcher](https://gitlab.com/craftyguy/networkd-dispatcher):  
Copyright (c) 2018 [Clayton Craft](clayton@craftyguy.net)

[networkd-broker](https://github.com/bpetlert/networkd-broker):  
Copyright (c) 2019 [Bhanupong Petchlert](bpetlert@gmail.com)

**[GNU GPLv3](./LICENSE)**  
This program is free software: you can redistribute it and/or modify it
under the terms of the GNU General Public License as published by the
Free Software Foundation, either version 3 of the License, or (at your
option) any later version.

This program is distributed in the hope that it will be useful, but
WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General
Public License for more details.

You should have received a copy of the GNU General Public License along
with this program. If not, see <https://www.gnu.org/licenses/>.
