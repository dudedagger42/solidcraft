# SolidCraft all-in-one build

> [!WARNING]
> **The simulation features (Airflow studies, the fan and compressor tools) are experimental and
> not ready for engineering use.** They have only passed simple self-checks and are not yet
> validated against published benchmarks or measurements. Don't base design, safety or
> certification decisions on their results; confirm anything that matters with a validated
> solver or a physical test.

This branch (`try-main` on [dudedagger42/solidcraft](https://github.com/dudedagger42/solidcraft)) is
upstream SolidCraft (the 0.21.0 release plus `main` since) with the fixes and features that are open
as pull requests upstream but not merged yet, in one build. It is not an official tangency-labs
release. Everything of ours that upstream merges drops out of this list over time.

What it adds on top of upstream:

- our open pull requests (bug fixes, quality-of-life and performance work, the simulation workspace);
- Make-o-Rama's #4767, **drag to extrude** (off by default: Preferences → Navigation → "Direct interaction");
- lofts with **up to 16 guide rails** (kept in this build while upstream's kernel work is paused).

## Download

Get the files from the [releases page](https://github.com/dudedagger42/solidcraft/releases): the
newest release named "all-in-one".

### Windows (10/11, 64-bit)

1. Download `solidcraft-<version>-all.zip`.
2. Right-click it → **Extract All…** (don't run it from inside the zip).
3. Open the folder and run `solidcraft.exe`.
4. If SmartScreen says "Windows protected your PC": **More info → Run anyway** (the build is unsigned).

Nothing is installed: delete the folder to remove it.

### macOS (11 or later; Apple silicon and Intel)

1. Download `solidcraft-<version>-macos-universal.dmg`.
2. Open it and drag **SolidCraft** into **Applications**.
3. The first time, **right-click SolidCraft in Applications → Open**, then **Open** again. The app is
   ad-hoc signed, not notarized, so a plain double-click the first time only says it can't be checked.
   After that it opens normally.
   - If macOS says the app "is damaged and can't be opened", run this once in Terminal, then open it
     again: `xattr -dr com.apple.quarantine /Applications/SolidCraft.app`

It installs next to an official SolidCraft only if you rename one of them: both are called
SolidCraft.app. Your designs are untouched either way.

## Build it yourself

With [Rust](https://rustup.rs) installed:

```sh
git clone --branch try-main https://github.com/dudedagger42/solidcraft.git
cd solidcraft
cargo run --release -p solidcraft
```

On macOS, `packaging/macos/package.sh --arch aarch64` (Apple silicon) or `--arch universal` makes the
same `.dmg` as the release. Every push of this branch also builds the macOS app on GitHub
(Actions → "all-in-one macOS"); the run's artifact holds the `.dmg`.

## Problems

If something looks wrong, check whether plain upstream SolidCraft does it too. If only this build
does, open an issue on [dudedagger42/solidcraft](https://github.com/dudedagger42/solidcraft/issues);
otherwise report it upstream at [tangency-labs/solidcraft](https://github.com/tangency-labs/solidcraft/issues).
