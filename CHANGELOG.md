# Changelog

All notable changes to Asphalt are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.2] - 2026-07-07

This release updates our `rbx-binary` and `rbx-xml` dependencies, which should fix issues users are having with syncing animations.

## [2.0.1] - 2026-06-01

Fixed duplicate Studio asset URI scheme by @revvy02 in #160

Fixed duplicate asset race conditions on Cloud and Studio targets causing inconsistencies between output of the first and following sync runs

## [2.0.0] - 2026-04-30

No changes have been made since the last RC, but if you're coming from 1.2.0:

Assets are now synced concurrently. This will result in significant speedups when syncing many new assets at once

Only files with supported extensions are read

The --target flag has been changed to a subcommand under sync

- `asphalt sync` remains valid to sync using the cloud target
- Breaking: `asphalt sync --target [cloud|studio|debug]` is now `asphalt sync [cloud|studio|debug]`
- Breaking: `asphalt sync --dry-run` is now `asphalt sync cloud --dry-run`

Web client properly uses Roblox's rate limit headers and works with concurrent uploads

Web client permanently gives up when it encounters an unknown error (like when the user has been moderated)

The progress bar is nicer and gives some statistics

Supported providing an argument for the "project" (which is really just the working directory that contains asphalt.toml). Useful for monorepos.

Removed the `ROBLOX_CONTENT_PATH` environment variable in favor of `ROBLOX_STUDIO_PATH`.

When rendering SVGs, a stylesheet is now added with currentColor set to white (thanks @askfalse in #158)

## [2.0.0-rc.5] - 2026-03-21

Fixed a sync bug where brace-style glob paths could be pruned during directory traversal and match zero assets. In cloud sync, that could incorrectly clear Asphalt's lockfile.

## [2.0.0-rc.4] - 2026-03-16

### Changes since the last RC

- When rendering SVGs, a stylesheet is now added with currentColor set to white (thanks @askfalse in #158)

## [2.0.0-rc.3] - 2026-01-20

### Changes since the last RC

- Only processes assets that are about to be synced

## [2.0.0-rc.2] - 2026-01-09

### Changes since the last RC

- Supported providing an argument for the "project" (which is really just the working directory that contains asphalt.toml). Useful for monorepos.
- Removed the `ROBLOX_CONTENT_PATH` environment variable in favor of `ROBLOX_STUDIO_PATH`.

## [2.0.0-rc.1]

- Assets are now synced concurrently. This will result in significant speedups when syncing many new assets at once
- Only files with supported extensions are read
- The `--target` flag has been changed to a subcommand under sync
  - `asphalt sync` remains valid to sync using the cloud target
  - Breaking: `asphalt sync --target [cloud|studio|debug]` is now `asphalt sync [cloud|studio|debug]`
  - Breaking: `asphalt sync --dry-run` is now `asphalt sync cloud --dry-run`
- Web client properly uses Roblox's rate limit headers and works with concurrent uploads
- Web client permanently gives up when it encounters an unknown error (like when the user has been moderated)
- The progress bar is nicer and gives some statistics

## [1.2.0]

- Roblox model uploads (.rbxm/.rbxmx) are now supported!
- Animation uploads no longer require a cookie! As such, Asphalt has dropped support for cookie authentication.
- GLTF models are now supported (but.. you're going to use the 3D Importer, right?)
- We now build for `aarch64-unknown-linux-gnu`.

The README has been refreshed with new information about supported asset types.

## [1.1.0] - 2025-09-13

Asphalt can now output the `Content` data type in place of string URIs. This can be toggled in config with `codegen.content`. Thanks to @daimond113 in #146.

## [1.0.0] - 2025-09-02

This marks the full release of Asphalt 1.0! A lot has changed since 0.9.1 that I won't describe here, but feel free to check the previous release notes.

Most users have been using the pre-release since now, but if you haven't (idk how considering 0.9.1 hasn't worked in months), this is a breaking change, so check the README to make sure you're using the correct configuration format.

## [1.0.0-pre.16] - 2025-09-02

Made some internal changes to the sync process.

Fixed the dry run mode making changes to files.

## [1.0.0-pre.15] - 2025-08-14

Videos are now properly supported.

Added the `--expected-price` arg to the sync and upload commands. This provides Roblox with the amount of Robux that you are willing to spend on each non-free asset upload (such as videos).

Asphalt now reads your files much quicker.

## [1.0.0-pre.14] - 2025-07-29

Files copied used for the Studio sync target are now named by their hashes, causing Studio to correctly reload the asset. Thanks to @iminlikewithyou in #139.

Changed the file skip warning to a debug log.

## [1.0.0-pre.13] - 2025-07-21

Implemented parallel file walking (more faster!)

Added disclaimer comments to the top of generated files

The CLI sync argument `suppress-duplicate-warnings` has been removed in favor of the `warn_each_duplicate` input config option, which has the same, but inverted behavior, and is defaulted to true.

## [1.0.0-pre.12] - 2025-07-19

This release includes a rewrite of the upload code, which brings some changes:

- Images are no longer downloaded, so web requests when uploading images are effectively halved.
- Cookies are no longer required for non-animation uploads. They are still required for animation uploads.
- We hit the Roblox API a little less hard to reduce rate limiting.

It should work okay, but I've only tested it lightly. Let me know. I'm also unsure if rbxmx animations are uploaded, but I don't have access to one right now. Also, let me know.

## [1.0.0-pre.11] - 2025-06-14

Fixed sync runs with nonexistent lockfiles not working.

## [1.0.0-pre.10] - 2025-05-31

This release fixes an issue where brand new lockfiles were being detected as out of date.

## [1.0.0-pre.9] - 2025-05-22

Codegen configuration is now optional

When using flat codegen style, asset key paths are now normalized to Unix-style on Windows

`Migrate-lockfile` command doesn't clog up `--help` output (thanks to @wackbyte in #124)

## [1.0.0-pre.8] - 2025-04-29

Fixes a bug introduced in the previous release that caused code to not generate for duplicate assets.

## [1.0.0-pre.7] - 2025-04-29

The lockfile structure has changed. Asset entries are now keyed by file hashes instead of file paths. Users can now freely restructure their assets without re-uploads!

Inputs are still saved in the lockfile separately, so moving a file from one input to another, or renaming an input, would still cause a re-upload.

This feature has not been extensively tested, so give it a whirl and let me know if you have any problems!

Asphalt will migrate your lockfile for you with the `asphalt migrate-lockfile` command. No reupload required.

As an added benefit of the above change, we now detect duplicate files, skip them, and warn you about each one.

To disable the warnings, use the `--suppress-duplicate-warnings` flag.

Added documentation for `bleed` field on input configs.

## [1.0.0-pre.6] - 2025-04-24

Fixed an issue where Asphalt could no longer update to groups. The caveat is that Asphalt now requires both an API key and a cookie for the cloud target. See the authentication section of the README for more info.

As far as I know, there is no other feasible way to grab image IDs without cookie authentication for groups. Please check out my DevForum post on the issue. More attention may motivate Roblox to fix this problem.

## [1.0.0-pre.5] - 2025-04-06

Adapted to Roblox's new Asset Delivery API–image uploads should now be fixed.

You now need the `legacy-asset:manage` permission scope on your API key in order to upload image assets.

## [1.0.0-pre.4] - 2025-03-30

Fixed an issue where changed assets weren't being re-uploaded.

## [1.0.0-pre.3] - 2025-03-28

Fixed issue with API Keys being required where they shouldn't again

## [1.0.0-pre.2] - 2025-03-27

Fixes codegen configuration being required

Fixes the API key being required when the context doesn't involve assets being uploaded

Fixes Studio target clearing the lockfile

## [1.0.0-pre.1] - 2025-03-26

Asset display names are now properly trimmed to avoid errors.

## [1.0.0-pre.0] - 2025-03-26

Asphalt has been rewritten from the ground up to better support large projects.

Here's a list of the key end-user changes:

- It now supports multiple asset inputs and outputs.
- Assets are now hashed prior to processing (such as alpha bleeding and SVG conversion). This means that updates to Asphalt's processing algorithms in future versions will not cause a re-upload. Unforeseen re-uploads should now be treated as breaking. I felt this was the best and simplest solution. If you wish to re-upload your assets in any case, you're welcome to delete their lockfile entries.
- Work has been done to facilitate concurrent reading, processing, and syncing of assets. There is more room for improvement here, but it should be faster.
- The terminal UI has been improved and now features progress spinners and bars.
- The dry run feature actually works now, and will error if your assets are not up-to-date.
- There's a new command to directly upload an asset, independently from the lockfile. The asset ID or link will be returned.

This is a pre-release, has not undergone much testing, and bugs are inevitable. Please report any issues you encounter to the Asphalt project thread in the ROSS server.

**Migration guide:**

1. Install this version using whatever method you prefer.
2. Adhere to the new configuration format. See the README for details.
3. Run `asphalt migrate-lockfile <input-name>` to update your lockfile to the new format.
4. Run `asphalt sync`. In theory, nothing should re-upload.

## [0.9.1] - 2025-02-12

Bleed entire image by @EgoMoose in #94

## [0.9.0] - 2025-02-06

Fixes & adjustments were made to the alpha bleeding algorithm. See the pull request for more details. Thanks to @EgoMoose in #93.

This is a breaking change. Your images may be re-uploaded if you update to 0.9.0.

## [0.8.4] - 2025-01-13

Added support for FLAC and WAV audio file formats, thanks to @Zidiam in #92.

## [0.8.3] - 2024-12-27

This is a re-release.

Added a command to migrate a Tarmac manifest, thanks to @Kampfkarren in #88.

Animation upload errors are now propagated, by @jackTabsCode in 0af498c

Asphalt can now be installed with Homebrew!:

```
brew install jacktabscode/tap/asphalt
```

## [0.8.2] - 2024-11-18

Asphalt now naively retries uploads that are rate limited.

Due to multiple image-related dependency upgrades, some decals may be re-uploaded as their output will have changed.

## [0.8.1] - 2024-08-05

Reworked code generation onto an AST-based style, with a `style` option (`flat` / `nested`).

Joined the asset directory with existing asset entries.

Added README instructions for ignoring generated files in linters/formatters and for building from source.

## [0.8.0] - 2024-06-30

Sync targets by @paradoxuum in #64

Please have a look at the README under `asphalt sync` for information on this new feature!

Removed `.lua` codegen support by @jackTabsCode in 95bfcdc. `.luau` files are now generated, and the `luau` setting will not do anything anymore.

## [0.7.2] - 2024-06-15

A new configuration option `exclude_assets` is available to skip processing files in the sync command. This option takes an array of glob patterns. Any files that match this glob will be completely skipped.

## [0.7.1] - 2024-06-06

The cookie for uploading animations will now be automatically detected from your Roblox Studio installation, if available.

## [0.7.0] - 2024-06-06

Animations are now supported! Please read the documentation to see how to sync them.

Improved verbosity of the output.

## [0.6.3] - 2024-06-04

Fixes an issue where errors caught during the sync process would not show their cause.

## [0.6.2] - 2024-06-03

Minor release that allows customizing logging with the `-v` flag.

## [0.6.1] - 2024-05-28

New command: `asphalt init`! Getting started with Asphalt has never been easier.

## [0.6.0] - 2024-05-26

Breaking: Asphalt's CLI has been split into subcommands. To sync, run `asphalt sync`.

Subcommand added to list uploaded assets: `asphalt list`.

## [0.5.1] - 2024-05-03

We now alpha bleed images by default (@jackTabsCode in #39)

We now support stripping file extensions (thanks to @paradoxuum in #38)

We now use Rustls for TLS, this should improve Linux compatibility (thanks to @paradoxuum in #37)

## [0.5.0] - 2024-04-25

We now support nested code generation, similar to Tarmac. (thanks to @karuzumi in #33)

Breaking: The configuration structure has changed. All options related to code generation now go in the `[codegen]` section. See the README for more details.

## [0.4.2] - 2024-04-14

We now support defining existing assets in `asphalt.toml`. This is helpful if you rely on Roblox marketplace assets that you don't want to reupload, but want to keep everything in the same place.

## [0.4.1] - 2024-04-04

Better error handling, again

## [0.4.0] - 2024-03-29

Breaking: Asphalt is now configured with a config file! See the README for details.

File names instead of paths are used for Roblox asset display names.

## [0.3.5] - 2024-03-29

Don't load font database for every SVG

## [0.3.4] - 2024-03-29

SVG support is here! It is recommended to use static SVGs with rasterized text as font rendering won't always work, especially if you don't have the font installed on your system.

## [0.3.3] - 2024-03-28

Much better error handling!

More performant (no more recursion, uses a ring buffer for looking through files)

Binaries available for more systems, thanks to cargo dist

## [0.3.2] - 2024-03-28

Better error handling with no more recursion.

## [0.3.1] - 2024-03-25

Support generating `.luau` file extensions with flag `--luau`

Fixed file paths on Windows systems

## [0.3.0] - 2024-03-11

Nested folder support. This does not create nested tables in the outputted files, instead making the keys the file path of the asset, which now include the extension-hence the breaking change.

## [0.2.1] - 2024-03-07

Errors when we receive no response from the get asset API

## [0.2.0] - 2024-03-07

Support for group owners. Use `--user-id <id>` or `--group-id <id>` to specify the asset creator.

Fixes a panic on files with no extension

## [0.1.1] - 2024-03-07

Hotfix to remove my hardcoded user ID. Whoops.

## [0.1.0] - 2024-03-07

Initial release 🎉!

Good luck
