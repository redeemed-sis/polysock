pkgname=polysock
pkgver=auto
pkgrel=1
arch=('x86_64')
license=('MIT')
depends=('gcc-libs' 'glibc')
makedepends=('rust' 'cargo')
options=('!debug')

pkgver() {
    grep '^version =' ${srcdir}/../Cargo.toml | cut -d '"' -f 2 | tr - .
}

build() {
  # --frozen гарантирует соответствие Cargo.lock
  cargo build --frozen --release --manifest-path ${srcdir}/../Cargo.toml --target-dir ${srcdir}/../target
}

package() {
  install -Dm755 "${srcdir}/../target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"
}
