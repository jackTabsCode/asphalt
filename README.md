# Asphalt

Asphalt is a simple CLI tool used to upload assets to Roblox and easily reference them in code.

## Features

-   Upload images, audio, and even SVGs!
-   Generate Luau code to reference the uploaded assets
-   Generate Typescript definitions for roblox-ts users
-   Uses the Open Cloud API
-   Supports uploading to groups

## Installation

### Aftman

```sh
aftman add jacktabscode/asphalt
```

### Cargo

```sh
cargo install asphalt
```

## Configuration

Asphalt is configured with a project file called `asphalt.toml`. It is required for the program to run.

### Format

- `asset_dir`: path
	- The directory of assets to upload to Roblox.
- `write_dir`: path
	- The directory to output the generated code to. This should probably be somewhere in your game's source folder.
- `creator`: Creator
	- The Roblox creator to upload the assets under.
- `typescript`: boolean (optional)
	- Generate a Typescript definition file.
- `luau`: boolean (optional)
	- Use the `luau` file extension.
- `output_name`: string (optional)
	- The name for the generated files. Defaults to `assets`.

#### Creator

- `type`: "user" or "group"
- `id`: number

## Usage

Just run `asphalt` and make sure you have a config file as specified above. When complete, it will generate a `asphalt.lock.toml` file which you should have committed to source control.

## API Key

You will need an API key to run Asphalt. You can specify this using the `--api-key` argument, or the `ASPHALT_API_KEY` environment variable.
