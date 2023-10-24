const std = @import("std");
const argtic = @import("zig-argtic");
const Allocator = std.mem.Allocator;

fn command_line_interface(allocator: Allocator) !void {
    const flag_help = argtic.Flag{
        .name = "help",
        .short = 'h',
        .abort = true,
        .help = "Displays this help message",
    };

    const specification = argtic.ArgumentSpecification{
        .name = "dyst",
        .short_description = "A distilled package manager for binaries released as GitHub release assets.",
        .flags = &.{flag_help},
    };

    const argument_vector = try std.process.argsAlloc(allocator);
    defer std.process.argsFree(allocator, argument_vector);

    const arguments = argtic.ArgumentProcessor.parse(
        allocator,
        specification,
        argument_vector[1..],
    ) catch |e| return try argtic.defaultErrorHandler(e);
    defer arguments.deinit();

    if (arguments.isArgument("help")) {
        try argtic.generateHelpMessage(arguments.tokenizer.specification);
    } else {
        try argtic.generateUsageMessage(arguments.tokenizer.specification);
    }
}

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer std.debug.assert(gpa.deinit() == .ok);
    const allocator = gpa.allocator();

    try command_line_interface(allocator);
}
