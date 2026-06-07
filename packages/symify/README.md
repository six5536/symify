# symify

Keep files in sync between a working location and a backing repository, as
symlinks or copies. The files you work with stay in place, while the real copies
live in a repository you can track with git. Dotfiles are a common use, but
symify works just as well for any files or folders you want to mirror, back up,
or deploy across machines.

## Install

```sh
npm install -g @six5536/symify
```

This package is a thin launcher. It declares a prebuilt binary for each
supported platform as an `optionalDependency`, and npm installs only the one
matching your machine; a small JS shim then runs it.

**Supported platforms:** Linux and macOS, `x64` and `arm64`. On any other
platform (including Windows), or to build from source, use the Rust toolchain
instead:

```sh
cargo install symify
```

## Quickstart

```sh
symify add ~/.zshrc      # move it into ~/dotfiles and replace it with a link
symify list              # see what's tracked
symify status            # show what each entry will do
symify sync              # adopt your live files into the store
symify deploy            # on a new machine, recreate links from the store
```

## Documentation

Full usage, configuration, and the safety model are documented in the GitHub
repository: <https://github.com/six5536/symify#readme>.

## License

MIT — see <https://github.com/six5536/symify/blob/main/LICENSE>.
