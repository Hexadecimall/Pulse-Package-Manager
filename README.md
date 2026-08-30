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
  drop it in your bin directory, and it's on your `PATH`. Works even on systems
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

Two global options:

- `--as-root` / `--as-user` choose where things go — system-wide
  (`/usr/local/bin`, needs root) or in your home (`~/.local/bin`, no root). The
  default follows how Pulse was installed; a system-mode operation that can't
  write its target falls back to `~/.local/bin` automatically.
- `--update [stable|beta|dev]` updates Pulse itself (see below).

## How it's laid out

Pulse keeps its **state** in `~/.pulse`, and installs **binaries** to a bin
directory that depends on the mode:

```
~/.pulse/
├── config          settings (TOML)
└── db.json         record of everything Pulse has installed

~/.local/bin/       user-mode installed binaries   (on your PATH)
/usr/local/bin/     system-mode installed binaries  (system install)
```

The project is a small workspace:

- **`lib/`** — the library (crate `pulse`). Source detection, the native
  installers, the direct-binary installer, and the state database.
- **`cli/`** — the command-line front end (produces the `pulse` binary).
- **`pulse/`** — `pulse-registry`, Pulse's own package registry (manifest
  format + index client). Not a default source yet.

## Installing

Grab a prebuilt binary — no build step:

```
curl -fsSL https://raw.githubusercontent.com/Hexadecimall/Pulse-Package-Manager/main/install.sh | bash
```

By default this installs to `/usr/local/bin` **setuid-root** (falling back to a
user install if root isn't available). Pass `--as-user` to install into
`~/.local/bin` with no root at all. On Windows, run `install.ps1` instead
(elevation is handled by UAC).

## Updating

Pulse updates itself from its own releases — three channels:

```
pulse --update           # your current channel (stable by default)
pulse --update stable    # thoroughly tested, fastest, most secure
pulse --update beta      # confirmed features, lightly tested
pulse --update dev       # newest, experimental
```

`--as-user` works here too: `pulse --update dev --as-user` updates the copy in
`~/.local/bin` instead of the system one.

## Building from source

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
