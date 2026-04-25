Cheese Paper supports flatpaks (mostly)

This should shortly also be handled by CI, but to build a flatpak normally:

1. If `cargo-sources.json` hasn't been updated since the last time that `Cargo.lock` was, run `./build/flatpak/update_sources.sh` to verify that sources are up to date. (this should be fixed by [issue 272](https://codeberg.org/ByteOfBrie/cheese-paper/issues/272))
2. Install flatpak-builder for your distribution
3. Ensure that flathub is set up as a user repo: 
```
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
```
4. Run the flatpak build:
```
flatpak-builder --force-clean --user --install-deps-from=flathub --install builddir build/flatpak/gay.brie.CheesePaper.yml
```
(or without `--install` if you don't want to install it on your system)