// Diagnostic sibling of check.zig: same input contract, but emits the first
// parse error's tag and byte offset instead of a bare verdict. Error TAGS
// rather than rendered messages, because rendering needs a writer and the
// writer API is exactly what moved in 0.15/0.16 — the tag is the stable
// handle and it is what clusters on.
const std = @import("std");
const posix = if (@hasDecl(std, "posix")) std.posix else std.os;

fn openRead(path: [*:0]const u8) !posix.fd_t {
    if (comptime @hasDecl(posix.O, "RDONLY")) {
        return posix.openatZ(posix.AT.FDCWD, path, posix.O.RDONLY | posix.O.CLOEXEC, 0);
    } else {
        return posix.openatZ(posix.AT.FDCWD, path, .{ .ACCMODE = .RDONLY, .CLOEXEC = true }, 0);
    }
}
fn closeFd(fd: posix.fd_t) void {
    if (comptime @hasDecl(posix, "close")) posix.close(fd) else _ = posix.system.close(fd);
}
fn writeAll(fd: posix.fd_t, bytes: []const u8) !void {
    var i: usize = 0;
    while (i < bytes.len) {
        const n = if (comptime @hasDecl(posix, "write")) try posix.write(fd, bytes[i..]) else n: {
            const r = posix.system.write(fd, bytes[i..].ptr, bytes.len - i);
            if (r < 0) return error.WriteFailed;
            break :n @as(usize, @intCast(r));
        };
        if (n == 0) return error.WriteFailed;
        i += n;
    }
}
fn readFileZ(gpa: std.mem.Allocator, path: [*:0]const u8) ![:0]u8 {
    const fd = try openRead(path);
    defer closeFd(fd);
    var buf = try gpa.alloc(u8, 64 * 1024);
    var len: usize = 0;
    while (true) {
        if (len == buf.len) buf = try gpa.realloc(buf, buf.len * 2);
        const n = try posix.read(fd, buf[len..]);
        if (n == 0) break;
        len += n;
    }
    buf = try gpa.realloc(buf, len + 1);
    buf[len] = 0;
    return buf[0..len :0];
}

pub fn main() !void {
    const gpa = std.heap.page_allocator;
    var arena_state = std.heap.ArenaAllocator.init(gpa);
    defer arena_state.deinit();

    var in = try gpa.alloc(u8, 1 << 20);
    var in_len: usize = 0;
    while (true) {
        if (in_len == in.len) in = try gpa.realloc(in, in.len * 2);
        const n = try posix.read(0, in[in_len..]);
        if (n == 0) break;
        in_len += n;
    }

    var lines = std.mem.splitScalar(u8, in[0..in_len], '\n');
    while (lines.next()) |raw| {
        const path = std.mem.trim(u8, raw, " \t\r");
        if (path.len == 0) continue;
        defer _ = arena_state.reset(.retain_capacity);
        const scratch = arena_state.allocator();
        const pathz = try std.mem.concatWithSentinel(scratch, u8, &.{path}, 0);
        const src = readFileZ(scratch, pathz.ptr) catch continue;
        var ast = std.zig.Ast.parse(scratch, src, .zig) catch continue;
        defer ast.deinit(scratch);
        if (ast.errors.len == 0) continue;
        const e = ast.errors[0];
        const off = ast.tokens.items(.start)[e.token];
        var line: usize = 1;
        for (src[0..off]) |c| { if (c == '\n') line += 1; }
        var buf: [512]u8 = undefined;
        const msg = std.fmt.bufPrint(&buf, "{s}\t{s}\t{d}\n", .{ path, @tagName(e.tag), line }) catch continue;
        try writeAll(1, msg);
    }
}
