# Maintainer: Bhanupong Petchlert <bpetlert@gmail.com>
pkgname=networkd-broker
pkgver=20190816
pkgrel=1
pkgdesc=""
arch=('x86_64')
url="https://github.com/bpetlert/networkd-broker"
license=('MIT')
makedepends=('rust' 'cargo')

# Build from local directory
source=()

# Using the most recent un-annotated tag reachable from the last commit.
pkgver() {
  cd "$startdir"
  git describe --long --tags | sed 's/\([^-]*-g\)/r\1/;s/-/./g'
}

# Fallback
pkgver() {
  date +%Y%m%d
}

build() {
  cd "$startdir"

  # Ignore target_dir in ~/.cargo/config, use BUILDDIR from makepkg instead
  CARGO_TARGET_DIR="$srcdir/../target" cargo build --release --locked
}

package() {
  cd "$srcdir/../"
  install -Dm755 "target/release/networkd-broker" "$pkgdir/usr/bin/networkd-broker"

  install -Dm644 "$startdir/networkd-broker.service" "$pkgdir/usr/lib/systemd/system/networkd-broker.service"

  install -dm755 "$pkgdir/etc/networkd/broker.d/"{carrier.d,configured.d,configuring.d,degraded.d,dormant.d,no-carrier.d,off.d,routable.d}

  install -Dm644 "$startdir/README.md" "$pkgdir/usr/share/doc/${pkgname}/README.md"
  install -Dm644 "$startdir/LICENSE" "$pkgdir/usr/share/licenses/${pkgname}/LICENSE"
}
