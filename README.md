# Asphalt

Asphalt is a command line tool used to upload assets to Roblox and easily reference them in code.
It's a modern alternative to [Tarmac](https://github.com/Roblox/Tarmac).

## Features

-   Syncs your images, sounds, [videos](#videos), [animations](#animations), and [3D models](#models) to Roblox
-   Generates Luau or Typescript code so you can use them in your game
-   Can target Roblox users or groups
-   Processes SVGs into PNGs and alpha bleeds images for crisp edges
-   Allows defining existing uploaded assets, so all of your stuff can be referenced in one place

## Features Coming Soon
-  Capablility to pack your images into spritesheets for lower client memory usage

## Installation

### [Mise](https://mise.jdx.dev)

```bash
mise use ubi:jacktabscode/asphalt
```

### [Cargo](https://crates.io/crates/asphalt) (build from source)

```bash
cargo install asphalt
```

<details>
<summary>View other installation options</summary>

### [Pesde](https://github.com/pesde-pkg/pesde)

```bash
pesde add --dev pesde/asphalt --target lune
```

### [Rokit](https://github.com/rojo-rbx/rokit)

```bash
rokit add jacktabscode/asphalt
```

### [Homebrew](https://brew.sh) (macOS/Linux)

```bash
brew tap jacktabscode/tap
brew install asphalt
```

[Asphalt cannot be installed with Foreman.](https://github.com/Roblox/foreman/issues/97)

</details>

## Commands

### `asphalt sync`

Syncs all of your assets defined in your inputs.

There are three targets you can use to sync assets:

-   `cloud`: Uploads your assets to Roblox. This will generate a `asphalt.lock.toml` file which you should commit to source control. This is the default target.

-   `studio`: Syncs assets locally to Roblox Studio. This is useful for testing assets in Studio before uploading them to Roblox.

-   `debug`: Syncs assets to an `.asphalt-debug` folder in the current directory. You can use this option see how Asphalt will process your files.

```bash
asphalt sync # Equivalent to --target cloud
asphalt sync --target studio
asphalt sync --target debug
```

You can also perform a dry run to verify which assets will be synced. This exits with a non-zero status code if any asset hashes have changed. You can use this for CI checks to ensure that your assets are up-to-date.

```bash
asphalt sync --dry-run
```

### `asphalt migrate-lockfile`

Will migrate your lockfile to the newest format, if there is one. See `asphalt migrate-lockfile --help` for more information.

### `asphalt upload`

Uploads a single asset to Roblox. See `asphalt upload --help` for more information.

## Configuration

Asphalt is configured with a project file called `asphalt.toml`. It is required for the program to run.

<details>
<summary>See an example</summary>

```toml
[creator]
type = "user"
id = 9670971

[codegen]
typescript = true
style = "flat"

[inputs.assets]
path = "assets/**/*"
output_path = "src/shared"

[inputs.assets.web]
"some_sound_on_roblox.ogg" = { id = 123456789 }
"some_image_on_roblox.png" = { id = 987654321 }
```

</details>

### Format

-   `creator`: Creator
	-   The Roblox creator to upload the assets under.
-   `codegen`: Codegen (optional)
	-   Code generation options.
-	`inputs`: map<string, Input>
	-   A map of input names to input configurations.

#### Creator

-	`type`: "user" or "group"
-	`id`: number

#### Codegen

-   `typescript`: boolean (optional)
    -   Generate a Typescript definition file.
-   `style`: "flat" | "nested" (optional)
    -   The code-generation style to use. Defaults to `flat`, which makes accessing assets feel like writing file paths. You may consider using `nested` if you are not a TypeScript user, however, as Luau does not support template literal types.
-   `strip_extensions`: boolean (optional)
    -   Whether to strip the file extension. Defaults to `false` for the same reason described above.
-   `content`: boolean (optional)
    -   Whether to output `Content` instead of `string`s. Defaults to `false`.

#### Input
-	`path`: glob
	-	A glob pattern to match files to upload.
-	`output_path`: string
	-	The directory path to output the generated code.
-	`web`: map<string, WebAsset>
	-	A map of paths relative to the input path to existing assets on Roblox.
- 	`bleed`: boolean (optional)
	- 	Whether to alpha bleed images. Defaults to `true`. Keep in mind that changing this setting won't invalidate your lockfile or reupload your images.
- 	`warn_each_duplicate`: boolean (optional)
	- 	Whether to emit a warning each time a duplicate file is found. Defaults to `true`.

#### WebAsset

-   `id`: number

## Code Generation

The formatting of code generation (such as spaces, tabs, width, and semicolons) is not guaranteed by Asphalt and may change between releases without being noted as a breaking change.

Therefore, it is recommended to add Asphalt's generated files to your linter/formatter's "ignore" list. Here are instructions for the most commonly used tools:

- [Stylua](https://github.com/JohnnyMorganz/StyLua?tab=readme-ov-file#glob-filtering)
- [Biome](https://biomejs.dev/guides/configure-biome/#ignore-files)
- [ESLint](https://eslint.org/docs/latest/use/configure/ignore)

## Authentication

- A properly scoped API key is always required.
- A cookie is required for animation uploads.

### API Key

You can specify this using the `--api-key` argument, or the `ASPHALT_API_KEY` environment variable.

You can get one from the [Creator Dashboard](https://create.roblox.com/dashboard/credentials).

The following permissions are required:
- `asset:read`
- `asset:write`

Make sure that you select an appropriate IP and that your API key is under the Creator (user, or group) that you've defined in `asphalt.toml`.

### Cookie

Your cookie will be pulled from your `ROBLOSECURITY` environment variable. If not present, it be automatically detected from the current Roblox Studio installation.

You will probably want to [disable Session Protection](https://create.roblox.com/settings/advanced) if you are using Asphalt in an environment where your IP address changes frequently, but we don't recommend this on your main Roblox account, as it makes your account less secure.

## Animations

> [!WARNING]
> This feature uses a private Studio API, so this feature may break without warning.

Asphalt expects a single [KeyframeSequence](https://create.roblox.com/docs/reference/engine/classes/KeyframeSequence) to be saved as either a `.rbxm` or `.rbxmx` file.

## Videos

When uploading videos, you must provide the `--expected-price` argument, which is the price you expect to be charged for the video. See the [Roblox documentation on Videos](https://create.roblox.com/docs/en-us/ui/video-frames#upload-videos) for more details.

## Models

Asphalt supports uploading `.fbx` files purely out of ease, but Roblox does not offer control over the import settings through the web API. As such, this is not a feature that most developers use.

We instead recommend uploading your models with the 3D Importer in Studio, which provides a 3D preview, error checking, and customizable settings. You can then use a tool like [Rojo](https://github.com/rojo-rbx/rojo) to build them into your project.

## Attributions

Thank you to [Tarmac](https://github.com/Roblox/tarmac) for the alpha bleeding implementation, which was used in this project.
