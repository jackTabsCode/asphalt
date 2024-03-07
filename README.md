Asphalt is a simple CLI tool used to upload assets to Roblox and easily reference them in code.

> [!WARNING]
> This is literally my first Rust program. I made it because [Tarmac](https://github.com/rojo-rbx/tarmac) is broken right now and I wanted a quick alternative. It's not perfect, doesn't have good error handling, and spams retries when it fails.

## Features

-   Upload images and audio
-   Generate Luau code to reference the uploaded assets
-   Generate Typescript definitions for roblox-ts users
-   Uses the Open Cloud API

## Installation

You can install Asphalt using [Aftman](https://github.com/LPGhatguy/aftman):

```sh
aftman add asphalt
```

## Usage

Just run `asphalt --help` to see the available options.
