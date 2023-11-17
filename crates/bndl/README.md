# bndl

[![crates.io](https://img.shields.io/crates/v/bndl_cli.svg)](https://crates.io/crates/bndl_cli)
[![npm](https://img.shields.io/npm/v/bndl-cli)](https://www.npmjs.com/package/bndl-cli)
![GitHub Workflow Status (with event)](https://img.shields.io/github/actions/workflow/status/segersniels/bndl/bin.yml)

Introducing `bndl`, a basic TypeScript transpiling and bundling tool for (primarily backend) monorepos. It uses [SWC](https://swc.rs/) under the hood so it benefits from the speed improvements that it brings over `tsc`.

<p align="center">
<img src="https://github.com/segersniels/bndl/blob/master/resources/bndl.png?raw=true" width="250">

It aims to be a near drop in replacement for people already accustomed to `tsc` and uses the `tsconfig.json` already present in your project. `bndl` goes through the monorepo, builds the current app (or package) with `swc`, identifies which dependencies are used by the consumer and copies them over to the compiled directory `node_modules` of said consumer.

The result? A `dist` that contains everything for your app to run. Simply copy the `dist` over to a Docker image and run it. Read more about it [here](https://niels.foo/post/typescript-monorepo-bundling-for-dummies).

## Usage

```bash
$ cargo +nightly install bndl_cli # or npm install -g bndl-cli
$ bndl --clean --outDir dist --project tsconfig.json --minify
```

If you don't want to deal with the overhead and want to keep your transition from `tsc` to a bundled setup as simple as possible, `bndl` might be the tool for you.
But I would still see `bndl` as a last resort if you can't get the more popular (and probably better maintained) tools like `tsup` or `webpack` running in your monorepo.

## Contributing

Expect a lot of missing functionality and potential things breaking. This was made with a specific use case in mind and there might be cases where functionality drifts from what you might need. Feel free to make issues or PRs adding your requested functionality.

Please provide the provided panic log or debug logging with `RUST_LOG=debug bndl ...` so your issue can get resolved quicker.
