use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::{collections::HashMap, fs};
use swc::{
    config::{JscConfig, Paths},
    BoolConfig,
};
use swc_ecma_ast;
use swc_ecma_parser::{Syntax, TsConfig};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackageJson {
    pub name: String,
    pub workspaces: Option<Vec<String>>,
    pub dependencies: Option<HashMap<String, String>>,
}

pub fn fetch_package_json(path: &Path) -> PackageJson {
    let package_json_str = fs::read_to_string(path).expect("Unable to read package.json");

    match serde_json::from_str(&package_json_str) {
        Ok(package_json) => package_json,
        Err(e) => {
            eprintln!("Error parsing package.json: {}", e);
            std::process::exit(1);
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CompilerOptions {
    pub module: Option<String>,
    pub declaration: Option<bool>,
    pub experimentalDecorators: Option<bool>,
    pub target: Option<String>,
    pub sourceMap: Option<bool>,
    pub baseUrl: Option<String>,
    pub paths: Option<Paths>,
    pub inlineSources: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TsConfigJson {
    pub extends: Option<String>,
    pub compilerOptions: CompilerOptions,
    pub exclude: Option<Vec<String>>,
}

fn merge_compiler_options(base: &CompilerOptions, child: &CompilerOptions) -> CompilerOptions {
    CompilerOptions {
        module: child.module.clone().or_else(|| base.module.clone()),
        declaration: child.declaration.or(base.declaration),
        experimentalDecorators: child.experimentalDecorators.or(base.experimentalDecorators),
        target: child.target.clone().or_else(|| base.target.clone()),
        sourceMap: child.sourceMap.or(base.sourceMap),
        baseUrl: child.baseUrl.clone().or_else(|| base.baseUrl.clone()),
        paths: child.paths.clone().or_else(|| base.paths.clone()),
        inlineSources: child.inlineSources.or(base.inlineSources),
    }
}

fn load_and_merge_tsconfig(config_path: &Path) -> serde_json::Result<TsConfigJson> {
    let config_str =
        fs::read_to_string(config_path).unwrap_or(String::from("{\"compilerOptions\": {}}"));

    let mut tsconfig: TsConfigJson = serde_json::from_str(&config_str)?;

    if let Some(extends) = &tsconfig.extends {
        let base_config_path = if extends.starts_with('.') {
            // Resolve the path of the base configuration relative to the child configuration
            config_path.parent().unwrap().join(extends)
        } else {
            // Assume `extends` is a path from the root or a module (not implemented here)
            PathBuf::from(extends)
        };

        let base_tsconfig = load_and_merge_tsconfig(&base_config_path)?;
        tsconfig.compilerOptions =
            merge_compiler_options(&base_tsconfig.compilerOptions, &tsconfig.compilerOptions);
    }

    Ok(tsconfig)
}

pub fn fetch_tsconfig(config_path: &str) -> serde_json::Result<TsConfigJson> {
    let tsconfig = load_and_merge_tsconfig(Path::new(config_path));

    tsconfig.map_err(|e| {
        eprintln!("Error parsing tsconfig.json: {}", e);
        e
    })
}

fn convert_target_to_es_version(target: &Option<String>) -> Option<swc_ecma_ast::EsVersion> {
    Some(match target {
        Some(target) => match target.to_lowercase().as_str() {
            "es3" => swc_ecma_ast::EsVersion::Es3,
            "es5" => swc_ecma_ast::EsVersion::Es5,
            "es2015" => swc_ecma_ast::EsVersion::Es2015,
            "es2016" => swc_ecma_ast::EsVersion::Es2016,
            "es2017" => swc_ecma_ast::EsVersion::Es2017,
            "es2018" => swc_ecma_ast::EsVersion::Es2018,
            "es2019" => swc_ecma_ast::EsVersion::Es2019,
            "es2020" => swc_ecma_ast::EsVersion::Es2020,
            "es2021" => swc_ecma_ast::EsVersion::Es2021,
            "esnext" => swc_ecma_ast::EsVersion::EsNext,
            _ => swc_ecma_ast::EsVersion::latest(),
        },
        None => swc_ecma_ast::EsVersion::latest(),
    })
}

fn convert_module(module: &Option<String>) -> Option<swc::config::ModuleConfig> {
    Some(match module {
        Some(module) => match module.to_lowercase().as_str() {
            "amd" => swc::config::ModuleConfig::Amd(Default::default()),
            "commonjs" => swc::config::ModuleConfig::CommonJs(Default::default()),
            "cjs" => swc::config::ModuleConfig::CommonJs(Default::default()),
            "es6" => swc::config::ModuleConfig::Es6(Default::default()),
            "es2015" => swc::config::ModuleConfig::Es6(Default::default()),
            "nodenext" => swc::config::ModuleConfig::NodeNext(Default::default()),
            "umd" => swc::config::ModuleConfig::Umd(Default::default()),
            "system" => swc::config::ModuleConfig::SystemJs(Default::default()),
            _ => swc::config::ModuleConfig::CommonJs(Default::default()),
        },
        None => swc::config::ModuleConfig::CommonJs(Default::default()),
    })
}

fn determine_base_url(base_url: Option<String>) -> PathBuf {
    Path::new(base_url.unwrap_or_default().trim_start_matches("./")).to_path_buf()
}

fn determine_paths(base_url: &PathBuf, paths: Option<Paths>) -> Paths {
    if base_url.to_string_lossy().len() == 0 {
        return Default::default();
    }

    paths.unwrap_or_default()
}

pub fn convert_ts_config_to_swc_config(
    ts_config: &TsConfigJson,
    fallback_legacy_dts: bool,
    minify_output: bool,
) -> swc::config::Config {
    let base_url = determine_base_url(ts_config.clone().compilerOptions.baseUrl);
    let paths = determine_paths(&base_url, ts_config.clone().compilerOptions.paths);
    let inline_sources = ts_config.compilerOptions.inlineSources.unwrap_or(false);

    swc::config::Config {
        minify: BoolConfig::from(minify_output),
        module: convert_module(&ts_config.compilerOptions.module),
        source_maps: if inline_sources {
            Some(swc::config::SourceMapsConfig::Str(String::from("inline")))
        } else {
            Some(swc::config::SourceMapsConfig::Bool(
                ts_config.compilerOptions.sourceMap.unwrap_or(false),
            ))
        },
        jsc: JscConfig {
            base_url,
            paths,
            transform: Some(swc::config::TransformConfig {
                legacy_decorator: BoolConfig::new(Some(true)),
                decorator_metadata: BoolConfig::new(Some(
                    ts_config
                        .compilerOptions
                        .experimentalDecorators
                        .unwrap_or_default(),
                )),
                ..Default::default()
            })
            .into(),
            preserve_all_comments: BoolConfig::new(Some(false)),
            keep_class_names: BoolConfig::new(Some(true)),
            target: convert_target_to_es_version(&ts_config.compilerOptions.target),
            syntax: Some(Syntax::Typescript(TsConfig {
                dts: !fallback_legacy_dts
                    && ts_config.compilerOptions.declaration.unwrap_or_default(),
                decorators: ts_config
                    .compilerOptions
                    .experimentalDecorators
                    .unwrap_or_default(),
                ..Default::default()
            })),
            ..Default::default()
        },
        ..Default::default()
    }
}
