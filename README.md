# Asphalt

Asphalt is a command line tool used to upload assets to Roblox and easily reference them in code.
It's a modern alternative to [Tarmac](https://github.com/Roblox/Tarmac).

## Features

-   Syncs your images, sounds, videos, animations, and models to Roblox! See the [supported asset types](#supported-asset-types)
-   Generates Luau or TypeScript code so you can use them in your game
-   Can target Roblox users or groups
-   Processes SVGs into PNGs and alpha bleeds images for crisp edges
-   Allows defining assets you already uploaded

## Features Coming Soon
-  Capability to pack your images into spritesheets for lower client memory usage

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
#:schema https://raw.githubusercontent.com/jackTabsCode/asphalt/refs/heads/main/schema.json

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

-   `creator`: [Creator](#creator)
	-   The Roblox creator to upload the assets under.
-   `codegen`: [Codegen](#codegen) (optional)
	-   Code generation options.
-	`inputs`: map<string, [Input](#input)>
	-   A map of input names to input configurations.

#### Creator

-	`type`: "user" or "group"
-	`id`: number

#### Codegen

-   `typescript`: boolean (optional)
    -   Generate a TypeScript definition file.
-   `style`: "flat" | "nested" (optional)
    -   The code generation style to use. Defaults to `flat`, which lets you index assets as if they were paths. You may consider using `nested` if you are not a TypeScript user as Luau does not support template literal types.
-   `strip_extensions`: boolean (optional)
    -   Whether to strip the file extension. Defaults to `false` for the same reason described above.
-   `content`: boolean (optional)
    -   Whether to output `Content` instead of `string`s. Defaults to `false`.

#### Input
-	`path`: glob
	-	A glob pattern to match files to upload.
-	`output_path`: string
	-	The directory path to output the generated code.
-	`web`: map<string, [WebAsset](#webasset)>
	-	A map of paths relative to the input path to existing assets on Roblox.
- 	`bleed`: boolean (optional)
	- 	Whether to alpha bleed images. Defaults to `true`. Keep in mind that changing this setting won't invalidate your lockfile or reupload your images.

#### WebAsset

-   `id`: number

## Authentication

You can specify your API key this using the `--api-key` argument, or the `ASPHALT_API_KEY` environment variable.

You can get one from the [Creator Dashboard](https://create.roblox.com/dashboard/credentials).

The following permissions are required:
- `asset:read`
- `asset:write`

Make sure that you select an appropriate IP and that your API key is under the Creator (user, or group) that you've defined in `asphalt.toml`.

## Supported Asset Types

- Images (.png, .jpg, .bmp, .tga, .svg)
	- SVGs are supported by Asphalt by converting them to PNGs.
- Audio (.mp3, .ogg, .wav, .flac)
- Videos (.mp4, .mov)
	- When uploading videos, you must provide the `--expected-price` argument, which is the price you expect to be charged for the video. See the [Roblox documentation on Videos](https://create.roblox.com/docs/en-us/ui/video-frames#upload-videos) for more details.
- Animations (.rbxm, .rbxmx)
	- Asphalt detects animations by looking to see if the saved class is a KeyframeSequence or a CurveAnimation. If it isn't, Asphalt will assume it is a Model.
- Roblox Models (.rbxm, .rbxmx)
- 3D Models (.fbx, .gltf, .glb)
	- Roblox does not offer control over the import settings through the web API. As such, this is not a route most should take. You should instead use the [3D Importer](https://create.roblox.com/docs/art/modeling/3d-importer) to upload these assets, then either sync them with Rojo or save them as Roblox model files.

## Code Generation

The formatting of code generation (such as spaces, tabs, width, and semicolons) is not guaranteed by Asphalt and may change between releases without being noted as a breaking change.

Therefore, it is recommended to add Asphalt's generated files to your linter/formatter's "ignore" list. Here are instructions for the most commonly used tools:

- [Stylua](https://github.com/JohnnyMorganz/StyLua?tab=readme-ov-file#glob-filtering)
- [Biome](https://biomejs.dev/guides/configure-biome/#ignore-files)
- [ESLint](https://eslint.org/docs/latest/use/configure/ignore)

## Attributions

Thank you to [Tarmac](https://github.com/Roblox/tarmac) for the alpha bleeding implementation, which was used in this project.
