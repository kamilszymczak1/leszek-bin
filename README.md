# Leszek

A simple Rust app for creating music.

## Authors

- Hubert Wasilewski @hubertwas
- Kamil Szymczak @kamilszymczak1

## Goals for the project

- Implement an interpreter so that we can define music in a custom language, not in Rust source code,
- Make the interpreter read live changes to a file and change the music according to them,
- Include multiple different sounds, samples, effects, and instruments,
- Allow multiple people to collaborate on the same music project at the same time

## Installation

### Installing SuperDirt

SuperDirt is a SuperCollider plugin ("Quark") that serves as the audio engine and provides the default sample library.

#### Step 1: Configure Audio Group

Add your user to the `audio` group for proper audio device access:

```bash
sudo usermod -a -G audio $USER
```

**Log out and back in** for the change to take effect. Verify with:

```bash
groups | grep audio
```

#### Step 2: Package Preconfiguration

**Install dependencies:**

Debian/Ubuntu/Mint/Pop!_OS:
```bash
sudo apt update
sudo apt install git jackd2 qjackctl zlib1g-dev gcc g++ ghc cabal-install
```

Arch/Manjaro:
```bash
sudo pacman -Syu
sudo pacman -Sy git jack2 qjackctl
```

Fedora:
```bash
sudo dnf install git-core qjackctl gcc-c++ cabal-install
```

**Remove conflicts (Arch/Manjaro only):**
```bash
sudo pacman -R lib32-mesa-demos mesa-demos
```

#### Step 3: Install SuperCollider

**Debian/Ubuntu/Mint/Pop!_OS:**
```bash
sudo apt update
sudo apt install supercollider sc3-plugins sc3-plugins-language sc3-plugins-server
```

**Arch/Manjaro:**
```bash
sudo pacman -Syu
sudo pacman -S supercollider sc3-plugins
```

**Fedora:**
```bash
sudo dnf install supercollider
# sc3-plugins from third-party repo (optional):
sudo dnf copr enable ycollet/audinux
sudo dnf install supercollider-sc3-plugins
sudo dnf copr disable ycollet/audinux
```

#### Step 4: Install SuperDirt

1. Check the latest SuperDirt version:
```bash
git ls-remote https://github.com/musikinformatik/SuperDirt.git | grep tags | tail -n1 | awk -F/ '{print $NF}'
```

2. Start the SuperCollider interpreter:
```bash
sclang
```

3. Install SuperDirt (update the version number if needed):
```supercollider
Quarks.checkForUpdates({Quarks.install("SuperDirt", "v1.7.4"); thisProcess.recompile()})
```

4. Wait for the installation to complete (it processes in the background), then press `Ctrl+D` to exit.

## Running the Project

### Step 1: Start the SuperDirt Server

1. Start the SuperCollider interpreter:
```bash
sclang
```

2. Boot the SuperDirt server:
```supercollider
SuperDirt.start
```

Keep this terminal running while using Leszek.

### Step 2: Run Leszek

Leszek supports three modes of operation:

#### Standalone Mode

Run Leszek with a local file:

```bash
cargo run -- standalone myfile.code
```

This watches the specified file for changes and plays the pattern through SuperDirt.

#### Collaborative Mode

Leszek supports real-time collaboration where multiple users can work on patterns simultaneously, with all patterns combined and played in parallel.

**Start the collaboration server** (on one machine):

```bash
cargo run -- server
```

By default, the server listens on `0.0.0.0:9999`. To use a different address:

```bash
cargo run -- server --bind 192.168.1.100:8888
```

**Connect as a client** (on each participant's machine):

```bash
cargo run -- collab myfile.code
```

By default, clients connect to `127.0.0.1:9999`. To connect to a remote server:

```bash
cargo run -- collab --server 192.168.1.100:9999 myfile.code
```

Each client edits their own local file. When any client saves changes:
1. The update is sent to the server
2. The server broadcasts it to all other clients
3. Each client combines all patterns using `in_parallel` and plays them locally

This allows multiple musicians to jam together in real-time, each contributing their own patterns to the mix.

## Syntax

Leszek uses a simple, expressive syntax for defining musical patterns. Here's an example from [demo.code](demo.code):

```
n([0, 8, 4, 6, 9, 2, 0, -2]
    .fc
    .fast(3)
    .add(slow(4, cat([0, 2])))
    .scale(["minor", "major", "lydian", ["minor", "major"].cat].cat)
    .transpose(["c", "c", "f", ["f", "dh"].fc].cat)
    .struct([1, 1, 1, [~, 1].fc, 1, 1, 1, ~].fc)
    .slow(2)
    )
    @room(0.2)
    @s("supervibe")
    @velocity(fast(8, fast(2, cat([0.9, 0.6, 0.7]))))
    @accelerate([0.0, 0.0, 0.01].cat.slow(4))
```

### Key elements:

- `n([...])`: defines note values as a list
- `.fc`: creates a "fast cycle" pattern from the preceding list
- `.fast(factor)` / `.slow(factor)`: speeds up or slows down the pattern
- `.add(pattern)`: adds values to the pattern for transposition
- `.scale(pattern)`: applies a musical scale like `"minor"`, `"major"`, or `"lydian"`
- `.transpose(pattern)`: transposes to a root note like `"c"`, `"f"`, or `"dh"` (D#)
- `.struct(pattern)`: applies a rhythmic structure to the pattern
- `cat([...])` / `.cat`: concatenates patterns sequentially
- `~`: represents a rest (silence)
- `@s("supervibe")`: sets the sound or sample to use
- `@room(0.2)`: applies reverb effect
- `@velocity(pattern)`: controls note velocity and volume
- `@accelerate(pattern)`: applies pitch acceleration effect

### Method chaining

The syntax supports method chaining with `.` for pattern transformations:

```
[0, 1, 2].fc.fast(2).slow(4)
```

### Control parameters

Use `@` to set SuperDirt control parameters:

```
n([0, 2, 4].fc)
    @s("piano")
    @room(0.3)
    @velocity(0.8)
```


