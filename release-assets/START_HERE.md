# NYEDArch — Start Here

NYEDArch turns files into a **capsule**: a standalone executable that carries your
data together with its own authorization policy and decryption logic. It opens
only when every protection you enabled is satisfied on the machine running it.

## 0. Install

```bash
./install                # per-user install; no administrator rights required
./install --verify       # check integrity only
./install --uninstall    # remove it again
./install --prefix DIR   # install somewhere specific
```

On Windows use `install.exe` with the same options.

**On macOS** the installer creates a real application bundle at
`~/Applications/NYEDArch.app`. That matters for more than tidiness: macOS only
offers location to an application bundle that declares why it wants it, so
launching NYEDArch from Applications is what makes the native location provider
work. Run the command-line client from a terminal instead and location falls
back to a browser prompt, which works too.

The installer verifies checksums before writing anything, does not modify your
shell profile or registry, and prints the exact PATH line for you to apply.

You can also skip installing and run `./bin/nyedarch` directly from here.

## 1. Read the licence first

`EULA.md`. It states plainly what system information is collected, what happens
if you lose key material, and what NYEDArch does **not** guarantee. The client
requires you to accept it before it will run.

## 2. Seal something

```bash
./bin/nyedarch seal ./my_folder ./capsule_project "a strong passphrase"
```

Two protections are always on and cannot be disabled: **machine** and
**passphrase**. Add more:

```bash
./bin/nyedarch seal ./my_folder ./capsule_project "passphrase" \
    --time 14:00 --time-tolerance-min 15 --tz-offset-min 330 \
    --location 150 \
    --trust colleague.nyfp \
    --one-shot
```

| Flag | Effect |
|---|---|
| `--time` | Opens only within a daily window (recurring — it does not expire the capsule) |
| `--location` | Opens only within a region, at the accuracy you specify |
| `--trust` | Adds another trusted machine from an exported `.nyfp` record |
| `--one-shot` | Best-effort self-destruct after a verified extraction |

## 3. Build the capsule

Locally:

```bash
cd ./capsule_project && sh ./stage-capsule.sh
```

Or remotely on GitHub Actions, which is the intended production path:

```bash
export NYEDARCH_GITHUB_TOKEN=...          # read from the environment, never argv
./bin/nyedarch build ./capsule_project <owner> <repo> private
```

Either way you get one `.nyarch` capsule.

## 4. Run it

```bash
./nyedarch-<id>.nyarch ./output          # Linux and macOS
./bin/nyedarch run ./nyedarch-<id>.nyarch ./output    # any platform
```

**Windows:** typing `capsule.nyarch` into cmd or PowerShell will not work,
because Windows decides what is executable from the file extension. Use
`nyedarch run`, drag the capsule onto the desktop client, or add `.NYARCH` to
your `PATHEXT`.

## 5. Add another trusted machine

On the other machine:

```bash
./bin/nyedarch export mymachine.nyfp Finance Laptop
```

Send that file back and pass it with `--trust`. The record is authenticated, so
editing its labels invalidates it.

## What to be clear-eyed about

- **A capsule is not a backup.** Lose the passphrase, the authorized machine, or
  your key material and the data is gone. There is no recovery and no backdoor.
- **The time protection is not an expiry.** "Every day at 14:00" means every day.
- **Self-destruct is best effort.** Software cannot guarantee erasure on SSDs or
  copy-on-write filesystems.
- **NYEDArch is not unbreakable.** `docs/ANTI_RE_ANALYSIS.md` §3 states exactly
  what someone holding a capsule can extract from it. Read it before deciding
  what to protect this way.

## Where to go next

| Question | Document |
|---|---|
| How does it work? | `docs/ARCHITECTURE.md` |
| Is it actually secure? | `docs/THREAT_MODEL.md`, `docs/ADVERSARIAL_REVIEW.md` |
| What are the weaknesses? | `docs/ANTI_RE_ANALYSIS.md` |
| How are keys handled? | `docs/KEY_HIERARCHY.md` |
| Everything, in depth | `NYEDArch_Technical_Documentation.docx` |
| What is planned next | `docs/license-server/` (design only, not built) |
