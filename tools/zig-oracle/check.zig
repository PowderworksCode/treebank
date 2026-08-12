//! Syntax-only Zig validity check for the treebank oracle.
//!
//! stdin:  one file path per line
//! stdout: "<path>\tvalid|invalid" per line
//!
//! The reference parser is Zig's own, `std.zig.Ast.parse(gpa, src, .zig)` —
//! the exact call the compiler makes to turn a file's text into a syntax
//! tree, and the same one `zig fmt` and every language server use. It never
//! resolves an `@import`, never runs `comptime`, never links: a file is
//! judged entirely on its own text, the property that makes CPython's
//! `compile()` and `ts.createSourceFile` usable the same way.
//!
//! WHICH ZIG. For every other language in this repo the question "is this
//! file valid?" has one answer. For Zig it has one answer *per compiler
//! version*, because the grammar itself moves between releases. So the
//! version is not an implementation detail of the oracle, it is half of the
//! oracle's output, and `ledger.json` records it next to the verdict counts
//! exactly as `generate_cli` records the tree-sitter CLI.
//!
//! This file is deliberately written to compile unmodified on 0.11.0 through
//! 0.16.0. That is not portability for its own sake — it is what makes a
//! version bump measurable instead of a leap: the same oracle source, built
//! by two toolchains, over the same corpus, gives the delta that says
//! whether the bump is safe. `Ast.parse`'s signature has not changed across
//! those six releases; everything around it has, which is why the I/O below
//! is raw syscalls rather than `std.fs` and `std.io` (both re-designed in
//! 0.15/0.16) and the allocator is `page_allocator` behind an arena rather
//! than the GPA that was renamed.
const std = @import("std");

/// `std.os` became `std.posix` in 0.12; the call names below did not change.
const posix = if (@hasDecl(std, "posix")) std.posix else std.os;

/// `openat` is the one opening call present in all six standard libraries;
/// 0.16 dropped the plain `open`/`openZ` wrappers entirely. `O` was a
/// namespace of integer constants through 0.11 and a packed struct from 0.12
/// on — the taken branch is comptime-known, so only it is analyzed.
fn openRead(path: [*:0]const u8) !posix.fd_t {
    if (comptime @hasDecl(posix.O, "RDONLY")) {
        return posix.openatZ(posix.AT.FDCWD, path, posix.O.RDONLY | posix.O.CLOEXEC, 0);
    } else {
        return posix.openatZ(posix.AT.FDCWD, path, .{ .ACCMODE = .RDONLY, .CLOEXEC = true }, 0);
    }
}

/// 0.16 removed the `close` and `write` wrappers along the way to the new
/// `std.Io` interface, so those two drop one layer to `posix.system`, which
/// is the OS backend std itself dispatches to (raw syscalls, or libc when
/// linking it). Not a Linux special case — the same layer on every target.
fn closeFd(fd: posix.fd_t) void {
    if (comptime @hasDecl(posix, "close")) posix.close(fd) else _ = posix.system.close(fd);
}

fn writeAll(fd: posix.fd_t, bytes: []const u8) !void {
    var i: usize = 0;
    while (i < bytes.len) {
        const n = if (comptime @hasDecl(posix, "write"))
            try posix.write(fd, bytes[i..])
        else n: {
            const r = posix.system.write(fd, bytes[i..].ptr, bytes.len - i);
            if (r < 0) return error.WriteFailed;
            break :n @as(usize, @intCast(r));
        };
        if (n == 0) return error.WriteFailed;
        i += n;
    }
}

/// Read a whole file into a NUL-terminated buffer, which is what `Ast.parse`
/// takes. Doubling read loop rather than fstat-then-read: no second syscall
/// shape to keep portable, and a file that grows under us cannot truncate.
fn readFileZ(gpa: std.mem.Allocator, path: [*:0]const u8) ![:0]u8 {
    const fd = try openRead(path);
    defer closeFd(fd);

    var buf = try gpa.alloc(u8, 64 * 1024);
    errdefer gpa.free(buf);
    var len: usize = 0;
    while (true) {
        if (len == buf.len) buf = try gpa.realloc(buf, buf.len * 2);
        const n = try posix.read(fd, buf[len..]);
        if (n == 0) break;
        len += n;
    }
    // +1 for the sentinel the parser requires.
    buf = try gpa.realloc(buf, len + 1);
    buf[len] = 0;
    return buf[0..len :0];
}

pub fn main() !void {
    // Two allocators on purpose. The path list and the output block live
    // for the whole run; everything a single file's parse touches lives in
    // an arena that is reset after each verdict, so a 70k-file batch stays
    // flat in memory instead of accumulating every AST it ever built.
    const gpa = std.heap.page_allocator;
    var arena_state = std.heap.ArenaAllocator.init(gpa);
    defer arena_state.deinit();

    // Slurp stdin whole. The batch is a path list, never large, and this
    // keeps the reading side to one syscall shape as well.
    var in = try gpa.alloc(u8, 64 * 1024);
    var in_len: usize = 0;
    while (true) {
        if (in_len == in.len) in = try gpa.realloc(in, in.len * 2);
        const n = try posix.read(0, in[in_len..]);
        if (n == 0) break;
        in_len += n;
    }

    // Accumulate output and flush in blocks: one write per file would be a
    // syscall per corpus entry, and the sweep drives tens of thousands.
    var out = try gpa.alloc(u8, 1 << 16);
    var out_len: usize = 0;

    var lines = std.mem.splitScalar(u8, in[0..in_len], '\n');
    while (lines.next()) |raw| {
        const path = std.mem.trim(u8, raw, " \t\r");
        if (path.len == 0) continue;

        defer _ = arena_state.reset(.retain_capacity);
        const scratch = arena_state.allocator();

        const pathz = try std.mem.concatWithSentinel(scratch, u8, &.{path}, 0);

        // An unreadable file is NOT an invalid file. Reporting it as
        // `invalid` looks harmless and is not: validate() is only ever
        // called on files the grammar already failed, and an invalid
        // verdict records the file as corpus NOISE. So a mistyped corpus
        // root would make every path unreadable, every grammar failure
        // noise, gap_files zero — and the sweep would report a flawless
        // grammar. A broken oracle must fail loudly, never quietly agree
        // with us; the reasoning is spelled out in
        // crates/treebank-cli/src/lang/exec_oracle.rs. So the read is
        // separate from the parse, and an I/O error is fatal.
        const src = readFileZ(scratch, pathz.ptr) catch |err| {
            var buf: [1024]u8 = undefined;
            const msg = std.fmt.bufPrint(&buf,
                "zig-oracle: cannot read {s}: {s}\n" ++
                "zig-oracle: this is an oracle failure, not a verdict; " ++
                "check the corpus root\n",
                .{ path, @errorName(err) },
            ) catch "zig-oracle: cannot read a corpus file\n";
            writeAll(2, msg) catch {};
            std.process.exit(1);
        };

        const verdict = blk: {
            // Everything below IS a verdict about the file's own content.
            // An embedded NUL is not Zig source; `Ast.parse` would stop at
            // it and call the truncated prefix valid. `Ast.parse`'s only
            // error is OOM, which is kept a verdict for the same reason
            // py-oracle keeps MemoryError one: it is reached by
            // pathological input rather than by a broken harness.
            if (std.mem.indexOfScalar(u8, src, 0) != null) break :blk false;
            var ast = std.zig.Ast.parse(scratch, src, .zig) catch break :blk false;
            defer ast.deinit(scratch);
            break :blk ast.errors.len == 0;
        };

        const label = if (verdict) "valid" else "invalid";
        const need = path.len + 1 + label.len + 1;
        if (out_len + need > out.len) {
            try writeAll(1, out[0..out_len]);
            out_len = 0;
        }
        @memcpy(out[out_len..][0..path.len], path);
        out_len += path.len;
        out[out_len] = '\t';
        out_len += 1;
        @memcpy(out[out_len..][0..label.len], label);
        out_len += label.len;
        out[out_len] = '\n';
        out_len += 1;
    }
    try writeAll(1, out[0..out_len]);
}
