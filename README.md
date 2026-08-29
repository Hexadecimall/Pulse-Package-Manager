# Pulse

One command to install software, no matter which package manager the machine
actually has.

Every operating system ships its own idea of a package manager — `apt` here,
`pacman` there, `dnf`, `winget`, Homebrew on a Mac. They all do the same job
with different names, different flags, and different output. Pulse sits on top
of them and gives you a single, consistent interface. Ask for a package and
Pulse figures out how to get it.

And when nothing on the system knows about the thing you want, Pulse can still
install it — by fetching the binary directly and putting it on your `PATH`.
That isn't a fallback for when the "real" way fails; it's a first-class way to
install software that a distro's repositories never packaged in the first
place.

## What it does

- **Speaks every backend.** apt, dnf, pacman, winget, Homebrew — detected
  automatically, used transparently. You run `pulse install`, Pulse picks the
  right tool for the platform you're on.
- **Installs binaries directly.** Pull a release straight from where it lives,
  drop it in `~/.pulse/bin`, and it's on your `PATH`. Works even on systems
  with no package manager Pulse recognizes.
- **Keeps track of what it installed.** Everything Pulse puts on your machine
  is recorded, so `list`, `update`, and `remove` know exactly what they're
  dealing with — including the binaries it fetched directly.

## Usage

```
pulse install <package>     # install, via a system manager or directly
pulse remove  <package>     # uninstall
pulse search  <query>       # search across available backends
pulse list                  # everything Pulse has installed
pulse update  [package]     # update one package, or all of them
pulse info    <package>     # details about a package
pulse backends              # show which managers were detected here
pulse doctor                # check the environment and report problems
```

## How it's laid out

Pulse keeps its state in a single directory, `~/.pulse`:

```
~/.pulse/
├── bin/            binaries Pulse installed directly (added to PATH)
├── config.toml     settings
└── db.json         record of everything Pulse has installed
```

The project itself is a small workspace:

- **`lib-pulse`** — the library. Backend detection, the package-manager
  adapters, direct-binary installation, and the state database. All the actual
  logic lives here, so it can be embedded in other tools too.
- **`pulse`** — the command-line front end. A thin layer that parses arguments
  and calls into `lib-pulse`.

## Building

```
git clone https://github.com/Hexadecimall/Pulse-Package-Manager
cd Pulse-Package-Manager
cargo build --release
```

The binary lands at `target/release/pulse`.

## Status

Early, and moving fast. Backend detection, the command surface, and the state
directory are in place; individual backends are being filled in one at a time.
Expect the set of supported managers to grow.

## License

MIT. See [LICENSE](LICENSE).
