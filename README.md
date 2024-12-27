# gleam-pkg

gleam package manager for installing Gleam CLI programs right inside your OS

to compile:

```bash
git clone https://github.com/enkerewpo/gleam-pkg
git submodule update --init --recursive
cargo build
```

## some design ideas

- support install from github or hex.pm
- workspace under `$home/.gleam_pkgs`
- package metadata management for installation and uninstallation

## repos of gleam cli apps

- [gleamfonts](https://github.com/massix/gleamfonts) (not in hex)
- [ormlette](https://github.com/ashercn97/ormlette) [hex](https://hex.pm/packages/ormlette)
- [gleewhois](https://github.com/kjartanhr/gleewhois) [hex](https://hex.pm/packages/gleewhois)

wheatfox 2024
