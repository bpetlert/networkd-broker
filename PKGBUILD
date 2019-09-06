# Maintainer: Bhanupong Petchlert <bpetlert@gmail.com>
pkgname=networkd-broker
pkgver=0.1.0.r0.g2245859
pkgrel=1
pkgdesc="An event broker daemon for systemd-networkd"
arch=('x86_64')
url="https://github.com/bpetlert/networkd-broker"
license=('GPL3')
depends=('systemd' 'iw')
makedepends=('rust' 'cargo')
provides=("${pkgname}")
conflicts=("${pkgname}")

# Build from local directory
source=()

# Using the most recent un-annotated tag reachable from the last commit.
pkgver() {
  cd "$startdir"
  git describe --long --tags | sed 's/\([^-]*-g\)/r\1/;s/-/./g'
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

  install -dm755 "$pkgdir/etc/networkd/broker.d/"{carrier.d,configured.d,configuring.d,degraded.d,dormant.d,linger.d,no-carrier.d,off.d,routable.d,unmanaged.d}

  install -Dm644 "$startdir/README.md" "$pkgdir/usr/share/doc/${pkgname}/README.md"
  install -Dm644 "$startdir/LICENSE" "$pkgdir/usr/share/licenses/${pkgname}/LICENSE"
}
