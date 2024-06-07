# Asphalt

Asphalt is a simple CLI tool used to upload assets to Roblox and easily reference them in code.

## Features

-   Upload images, SVGs, sounds, models, and animations to Roblox
-   Generate Luau code to reference the uploaded assets
-   Generate Typescript definitions for roblox-ts users
-   Uses the Open Cloud API
-   Supports uploading to groups
-   Define existing uploaded assets, so all of your stuff can be referenced in one place
-   Alpha bleeds images for crisp edges when scaled

## Installation

### Aftman

```sh
aftman add jacktabscode/asphalt
```

### Cargo

```sh
cargo install asphalt
```

## Commands

### `asphalt init`

Guides you through setting up a new Asphalt project in the current directory.

### `asphalt sync`

Uploads all assets in the `asset_dir` to Roblox. It will also generate a `asphalt.lock.toml` file which you should commit to source control.

### `asphalt list`

Lists asset paths from the lockfile and their corresponding Roblox asset IDs.

## Configuration

Asphalt is configured with a project file called `asphalt.toml`. It is required for the program to run.

<details>
<summary>Example</summary>

```toml
asset_dir = "assets/"
write_dir = "src/shared/"

[codegen]
typescript = true
luau = true
style = "flat"
output_name = "assets"

[creator]
type = "user"
id = 9670971

[existing]
"test/some_sound_on_roblox.ogg" = { id = 123456789 }
"test/some_image_on_roblox.png" = { id = 987654321 }
```

</details>

### Format

-   `asset_dir`: path
    -   The directory of assets to upload to Roblox.
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
-   `luau`: boolean (optional)
    -   Use the `luau` file extension.
-   `style`: "flat" | "nested" (optional)
    -   The code-generation style to use. Defaults to `flat`. If you would like to have an experience similar to [Tarmac](https://github.com/rojo-rbx/tarmac), use `nested`.
-   `output_name`: string (optional)
    -   The name for the generated files. Defaults to `assets`.
-   `strip_extension`: boolean (optional)
    -   Whether to strip the file extension. Defaults to `false`. If you would like to have an experience similar to [Tarmac](https://github.com/rojo-rbx/tarmac), use `true`.

#### ExistingAsset

-   `id`: number

## API Key

You will need an API key to sync with Asphalt. You can specify this using the `--api-key` argument, or the `ASPHALT_API_KEY` environment variable.

You can get one from the [Creator Dashboard](https://create.roblox.com/dashboard/credentials). Make sure you select the correct group and Asset-related permissions.

## Cookie
You will need a cookie to upload animations to Roblox. This is because the Open Cloud API does not support them. It will automatically detected from the current Roblox Studio installation. Otherwise, you can specify this using the `--cookie` argument, or the `ASPHALT_COOKIE` environment variable.

You will probably want to [disable Session Protection](https://create.roblox.com/settings/advanced) if you are using Asphalt in an environment where your IP address changes frequently.

## Animations

> [!WARNING]
> This feature is experimental, and Roblox may break the API we use or change its behavior without warning.

To upload animations, make sure you specify a cookie as noted above.

Asphalt expects a single [KeyframeSequence](https://create.roblox.com/docs/reference/engine/classes/KeyframeSequence) to be saved as either a `.rbxm` or `.rbxmx` file.

## Attributions

Thank you to [Tarmac](https://github.com/Roblox/tarmac) for the alpha bleeding and nested codegen implementations, which were used in this project.
