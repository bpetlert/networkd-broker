@_default:
  just --list

bumpversion version:
  just _bump-cargo {{version}}
  just _bump-pkgbuild {{version}}
  cargo check

@_bump-cargo version:
  cargo bump {{version}}

@_bump-pkgbuild version:
  sed -i -e "s/pkgver=.*/pkgver={{version}}/g" -e "s/pkgrel=.*/pkgrel=1/g"  PKGBUILD.local
  sed -i -e "s/pkgver=.*/pkgver={{version}}/g" -e "s/pkgrel=.*/pkgrel=1/g"  PKGBUILD.aur

release version:
  git add Cargo.lock Cargo.toml PKGBUILD.aur PKGBUILD.local
  git commit --message="chore(release): {{version}}"
  git tag --sign --annotate {{version}} --message="version {{version}}" --edit

test case:
  cargo test -- {{case}} --nocapture

update-deps:
  cargo upgrade
  cargo update

makepkg:
  makepkg -p PKGBUILD.local
