# bndl

[![crates.io](https://img.shields.io/crates/v/bndl_cli.svg)](https://crates.io/crates/bndl_cli)
[![npm](https://img.shields.io/npm/v/bndl-cli)](https://www.npmjs.com/package/bndl-cli)
![GitHub Workflow Status (with event)](https://img.shields.io/github/actions/workflow/status/segersniels/bndl/bin.yml)

Introducing `bndl`, a barebones TypeScript transpiling and bundling tool for backend monorepos.

<p align="center">
<img src="https://github.com/segersniels/bndl/blob/master/resources/bndl.png?raw=true" width="250">

It aims to be a near drop-in replacement for those accustomed to `tsc`, utilizing the existing `tsconfig.json` in your project. `bndl` traverses the monorepo, builds the current workspace with [SWC](https://swc.rs/), identifies the dependencies used by the consumer, and copies them to the compiled directory's `node_modules`.

The result? An output directory containing everything necessary for your app to run. Simply copy the dist directory to a Docker image and execute it.

## Installing

### cargo

```bash
$ cargo install bndl_cli
```

### npm

```bash
$ npm install -g bndl-cli
```

### curl

```bash
$ sh -c "$(curl -fsSL https://raw.githubusercontent.com/segersniels/bndl/master/scripts/install.sh)"
```

### wget

```bash
$ sh -c "$(wget https://raw.githubusercontent.com/segersniels/bndl/master/scripts/install.sh -O -)"
```

## Usage

```bash
Usage: bndl [OPTIONS] [COMMAND]

Options:
  -p, --project <project>  The path to the project config file
      --outDir <outDir>    Specify an output folder for all emitted files.
      --clean              Clean the output folder if it exists before bundling
      --only-bundle        Skips compilation and only bundles the input files, assuming they are already compiled beforehand
      --no-bundle          Disable automatic bundling of internal monorepo dependencies
  -m, --minify             Minify the output bundle
  -w, --watch              Experimental: watch the input files for changes and recompile when they change
      --exec <exec>        Experimental: use in conjunction with --watch to execute a command after each successful compilation
  -h, --help               Print help
  -V, --version            Print version
```

## Known limitations

### Building dependencies

`bndl` only builds the current workspace, not its dependencies. You need to build internal dependencies before the consuming workspace. Using something like Turborepo, you can run `npx turbo run build --filter <your-workspace>` with a config like this:

```json
{
    "tasks": {
        "build": {
            "dependsOn": ["^build"],
            "outputs": ["dist/**"]
        }
    }
}
```

This config ensures Turborepo builds internal dependencies first before the consuming workspace, making them ready for bundling.

### Watch

`bndl` only watches the current workspace for changes, not internal dependencies. To handle this, use the `--exec` option to recompile and bundle internal dependencies.

```bash
bndl --watch --exec "npx turbo run build --filter <your-workspace>^... && npx bndl --only-bundle && npm run start"
```

This command will first build all internal dependencies (excluding the consuming application) and then tell `bndl` to bundle them. Note that this triggers only when a file changes in the consuming application. Save a file in the consuming application to trigger a rebuild.

Using Turborepo and its cache should make this relatively fast, minimizing delay between file changes and restarts.

## Contributing

Expect missing functionality and potential breakage. This tool was created for a specific use case and might differ from your needs. Feel free to open issues or PRs to add the functionality you need.

Submit issues with panic logs or debug information (`RUST_LOG=debug bndl ...`) for quicker resolution.
