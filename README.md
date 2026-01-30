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

#### Step 2: Install SuperCollider

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

#### Step 3: Install SuperDirt

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

In a separate terminal, run:

```bash
cargo run
```

## Syntax

Leszek uses a simple, expressive syntax for defining musical patterns. Here's an example from [demo.code](demo.code):

```
par([
    n(scale(slow(2, cat(["cminor", "cmajor"])), fc([0, 3, 2, 1])))
        .s("arpy")
])
```

### Key elements:

- `par([...])` — plays patterns in parallel (simultaneously)
- `n(...)` — defines note values
- `scale(name, pattern)` — applies a musical scale to the pattern
- `slow(factor, pattern)` — slows down the pattern by the given factor
- `cat([...])` — concatenates patterns sequentially
- `fc([...])` — creates a "fast cycle" pattern from a sequence of values
- `.s("arpy")` — sets the sound/sample to use (e.g., "arpy" is an arpeggio synth)

Numbers in the pattern represent scale degrees that get played in sequence.


