# bndl

Introducing `bndl` 📦, a basic TypeScript transpiling and bundling tool for (primarily backend) monorepos. It uses [SWC](https://swc.rs/) under the hood so it benefits from the speed improvements that it brings over `tsc`.

While other bundling tools focus on bundling their dependencies inside the files themselves, `bndl` approaches this in a different, maybe slightly odd way.

Instead of trying to bundle everything together it simply tries to copy the built internal packages to the output directory `node_modules`. This way `node` can resolve these modules from the nearest available `node_modules` without having to rely on symbolic links and everything being in place. This can remove a lot of overhead and debugging because of the more simplistic approach to the problem that others try to solve in a different way.

This of course ties in with the monorepo tool that you use (like [Turborepo](https://turbo.build/repo)) and assumes that all internal packages are built before the consuming application.

In a [Turborepo](https://turbo.build/repo) environment this could look like the following:

1. Have turbo configured to build internal dependencies first `"dependsOn": ["^build"]`
2. Run `turbo run build`
3. Internal shared packages build first as configured
4. Consuming application is built through `bndl` after, and does:
    1. Build through `swc`
    2. Identifies internal used `dependencies` specified in the consuming app's `package.json`
    3. Copies over the built `dist` directories to the consuming app's `dist/node_modules/`.

You end up with a compiled _JavaScript_ project that has access to all of its internal monorepo packages without having to worry about them being in the right place in your eg. Docker image.

## Should you use it?

I would see `bndl` as a last resort if you can't get the more popular (and probably better maintained) tools like `tsup` or `webpack` running in your monorepo.

But, if you don't want to deal with the overhead and want to keep your transition from `tsc` to a bundled setup as simple as possible, `bndl` might be the tool for you.

## Contributing

Expect a lot of missing functionality and potential things breaking. This was made with a specific use case in mind and there might be cases where functionality drifts from what you might need. Feel free to make issues or PRs adding your requested functionality.

Please provide the provided panic log or debug logging with `RUST_LOG=debug bndl ...` so your issue can get resolved quicker.
