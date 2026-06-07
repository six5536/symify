# symify

Keep your files in sync with a backing repository — as symlinks or copies. A
dotfiles-style file manager: the files you use day to day stay where programs
expect them, while the real copies live in a repository you can keep under
version control (e.g. git).

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

Full documentation — configuration, path resolution, the per-entry state
machine, the safety model, and every command — lives in the GitHub repository:

- Readme & usage: <https://github.com/six5536/symify#readme>
- Architecture & design: <https://github.com/six5536/symify/blob/main/specs/ARCHITECTURE.md>

## License

MIT — see <https://github.com/six5536/symify/blob/main/LICENSE>.
