# bndl_convert

Internal conversion crate to transform a `tsconfig.json` into an SWC compatible `JSConfig`.

## Usage

### CLI

```bash
$ cargo install bndl_convert # or npm install -g bndl-convert
$ bndl_convert --minify ./tsconfig.json
```

### Crate

```bash
$ cargo add bndl_convert
```

```rust
use bndl_convert::{convert, fetch_tsconfig}
use swc::config::Options;

fn main() {
     match fetch_tsconfig("./tsconfig.json") {
        Ok(ts_config) => {
            let config = convert(&ts_config, None, None);
            let options: Options = Options {
                config,
                ..Default::default()
            };
        }
        Err(e) => {
            eprintln!("{}", e)
        }
    }
}
```
