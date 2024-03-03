# bndl

[![crates.io](https://img.shields.io/crates/v/bndl_cli.svg)](https://crates.io/crates/bndl_cli)
[![npm](https://img.shields.io/npm/v/bndl-cli)](https://www.npmjs.com/package/bndl-cli)
![GitHub Workflow Status (with event)](https://img.shields.io/github/actions/workflow/status/segersniels/bndl/bin.yml)

Introducing `bndl`, a basic TypeScript transpiling and bundling tool for backend monorepos. It uses [SWC](https://swc.rs/) under the hood so it benefits from the speed improvements that it brings over `tsc`.

<p align="center">
<img src="https://github.com/segersniels/bndl/blob/master/resources/bndl.png?raw=true" width="250">

It aims to be a near drop in replacement for people already accustomed to `tsc` and uses the `tsconfig.json` already present in your project. `bndl` goes through the monorepo, builds the current app (or package) with `swc`, identifies which dependencies are used by the consumer and copies them over to the compiled directory `node_modules` of said consumer.

The result? A `dist` that contains everything for your app to run. Simply copy the `dist` over to a Docker image and run it. Read more about it [here](https://niels.foo/post/typescript-monorepo-bundling-for-dummies).

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
  -h, --help               Print help
  -V, --version            Print version
```

## Known limitations

### Watch

As `bndl` watches files in the current workspace for changes it won't detect file changes in internal dependencies. To save on resouces and downtime between file changes and restarts `bndl` also skips the bundling of internal dependencies. To work around this you could make clever use of `nodemon` and let it watch the output directory and recompile the changed files from the internal dependencies.

```bash
npx nodemon --watch <out-dir> --delay 100ms -e js,json --exec "npx turbo run build --filter <your-workspace>^... && npx bndl --only-bundle && npm run start"
```

The above will first build all internal dependendies (excl. the consuming application) and tell `bndl` to only bundle those internal dependencies, skipping any form of compilation. Keep in mind that this will only run when a file changes in the consuming application, not when you save the dependency itself.

## Contributing

Expect a lot of missing functionality and potential things breaking. This was made with a specific use case in mind and there might be cases where functionality drifts from what you might need. Feel free to make issues or PRs adding your requested functionality.

Please provide the provided panic log or debug logging with `RUST_LOG=debug bndl ...` so your issue can get resolved quicker.
