# Asphalt

Asphalt is a command line tool used to upload assets to Roblox and easily reference them in code.
It's a modern alternative to [Tarmac](https://github.com/Roblox/Tarmac).

## Features

-   Syncs your images, sounds, models, and animations to Roblox
-   Generates Luau or Typescript code so you can use them in your game
-   Can target Roblox users or groups
-   Processes SVGs into PNGs and alpha bleeds images for crisp edges
-   Allows defining existing uploaded assets, so all of your stuff can be referenced in one place

## Features Coming Soon
-  Capablility to pack your images into spritesheets for lower client memory usage
-  Extended support for different audio formats

## Installation

### [Homebrew](https://brew.sh) (macOS/Linux)

```sh
brew tap jacktabscode/tap
brew install asphalt
```

### [Aftman](https://github.com/lpghatguy/aftman) or [Rokit](https://github.com/rojo-rbx/rokit)

```sh
aftman add jacktabscode/asphalt
```

### [Cargo](https://crates.io/crates/asphalt) (build from source)

```sh
cargo install asphalt
```

[Asphalt cannot be installed with Foreman.](https://github.com/Roblox/foreman/issues/97)

## Commands

### `asphalt init`

Guides you through setting up a new Asphalt project in the current directory.

### `asphalt sync`

Syncs all assets in `asset_dir`.

There are three targets you can use to sync assets:

-   `cloud`: Uploads your assets to Roblox. This will generate a `asphalt.lock.toml` file which you should commit to source control. This is the default target.

-   `studio`: Syncs assets locally to Roblox Studio. This is useful for testing assets in Studio before uploading them to Roblox.

-   `debug`: Syncs assets to an `.asphalt-debug` folder in the current directory.

```bash
asphalt sync # Equivalent to --target cloud
asphalt sync --target studio
asphalt sync --target debug
```

You can also perform a dry run to verify which assets will be synced. This displays the assets that would be synced without syncing them.

```bash
asphalt sync --dry-run
```

### `asphalt list`

Lists asset paths from the lockfile and their corresponding Roblox asset IDs.

### `asphalt migrate-tarmac-manifest`

Will migrate over an existing `tarmac-manifest.toml` to `asphalt.lock.toml`.

## Configuration

Asphalt is configured with a project file called `asphalt.toml`. It is required for the program to run.

<details>
<summary>Example</summary>

```toml
asset_dir = "assets/"
exclude_assets = ["**/*.txt", "**/*.DS_Store"]

write_dir = "src/shared/"

[codegen]
typescript = true
style = "flat"
output_name = "assets"

[creator]
type = "user"
id = 9670971

[existing]
"some_sound_on_roblox.ogg" = { id = 123456789 }
"some_image_on_roblox.png" = { id = 987654321 }
```

</details>

### Format

-   `asset_dir`: path
    -   The directory of assets to upload to Roblox.
-	`exclude_assets`: array<string> (optional)
	-	An array of glob patterns to exclude when processing the assets directory.
-   `write_dir`: path
    -   The directory to output the generated code to. This should probably be somewhere in your game's source folder.
-   `creator`: Creator
    -   The Roblox creator to upload the assets under.
-   `codegen`: Codegen
    -   Code generation options.
-   `existing`: map<string, ExistingAsset> (optional)

#### Creator

-   `type`: "user" or "group"
-   `id`: number

#### Codegen

-   `typescript`: boolean (optional)
    -   Generate a Typescript definition file.
-   `style`: "flat" | "nested" (optional)
    -   The code-generation style to use. Defaults to `flat`, which makes accessing assets feel like writing file paths. You may consider using `nested` if you are not a TypeScript user, however, as Luau does not support template literal types.
-   `output_name`: string (optional)
    -   The name for the generated files. Defaults to `assets`.
-   `strip_extension`: boolean (optional)
    -   Whether to strip the file extension. Defaults to `false`. We recommend `true` if using the `nested` codegen style.

#### ExistingAsset

-   `id`: number

## Code Generation
The formatting of code generation (such as spaces, tabs, width, and semicolons) is not guaranteed by Asphalt and may change between releases without being noted as a breaking change.

Therefore, it is recommended to add Asphalt's generated files to your linter/formatter's "ignore" list. Here are instructions for the most commonly used tools:

- [Stylua](https://github.com/JohnnyMorganz/StyLua?tab=readme-ov-file#glob-filtering)
- [Biome](https://biomejs.dev/guides/configure-biome/#ignore-files)
- [ESLint](https://eslint.org/docs/latest/use/configure/ignore)

## API Key

You will need an API key to sync with Asphalt. You can specify this using the `--api-key` argument, or the `ASPHALT_API_KEY` environment variable.

You can get one from the [Creator Dashboard](https://create.roblox.com/dashboard/credentials). Make sure you select the correct group and Asset-related permissions.

## Cookie
You will need a cookie to upload animations to Roblox. This is because the Open Cloud API does not support them. It will automatically detected from the current Roblox Studio installation. Otherwise, you can specify this using the `--cookie` argument, or the `ASPHALT_COOKIE` environment variable.

You will probably want to [disable Session Protection](https://create.roblox.com/settings/advanced) if you are using Asphalt in an environment where your IP address changes frequently, but we don't recommend this on your main Roblox account, as it makes your account less secure.

## Animations

> [!WARNING]
> This feature is experimental, and Roblox may break the API we use or change its behavior without warning.

To upload animations, make sure you specify a cookie as noted above.

Asphalt expects a single [KeyframeSequence](https://create.roblox.com/docs/reference/engine/classes/KeyframeSequence) to be saved as either a `.rbxm` or `.rbxmx` file.

## Attributions

Thank you to [Tarmac](https://github.com/Roblox/tarmac) for the alpha bleeding and nested codegen implementations, which were used in this project.
