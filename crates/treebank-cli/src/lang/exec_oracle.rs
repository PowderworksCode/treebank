//! Shared driver for reference parsers that have **no batch mode**.
//!
//! `stdin_oracle` covers the cheap shape: one long-lived process reads a
//! whole batch of paths and answers them all, so the interpreter starts
//! once. Some reference parsers cannot do that. `php -l` takes exactly one
//! file per invocation, and so do `bash -n` and `awk -f` — the only way to
//! ask them about a thousand files is a thousand processes.
//!
//! Check for a batch path before reaching for this, because the *tool* being
//! fork-per-file does not mean the *language* is. Lua looked like a member
//! of this class — `luac -p` takes one file per run, and the roadmap costed
//! it that way — but `loadfile(path, "t")` inside one long-lived `lua` is
//! the same C entry point (`luaL_loadfilex`) that `luac -p` itself calls,
//! so treebank-lua uses `stdin_oracle` and measures 0.17 s/1000 against
//! 0.48 s for the same `luac -p` parallelized to 16 cores. A batch path
//! through the same parser beats this driver by ~3x when one exists.
//!
//! When you do find one, test it adversarially rather than by agreement:
//! treebank-php built a batch oracle that matched `php -l` on all 1703
//! corpus files and still silently accepted ten classes of invalid PHP,
//! which only a negative battery caught. See that ledger's
//! `corpus.oracle_batch_note` and treebank-lua's
//! `oracle_not_luac.adversarial_battery`.
//!
//! That is 20–90× the per-file cost of every batch oracle, and it is paid in
//! process startup rather than in parsing: measured on 1000 files from the
//! top Packagist packages, `php -l` runs 15.4 ms/file serially, of which the
//! parse is a rounding error. An oracle in this class is not disqualified by
//! that number, it is just one that has to be run concurrently — the work is
//! embarrassingly parallel because every file is judged on its own text.
//!
//! Measured here on 16 cores, same 1000 files:
//!
//! | workers | s / 1000 |
//! |---|---|
//! | 1 | 15.4 |
//! | 8 | 1.24 |
//! | 16 | 0.81 |
//! | 32 | 0.84 |
//!
//! So the curve flattens at the core count and gives nothing back after it,
//! which is why the default is `available_parallelism()` rather than a
//! number someone picked. Under load the degradation is mild rather than
//! cliff-edged: with four other build sessions on the same box (load average
//! 6–8) the 16-worker figure moved to 0.87–0.97 s, still ~17× better than
//! serial, because each unit of work is a fork that exits rather than a
//! thread holding a core. `TREEBANK_ORACLE_JOBS` overrides the default for
//! the case where a human knows something the load average does not.
//!
//! This lives here, next to `stdin_oracle`, rather than in any one language,
//! because the next fork-per-file oracle should inherit it by calling it.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{anyhow, bail, Context, Result};

/// How many oracle processes to keep in flight.
///
/// `available_parallelism()` reports the cores this process may actually use
/// — it honours cgroup quotas and CPU affinity, so in a container it is the
/// container's share rather than the host's core count, which is the number
/// that matters here.
fn jobs() -> usize {
    if let Some(n) = std::env::var("TREEBANK_ORACLE_JOBS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| *n > 0)
    {
        return n;
    }
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
}

/// Run `program args… <file>` once per path, `jobs()` at a time, and read
/// each verdict off the child's exit status.
///
/// `reject_statuses` are the statuses the tool uses to say **this file is
/// not valid**, and they are a parameter rather than "any non-zero" on
/// purpose. `php -l` exits 255 for a syntax error but 1 for *could not open
/// input file*; `luac -p` and `bash -n` use different numbers again.
/// Collapsing every non-zero status into "invalid" would mean a mistyped
/// corpus root makes every file invalid, every failing file gets recorded as
/// corpus noise, `gap_files` falls to zero and the sweep reports a flawless
/// grammar. A broken oracle must fail loudly, never quietly agree with us —
/// so any status outside the list is an error, and it carries the child's
/// own output so the cause is on screen.
///
/// It is a *list* because one tool can spell rejection more than one way.
/// Measured, not anticipated: `bash -n` exits 2 for a syntax error almost
/// everywhere, but **1** when the error is inside an array-assignment word
/// list (`x=( a+([0-9]) )`), which is a real construct in a real corpus file
/// — linux's `tools/testing/selftests/wireguard/netns.sh`. Passing `&[1, 2]`
/// keeps the property that matters: 126 (bash refuses a binary or a
/// directory) and 127 (no such file) are still outside the list, so a
/// mistyped root still fails loudly instead of scoring every file invalid.
///
/// `hint` is shown when the process cannot be spawned at all, which is where
/// a missing interpreter surfaces.
pub fn run(
    program: &str,
    args: &[&str],
    reject_statuses: &[i32],
    hint: &str,
    srcroot: &Path,
    paths: &[String],
) -> Result<HashMap<String, bool>> {
    if paths.is_empty() {
        return Ok(HashMap::new());
    }
    let workers = jobs().min(paths.len());
    let cursor = AtomicUsize::new(0);

    // Each worker pulls the next index and keeps its own results, so the
    // only shared mutable state is the cursor and there is no lock on the
    // hot path. Work is handed out one file at a time rather than in
    // contiguous slices because corpus files differ in size by two orders of
    // magnitude, and a static split would leave workers idle behind whoever
    // drew the 200 KB file.
    let collected: Vec<Result<Vec<(String, bool)>>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| -> Result<Vec<(String, bool)>> {
                    let mut out = Vec::new();
                    loop {
                        let i = cursor.fetch_add(1, Ordering::Relaxed);
                        let Some(rel) = paths.get(i) else { break };
                        let full = srcroot.join(rel);
                        // Captured rather than inherited: at one process per
                        // file the diagnostics of a few hundred rejects would
                        // bury the sweep's own output, and on the error path
                        // below this is the only explanation there is.
                        let output = Command::new(program)
                            .args(args)
                            .arg(&full)
                            .output()
                            .with_context(|| hint.to_string())?;
                        let valid = match output.status.code() {
                            Some(0) => true,
                            Some(c) if reject_statuses.contains(&c) => false,
                            Some(c) => bail!(
                                "{program} exited with {c} on {} (expected 0 or one of \
                                 {reject_statuses:?}); this is an oracle failure, not a \
                                 verdict:\n{}{}",
                                full.display(),
                                String::from_utf8_lossy(&output.stdout).trim_end(),
                                String::from_utf8_lossy(&output.stderr).trim_end(),
                            ),
                            // Killed by a signal: the reference parser did not
                            // reach a verdict, so there is nothing honest to
                            // record for this file.
                            None => bail!(
                                "{program} was killed by a signal on {}",
                                full.display()
                            ),
                        };
                        out.push((rel.clone(), valid));
                    }
                    Ok(out)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|_| Err(anyhow!("oracle worker panicked"))))
            .collect()
    });

    let mut map = HashMap::new();
    for chunk in collected {
        map.extend(chunk?);
    }
    Ok(map)
}
