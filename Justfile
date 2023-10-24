ip := "/usr/bin/ip -color"

VIR_IFACE := "nwd-eth0"

@_default:
  just --list

run +ARGS='':
  cargo run -- {{ARGS}}

# Run with debug log
run-debug +ARGS='':
  RUST_BACKTRACE=1 RUST_LOG=networkd_broker=debug cargo run -- {{ARGS}}

# Run test
test +CASES='':
  RUST_BACKTRACE=1 RUST_LOG=networkd_broker=debug cargo test -- {{CASES}}

# Increase semver
bump-version VERSION:
  just _bump-cargo {{VERSION}}
  just _bump-pkgbuild {{VERSION}}
  cargo check

@_bump-cargo VERSION:
  cargo bump {{VERSION}}

@_bump-pkgbuild VERSION:
  sed -i -e "s/pkgver=.*/pkgver={{VERSION}}/g" -e "s/pkgrel=.*/pkgrel=1/g"  PKGBUILD.local
  sed -i -e "s/pkgver=.*/pkgver={{VERSION}}/g" -e "s/pkgrel=.*/pkgrel=1/g"  PKGBUILD.aur

# Commit bump version and release
release VERSION:
  git add Cargo.lock Cargo.toml PKGBUILD.aur PKGBUILD.local
  git commit --message="chore(release): {{VERSION}}"
  git tag --sign --annotate {{VERSION}} --message="version {{VERSION}}" --edit

# Update and audit dependencies
update-deps:
  cargo upgrade
  cargo update
  cargo audit

# Crate Arch package from GIT source
makepkg:
  makepkg -p PKGBUILD.local
  git co PKGBUILD.local

# Monitor org.freedesktop.network1
monitor-bus:
  sudo busctl monitor --match "type='signal',path_namespace='/org/freedesktop/network1/link',interface='org.freedesktop.DBus.Properties',member='PropertiesChanged'" | grep --after-context=2 OperationalState

# Create virtual network interface
iface-create NAME=VIR_IFACE:
  sudo modprobe dummy
  sudo {{ip}} link add {{NAME}} type dummy
  {{ip}} link show {{NAME}}

# Remove virtual network interface
iface-delete NAME=VIR_IFACE:
  sudo {{ip}} link delete {{NAME}} type dummy
  sudo rmmod dummy
  {{ip}} link show

iface-up NAME=VIR_IFACE:
  sudo {{ip}} link set dev {{NAME}} up
  {{ip}} address show {{NAME}}

iface-down NAME=VIR_IFACE:
  sudo {{ip}} link set dev {{NAME}} down
  {{ip}} address show {{NAME}}

iface-ip-set NAME=VIR_IFACE:
  sudo {{ip}} addr add 192.168.1.100/24 broadcast + dev {{NAME}}
  {{ip}} address show {{NAME}}

iface-ip-del NAME=VIR_IFACE:
  sudo {{ip}} address flush dev {{NAME}}
  {{ip}} address show {{NAME}}
