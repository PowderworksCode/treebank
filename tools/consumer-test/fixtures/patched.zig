const builtin = @import("builtin");

// 0003: async and await are ordinary identifiers from Zig 0.15 on
const Fixture = struct {
    await: u8,
    async: u8,
};

const Tag = enum { async, await, other };

// 0003: asm clobbers became a struct literal in Zig 0.15
pub fn barrier() void {
    asm volatile ("" ::: .{ .memory = true });
}

// ...and the pre-0.15 string form still parses, because this grammar has to
// read both eras of a language whose syntax moves between releases
pub fn barrierOld() void {
    asm volatile ("" ::: "memory");
}

// 0004: the pointee of a pointer may itself be an `if` type expression
const Command = struct {
    status: *if (builtin.link_libc) c_int else u32,
};
