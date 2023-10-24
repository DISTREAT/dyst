const std = @import("std");
const Allocator = std.mem.Allocator;

const Release = struct {
    allocator: Allocator,
    name: []const u8,
    tag_name: []const u8,
    assets: []Asset,

    pub fn deinit(self: Release) void {
        self.allocator.free(self.name);
        self.allocator.free(self.tag_name);
        self.allocator.free(self.assets);
    }
};

const Asset = struct {
    allocator: Allocator,
    name: []const u8,
    content_type: []const u8,
    browser_download_url: []const u8,
    size: i64,

    pub fn deinit(self: Asset) void {
        self.allocator.free(self.name);
        self.allocator.free(self.content_type);
        self.allocator.free(self.browser_download_url);
    }
};

pub fn fetchReleases(allocator: Allocator, repository: []const u8) ![]Release {
    var client = std.http.Client{ .allocator = allocator };
    defer client.deinit();

    const api_url = try std.fmt.allocPrint(allocator, "https://api.github.com/repos/{s}/releases", .{repository});
    defer allocator.free(api_url);

    const api_uri = try std.Uri.parse(api_url);

    var request = try client.request(.GET, api_uri, .{ .allocator = allocator }, .{});
    defer request.deinit();

    try request.start();
    try request.wait();

    if (request.response.status != .ok) {} // TODO: error handling

    const body = try request.reader().readAllAlloc(allocator, 1000000); // 10MB seem more than enough?
    defer allocator.free(body);

    const parsed_body = try std.json.parseFromSlice(std.json.Value, allocator, body, .{ .ignore_unknown_fields = true });
    defer parsed_body.deinit();

    var releases = std.ArrayList(Release).init(allocator);

    for (parsed_body.value.array.items) |json_release_object| {
        const json_release = json_release_object.object;
        const release_name = json_release.get("name").?.string;
        const release_tag_name = json_release.get("tag_name").?.string;

        var assets = std.ArrayList(Asset).init(allocator);

        for (json_release.get("assets").?.array.items) |json_asset_object| {
            const json_asset = json_asset_object.object;
            const asset_name = json_asset.get("name").?.string;
            const asset_content_type = json_asset.get("content_type").?.string;
            const asset_browser_download_url = json_asset.get("browser_download_url").?.string;
            const asset_size = json_asset.get("size").?.integer;

            try assets.append(Asset{
                .allocator = allocator,
                .name = try allocator.dupe(u8, asset_name),
                .content_type = try allocator.dupe(u8, asset_content_type),
                .browser_download_url = try allocator.dupe(u8, asset_browser_download_url),
                .size = asset_size,
            });
        }

        try releases.append(Release{
            .allocator = allocator,
            .name = try allocator.dupe(u8, release_name),
            .tag_name = try allocator.dupe(u8, release_tag_name),
            .assets = try assets.toOwnedSlice(),
        });
    }

    return releases.toOwnedSlice();
}

pub fn freeReleases(allocator: Allocator, releases: []Release) void {
    for (releases) |release| {
        for (release.assets) |asset| {
            asset.deinit();
        }
        release.deinit();
    }
    allocator.free(releases);
}
