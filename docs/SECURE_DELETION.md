# Secure deletion and self-destruction — test programme

**Status: IMPLEMENTED AND TESTED** (Linux; macOS and Windows behaviour is
covered by the same tests and runs on CI runners).

Tracked as item B1 in `PENDING_DOCUMENTATION_WORK.md`.

## The claim this programme exists to constrain

NYEDArch does **not** claim secure erasure. Software cannot guarantee that data
is unrecoverable from modern storage: an SSD controller may have written the
block elsewhere and left the original cell intact, a copy-on-write filesystem
may retain older versions in snapshots, and a journal may hold fragments.
Overwriting through the filesystem asks the OS to change logical contents and
nothing more.

What the implementation does is a **best-effort destruction that reports what
actually happened**. The previous implementation was a bare `remove_file` whose
result was discarded, so it could not distinguish a capsule that was gone from
one still sitting on disk. That is now impossible: every destruction returns an
outcome, and the capsule prints it.

## The three categories, kept separate

| Category | Meaning |
|---|---|
| **Verified** | The path no longer exists, checked after the fact rather than inferred from a success return |
| **Best-effort** | Contents overwritten, flushed and synced before unlinking — reduces what a casual recovery finds, guarantees nothing about the physical medium |
| **Irreducible** | Free-space remnants, SSD wear-levelling, CoW snapshots, journals, and any copy made before destruction. Outside the reach of any user-space program |

## Outcomes the implementation can report

| Outcome | Meaning |
|---|---|
| `Removed` | Overwritten, then unlinked, then verified absent |
| `OverwrittenButPresent` | Contents replaced but the file could not be unlinked. The bytes at that path are no longer the capsule |
| `RemovedWithoutOverwrite` | Unlinked, but the contents were never overwritten and may survive in free space |
| `Failed` | Neither achieved |
| `NothingToDo` | Nothing at that path |

`Failed` and `OverwrittenButPresent` never report the path as cleared. A test
asserts that outcomes cannot overclaim.

## Cases exercised

| Case | Observed | Interpretation |
|---|---|---|
| Normal deletion | `Removed` | Overwritten, unlinked, verified absent |
| Contents actually overwritten | marker absent after pass | The overwrite reaches the file, not just the return value |
| Empty file | `Removed` | No special-casing needed |
| Large file (300 KB, exceeds the 64 KiB buffer) | `Removed` | The loop covers the whole length |
| Missing file | `NothingToDo` | Not an error |
| Directory passed by mistake | `Failed`, directory untouched | Refuses rather than recursing into something the caller did not mean |
| Read-only **directory** | not cleared, file survives | Removal needs write permission on the directory; reported honestly |
| Unwritable **file** | `RemovedWithoutOverwrite` | Cannot have been overwritten, and does not claim to be |
| Open handle held during deletion | reported per platform | Unix unlinks with handles open; Windows generally refuses. Either is fine; misreporting is not |
| Renamed capsule | `Removed` | Destroyed at its current path |
| **Copy made beforehand** | copy survives, intact | Destruction cannot reach copies — asserted by test, not assumed |
| Working tree with nested directories | every file reported, tree gone | Distinguishes "cleaned up" from "mostly cleaned up" |
| **Self-destruction of a running capsule** | `RemovedWithoutOverwrite` on Linux | See below |

## Layered destruction

Destruction is attempted through three independent mechanisms. They are
redundant on purpose: each covers a way the others can fail, and all are
idempotent, so whichever runs first simply wins.

| Layer | Mechanism | Covers |
|---|---|---|
| 1 | In-process overwrite, rename, unlink, verify | The normal case. The only layer whose result can be **confirmed** before exit |
| 2 | Platform one-shot scheduler — transient `systemd` unit, `at`, `launchd`, or a self-removing scheduled task | The child being killed. The job belongs to a system service, not to a process in the attacker's tree |
| 3 | Detached child process | The scheduler being absent or unavailable to this user |

Layers 2 and 3 both run when both are available. What actually engages, measured
on real runners rather than assumed:

| Platform | Layers engaged | Observed |
|---|---|---|
| Linux | 2 + 3 | `handed to a transient systemd job and a detached shell` |
| macOS | 1 | `capsule destroyed (overwritten and removed)` — macOS permits writing a running image, so the confirmed path succeeds outright |
| Windows | 2 + 3 | `handed to a self-removing scheduled task and a detached PowerShell` |

**Windows layer 2 now engages.** Two `schtasks` constraints had to be found by
watching which mechanisms actually took, rather than assuming the call worked:
`/tr` truncates beyond 261 characters, and `/z` is rejected without an end
boundary — either one caused creation to fail silently. The scheduled task now
performs the **removal** and deletes its own registration; the detached child
performs the **overwrite**. If the child is killed the file is still removed; if
the scheduler is unavailable the child still scrubs and removes.

A first diagnostic attempt was itself invalid and is recorded so it is not
repeated: probing `schtasks /create` from Git Bash fails with
`Invalid argument/option - 'C:/Program Files/Git/create'`, because MSYS
path-translates any argument beginning with `/`. The Rust code calls `schtasks`
directly and is unaffected by that, so the harness proved nothing about the
product.

### Why a scheduler and not a persistent service

A helper that reinstalls itself when killed is textbook malware persistence:
every endpoint product flags it, and `PHILOSOPHY.md` §5.4 draws the line at
NYEDArch's own artifacts. Each mechanism used here is a **documented one-shot
scheduling interface** that removes its own job after running - `--collect` on
systemd, `/z` on a scheduled task. Nothing is installed, no autostart entry is
created, and nothing outlives the deletion it was created to perform. That is
the difference between using a scheduler and establishing persistence, and it is
the reason the design stays on the right side of it.

### Two bugs the cross-platform tests caught

**BSD `wc -c` pads its output.** Where `shred` is absent the fallback computed
`sz=$(wc -c < file)`, and on macOS that yields `"    5800"`. Unquoted, the
`count=` argument word-split, `dd` failed, and the file was **unlinked without
ever being overwritten** — while reporting success. Found by the inode-witness
test on a macOS runner, not by reading the script. The length is now stripped
and the write is done in 4 KiB blocks rather than one-byte writes.

**`schtasks /tr` truncates beyond 261 characters.** The full scrub command did
not fit, so task creation failed and the Windows scheduler layer silently never
engaged — the run reported only the detached child. The scheduled command is now
compact, with a shorter delete-only form as a second fallback: the scheduler
guarantees *removal*, the child performs the *overwrite*.

Both were silent failures that looked like success, which is the failure mode
this whole module exists to eliminate.

### The path never reaches a shell unchecked

Every mechanism embeds the capsule path in a shell or PowerShell command, so a
path containing a quote, a newline, a carriage return, or a NUL is **refused
outright** rather than escaped. The check lives at the single entry point, not
in each of the three script builders: a guard that must be remembered three
times is one that will eventually be forgotten. Tested against injection
attempts including `/tmp/a\nrm -rf ~`.

## The running-executable problem — SOLVED by delegation

A process cannot overwrite its own running image: Linux returns `ETXTBSY` for a
write open, and Windows generally refuses to unlink a mapped image. The capsule
could unlink itself but never scrub its contents, leaving the bytes in free
space.

**The work is now handed to a process that outlives the capsule.** The command is
passed to the system shell **inline** — nothing is written to disk, so there is
no helper binary to find, tamper with, replace, or leave behind. It carries one
path and no secrets: everything it can do is scrub and delete that path. It waits
for the capsule's own pid to disappear, then overwrites (`shred -u -n 1`, or
`dd` from `/dev/urandom` where `shred` is absent) and unlinks. On Windows the
same job goes to a hidden PowerShell that overwrites the file's length with
random bytes and removes it.

### Order of operations, and the bug that made it matter

The obvious implementation is wrong, and was written first: unlink here, then
delegate the scrub. On Unix the unlink **succeeds even for a running image**, so
by the time the helper ran the path was gone and it overwrote nothing. The file
vanished, its bytes stayed in free space, and the user was told it had been
destroyed — the exact false assurance this module exists to prevent.

So the overwrite is attempted in place first. Only if it succeeds is the file
unlinked here, where the result can be verified. If it fails — the normal case
for a running image — the file is handed over **intact**, so the helper still
has something to scrub.

### Verified at the inode, not the filename

Removal alone proves nothing about the contents, and once the path is gone there
is nothing left to inspect. So the test keeps a **second hard link** to the same
inode: the helper scrubs and unlinks its path, and the surviving link shows
whether the bytes were really replaced. They are.

Observed on a real one-shot capsule:

```
NYEDArch: capsule scheduled for destruction (overwrite and removal handed to a
detached shell that runs after this process exits (requested, not confirmed))
immediately after exit: present (766,816 bytes)   <- intact, so it can be scrubbed
after 4s:               gone - scrubbed then removed
```

### What is and is not guaranteed

Under a **software-only** adversary who does not interfere after the capsule
runs, destruction completes: the OS scheduler owns the job, and killing the
child no longer prevents it. Verified on all three platforms.

It is still **not** a guarantee against: an adversary who removes the scheduled job as well as the child, denies both a
shell, powers the machine off mid-scrub, or snapshotted the disk beforehand -
and nothing here reaches copies made earlier. The
outcome is reported as `Delegated` — deliberately a separate state from
`Removed`, because this process **requested** destruction and cannot observe
whether it completed. Reporting it as removed would be a claim nobody verified.

## Remaining limitation (KNOWN LIMITATION)

Observed on a real one-shot capsule:

```
NYEDArch: extraction complete - 1 files, 1 dirs, 33 bytes -> /tmp/os_out
NYEDArch: capsule destroyed (removed without overwriting: Text file busy (os error 26))
```

A Linux process cannot open its own running image for writing — `ETXTBSY`. So
the overwrite pass fails and the capsule is unlinked without its contents being
replaced. The file disappears from its path; its bytes may remain in free space
until reused.

This is reported rather than hidden. Windows generally refuses to unlink a
running image at all, in which case the rename step at least detaches the
capsule from its path and the user is told plainly:

```
NYEDArch: the capsule could NOT be destroyed - ...
NYEDArch: delete it yourself. Its payload stays encrypted either way.
```

A future option is a detached helper that overwrites after the process exits,
but the helper is itself an artifact an attacker can prevent from running, so it
would add complexity without adding a guarantee. It is not implemented.

## Why this is defence in depth, not the boundary

The payload stays encrypted whether or not the bytes survive. Destruction exists
to make repeated hostile experimentation expensive — a failed experiment can
consume the attacker's specimen — not to make recovery impossible. Deletion is
an anti-iteration measure. It is not, and is never described as, an erasure
guarantee.

## Note on the test environment

Two permission cases cannot demonstrate anything as root, which bypasses
filesystem permission checks. They detect that condition behaviourally and skip
rather than weakening their assertions, so they still run on any ordinary
account, including the CI runners.
