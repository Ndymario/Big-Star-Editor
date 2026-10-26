# AUR package

`PKGBUILD` and `.SRCINFO` for the `big-star-editor` AUR package. This directory
is the source of truth; the AUR repository is a clone of it.

Unlike the AppImage and the installer, it carries nothing the system has: the
editor is built from the tag's source and installed beside the system's
programs (`/usr/bin/bigstar`, its catalogs in `/usr/share/bigstar/data`). The
window system, keyboard and graphics libraries are opened at run time, so
`depends` names them even though nothing links them.

NCPatcher is not a dependency. The editor opens and edits a project without
it, and runs the `ncpatcher` it finds on the `PATH` to build.

## Releasing a new version

1. Set `version` in `Cargo.toml`, commit, then tag and push:

   ```sh
   git tag vx.y.z && git push --tags
   ```

   The Package workflow checks the tag against that version, builds the
   AppImage and the installer, and creates the GitHub release.

2. Update this directory:

   ```sh
   cd packaging/aur
   sed -i "s/^pkgver=.*/pkgver=x.y.z/" PKGBUILD
   updpkgsums                            # fills sha256sums from the new tag
   makepkg --printsrcinfo > .SRCINFO
   ```

3. Test the build in a clean chroot, then push `PKGBUILD` and `.SRCINFO` to the
   AUR repository:

   ```sh
   makepkg -si                           # or: extra-x86_64-build
   git -C /path/to/aur-repo commit -am "Update to x.y.z" && git -C /path/to/aur-repo push
   ```

`sha256sums=('SKIP')` is a placeholder until the first tag exists. `updpkgsums`
replaces it with the real hash of the release tarball, and it should not be
committed as `SKIP` after that.

## Trying it before a tag exists

```sh
mkdir -p build-aur/Big-Star-Editor-0.1.0
git ls-files -co --exclude-standard -z | tar --null -T - -cf - | tar -xf - -C build-aur/Big-Star-Editor-0.1.0
tar -czf build-aur/big-star-editor-0.1.0.tar.gz -C build-aur Big-Star-Editor-0.1.0 && rm -rf build-aur/Big-Star-Editor-0.1.0
cp packaging/aur/PKGBUILD build-aur/ && cd build-aur && makepkg -s --skipchecksums
```
