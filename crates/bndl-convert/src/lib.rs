use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use swc::config::{Config, ModuleConfig, Options, SourceMapsConfig};
use swc::{
    config::{JscConfig, Paths},
    BoolConfig,
};

use swc_ecma_parser::{Syntax, TsConfig};

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
    pub declarationDir: Option<String>,
    pub outDir: Option<String>,
    pub removeComments: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TsConfigJson {
    pub extends: Option<String>,
    pub compilerOptions: CompilerOptions,
    pub exclude: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableConfig {
    #[serde(default)]
    pub jsc: JscConfig,
    #[serde(default)]
    pub source_maps: Option<SourceMapsConfig>,
    #[serde(default)]
    pub module: Option<ModuleConfig>,
    #[serde(default)]
    pub minify: BoolConfig<false>,
}

impl From<&Config> for SerializableConfig {
    fn from(internal: &Config) -> Self {
        SerializableConfig {
            jsc: internal.jsc.clone(),
            source_maps: internal.source_maps.clone(),
            module: internal.module.clone(),
            minify: internal.minify,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableOptions {
    #[serde(flatten)]
    pub config: SerializableConfig,
    #[serde(default)]
    pub source_maps: Option<SourceMapsConfig>,
    #[serde(default)]
    pub output_path: Option<PathBuf>,
}

impl From<&Options> for SerializableOptions {
    fn from(internal: &Options) -> Self {
        SerializableOptions {
            config: SerializableConfig::from(&internal.config),
            source_maps: internal.source_maps.clone(),
            output_path: internal.output_path.clone(),
        }
    }
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
        declarationDir: child
            .declarationDir
            .clone()
            .or_else(|| base.declarationDir.clone()),
        outDir: child.outDir.clone().or_else(|| base.outDir.clone()),
        removeComments: child.removeComments.or(base.removeComments),
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

/// Fetch a `tsconfig.json` and merge it with any `extends` configurations
///
/// ```rust
/// use bndl_convert::{convert, fetch_tsconfig}
///
/// fn main() {
///      match fetch_tsconfig("./tsconfig.json") {
///         Ok(ts_config) => {
///             let config = convert(&ts_config, None, None);
///         }
///         Err(e) => {
///             eprintln!("{}", e)
///         }
///     }
/// }
/// ```
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

fn determine_paths(base_url: &Path, paths: Option<Paths>) -> Paths {
    if base_url.to_string_lossy().len() == 0 {
        return Default::default();
    }

    paths.unwrap_or_default()
}

/// Transform a `tsconfig.json` into an SWC compatible `JSConfig`
///
/// * `minify_output` - Tell SWC to minify the output bundle
/// * `enable_experimental_swc_declarations` - The internal `d.ts` behavior of SWC is weird,
/// you can disable this in most cases (there is probably a reason why it's not exposed to the NPM package)
///
/// ```rust
/// use bndl_convert::{convert, fetch_tsconfig}
/// use swc::config::Options;
///
/// fn main() {
///      match fetch_tsconfig("./tsconfig.json") {
///         Ok(ts_config) => {
///             let config = convert(&ts_config, None, None);
///             let options: Options = Options {
///                 config,
///                 ..Default::default()
///             };
///         }
///         Err(e) => {
///             eprintln!("{}", e)
///         }
///     }
/// }
/// ```
pub fn convert(
    ts_config: &TsConfigJson,
    minify_output: Option<bool>,
    enable_experimental_swc_declarations: Option<bool>,
) -> swc::config::Options {
    let base_url = determine_base_url(ts_config.clone().compilerOptions.baseUrl);
    let paths = determine_paths(&base_url, ts_config.clone().compilerOptions.paths);
    let inline_sources = ts_config.compilerOptions.inlineSources.unwrap_or(false);
    let out_dir = ts_config.clone().compilerOptions.outDir.unwrap_or_default();

    swc::config::Options {
        output_path: if out_dir.is_empty() {
            None
        } else {
            Some(Path::new(&out_dir).to_path_buf())
        },
        source_maps: if inline_sources {
            Some(swc::config::SourceMapsConfig::Str(String::from("inline")))
        } else {
            Some(swc::config::SourceMapsConfig::Bool(
                ts_config.compilerOptions.sourceMap.unwrap_or(false),
            ))
        },
        config: swc::config::Config {
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
                    legacy_decorator: BoolConfig::new(Some(false)),
                    decorator_metadata: BoolConfig::new(Some(
                        ts_config
                            .compilerOptions
                            .experimentalDecorators
                            .unwrap_or_default(),
                    )),
                    ..Default::default()
                })
                .into(),
                preserve_all_comments: if ts_config.compilerOptions.removeComments.is_some() {
                    BoolConfig::new(Some(!ts_config.compilerOptions.removeComments.unwrap()))
                } else {
                    BoolConfig::new(Some(true))
                },
                keep_class_names: BoolConfig::new(Some(true)),
                target: convert_target_to_es_version(&ts_config.compilerOptions.target),
                syntax: Some(Syntax::Typescript(TsConfig {
                    dts: enable_experimental_swc_declarations.unwrap_or(false)
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
        },
        ..Default::default()
    }
}
