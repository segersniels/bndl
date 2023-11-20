use std::path::Path;

/// https://github.com/swc-project/swc/blob/5fa2ed4bfdb49f77d4cb77004e1f61c8e2c36b64/crates/swc_cli_impl/src/commands/compile.rs#L540
pub fn extend_source_map(
    source_map: String,
    source_file_name: &Option<String>,
    source_root: &Option<String>,
) -> Vec<u8> {
    let mut source_map = swc::sourcemap::SourceMap::from_reader(source_map.as_bytes())
        .expect("failed to encode source map");

    if source_map.get_token_count() != 0 {
        if let Some(ref source_file_name) = source_file_name {
            source_map.set_source(0u32, source_file_name);
        }
    }

    if let Some(root) = source_root {
        source_map.set_source_root(Some(root));
    }

    let mut buf = vec![];
    source_map
        .to_writer(&mut buf)
        .expect("failed to decode source map");

    buf
}

pub fn determine_source_file_name(input_path: &Path, output_path: &Path) -> Option<String> {
    pathdiff::diff_paths(
        input_path.canonicalize().unwrap(),
        output_path.canonicalize().unwrap(),
    )
    .map(|diff| diff.to_string_lossy().to_string())
}
