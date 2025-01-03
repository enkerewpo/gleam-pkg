# gleam-pkg

gleam package manager for installing Gleam CLI programs right inside your OS

this is a prototype for functionality of gleam cli apps shipping and management.

discussion here: https://github.com/gleam-lang/gleam/discussions/4136

to compile:

```bash
git clone https://github.com/enkerewpo/gleam-pkg
git submodule update --init --recursive
cargo build
```
## usage example

```bash
# gleam-pkg install gleewhois # this will be supported when we can install gleam-pkg and run it in shell
cargo run -- install gleewhois
# source ~/.bashrc or ~/.zshrc because first time gleam-pkg will ask
# whether you want to add ~/.gleam-pkgs to PATH for using the gleam app in your shell
gleewhois --help
```

## some design ideas

- support install from github or hex.pm
- workspace under `$home/.gleam_pkgs`
- package metadata management for installation and uninstallation

## uninstalling

```bash
rm -rf ~/.gleam_pkgs
```

and remove the gleam_pkgs path in PATH env from your shell rc file.

## repos of gleam cli apps

- [gleamfonts](https://github.com/massix/gleamfonts) (not in hex)
- [ormlette](https://github.com/ashercn97/ormlette) [hex](https://hex.pm/packages/ormlette)
- [gleewhois](https://github.com/kjartanhr/gleewhois) [hex](https://hex.pm/packages/gleewhois)
- [gleescript](https://github.com/lpil/gleescript) [hex](https://hexdocs.pm/gleescript)

wheatfox 2024
