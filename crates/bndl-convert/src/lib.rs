use log::debug;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    pub inlineSourceMap: Option<bool>,
    pub declarationDir: Option<String>,
    pub outDir: Option<String>,
    pub removeComments: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TsConfigJson {
    pub extends: Option<String>,
    pub compilerOptions: Option<CompilerOptions>,
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
    pub inline_sources_content: BoolConfig<true>,
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
            inline_sources_content: internal.inline_sources_content,
            module: internal.module.clone(),
            minify: internal.minify,
        }
    }
}

const fn default_swcrc() -> bool {
    true
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableOptions {
    #[serde(flatten)]
    pub config: SerializableConfig,
    #[serde(default)]
    pub source_maps: Option<SourceMapsConfig>,
    #[serde(default)]
    pub source_root: Option<String>,
    #[serde(default)]
    pub output_path: Option<PathBuf>,
    #[serde(default = "default_swcrc")]
    pub swcrc: bool,
}

impl From<&Options> for SerializableOptions {
    fn from(internal: &Options) -> Self {
        SerializableOptions {
            config: SerializableConfig::from(&internal.config),
            source_maps: internal.source_maps.clone(),
            source_root: internal.source_root.clone(),
            output_path: internal.output_path.clone(),
            swcrc: internal.swcrc,
        }
    }
}

fn merge_compiler_options(
    base: &Option<CompilerOptions>,
    child: &Option<CompilerOptions>,
) -> Option<CompilerOptions> {
    if child.is_none() {
        return base.clone();
    }

    if let Some(base_options) = base {
        if let Some(child_options) = child {
            // If both base and child are valid configs, merge them together
            Some(CompilerOptions {
                module: child_options
                    .module
                    .clone()
                    .or_else(|| base_options.module.clone()),
                declaration: child_options.declaration.or(base_options.declaration),
                experimentalDecorators: child_options
                    .experimentalDecorators
                    .or(base_options.experimentalDecorators),
                target: child_options
                    .target
                    .clone()
                    .or_else(|| base_options.target.clone()),
                sourceMap: child_options.sourceMap.or(base_options.sourceMap),
                baseUrl: child_options
                    .baseUrl
                    .clone()
                    .or_else(|| base_options.baseUrl.clone()),
                paths: child_options
                    .paths
                    .clone()
                    .or_else(|| base_options.paths.clone()),
                inlineSources: child_options.inlineSources.or(base_options.inlineSources),
                inlineSourceMap: child_options
                    .inlineSourceMap
                    .or(base_options.inlineSourceMap),
                declarationDir: child_options
                    .declarationDir
                    .clone()
                    .or_else(|| base_options.declarationDir.clone()),
                outDir: child_options
                    .outDir
                    .clone()
                    .or_else(|| base_options.outDir.clone()),
                removeComments: child_options.removeComments.or(base_options.removeComments),
            })
        } else {
            // Child is not a valid config, return the base and don't bother merging
            base.clone()
        }
    } else {
        // Base is not a valid config, don't bother merging
        child.clone()
    }
}

/// We will attempt to find the config in the internal packages if we can't resolve
/// it as a relative path. We do this by constantly stripping the last part from the
/// path and checking if the stripped down path exists in the internal packages list.
/// Once we identify it's an internal package we fetch the config from the internal package
/// by appending what we stripped during our lookup.
///
/// If we can't find the config in the internal packages, we return an empty config.
fn fetch_config_content(
    config_path: &Path,
    internal_packages: &HashMap<String, PathBuf>,
) -> String {
    match fs::read_to_string(config_path) {
        Ok(config_str) => config_str,
        Err(_) => {
            let mut relative_path_to_append = Vec::new();
            let mut path = config_path.to_str().unwrap();
            let mut content = String::from("{\"compilerOptions\": {}}");

            while let Some(index) = path.rfind('/') {
                let package_name = &path[..index];

                // Keep track of the parts we strip from the path since we need to append them later
                relative_path_to_append.push(&path[index + 1..]);

                // Look up the path for the specified package
                if let Some(package_path) = internal_packages.get(package_name) {
                    // Need to reverse to get the correct order again since we stripped from the end
                    relative_path_to_append.reverse();

                    // Construct full path to try and fetch
                    let full_path = package_path.join(relative_path_to_append.join("/"));
                    debug!("Found internal extend {:?}", full_path);

                    content = fetch_config_content(&full_path, internal_packages);

                    break;
                } else {
                    path = package_name;
                }
            }

            content
        }
    }
}

fn load_and_merge_tsconfig(
    config_path: &Path,
    internal_packages: &HashMap<String, PathBuf>,
) -> serde_json::Result<TsConfigJson> {
    let config_str = fetch_config_content(config_path, internal_packages);
    let mut tsconfig: TsConfigJson = serde_json::from_str(&config_str)?;

    if let Some(ref extends) = tsconfig.extends {
        let base_config_path = if extends.starts_with('.') {
            // Resolve the path of the base configuration relative to the child configuration
            config_path.parent().unwrap().join(extends)
        } else {
            // Assume `extends` is a path from the root or a module (not implemented here)
            PathBuf::from(extends)
        };

        let base_tsconfig = load_and_merge_tsconfig(&base_config_path, internal_packages)?;
        tsconfig.compilerOptions =
            merge_compiler_options(&base_tsconfig.compilerOptions, &tsconfig.compilerOptions);
    }

    Ok(tsconfig)
}

pub fn fetch_tsconfig(config_path: &Path) -> Result<TsConfigJson, String> {
    if !config_path.exists() {
        return Err(format!("Unable to find {:?}", config_path));
    }

    let packages = bndl_deps::fetch_packages();

    match load_and_merge_tsconfig(config_path, &packages) {
        Ok(tsconfig) => Ok(tsconfig),
        Err(e) => Err(format!("Error parsing tsconfig.json: {}", e)),
    }
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
    PathBuf::from(base_url.unwrap_or_default().trim_start_matches("./"))
}

fn determine_paths(base_url: &Path, paths: Option<Paths>) -> Paths {
    if base_url.to_string_lossy().len() == 0 {
        return Default::default();
    }

    paths.unwrap_or_default()
}

fn convert_impl(
    tsconfig: &TsConfigJson,
    minify_output: Option<bool>,
    enable_experimental_swc_declarations: Option<bool>,
) -> swc::config::Options {
    if let Some(compiler_options) = tsconfig.compilerOptions.clone() {
        let base_url = determine_base_url(compiler_options.baseUrl);
        let paths = determine_paths(&base_url, compiler_options.paths);
        let inline_source_map = compiler_options.inlineSourceMap.unwrap_or(false);
        let inline_sources_content = compiler_options.inlineSources.unwrap_or(false);
        let out_dir = compiler_options.outDir.unwrap_or_default();

        swc::config::Options {
            output_path: if out_dir.is_empty() {
                None
            } else {
                Some(PathBuf::from(&out_dir))
            },
            source_maps: if inline_source_map {
                Some(swc::config::SourceMapsConfig::Str(String::from("inline")))
            } else {
                Some(swc::config::SourceMapsConfig::Bool(
                    compiler_options.sourceMap.unwrap_or(false),
                ))
            },
            config: swc::config::Config {
                minify: BoolConfig::from(minify_output),
                module: convert_module(&compiler_options.module),
                inline_sources_content: BoolConfig::from(inline_sources_content),
                source_maps: if inline_source_map {
                    Some(swc::config::SourceMapsConfig::Str(String::from("inline")))
                } else {
                    Some(swc::config::SourceMapsConfig::Bool(
                        compiler_options.sourceMap.unwrap_or(false),
                    ))
                },
                jsc: JscConfig {
                    base_url,
                    paths,
                    transform: Some(swc::config::TransformConfig {
                        legacy_decorator: BoolConfig::new(Some(false)),
                        decorator_metadata: BoolConfig::new(Some(
                            compiler_options.experimentalDecorators.unwrap_or_default(),
                        )),
                        ..Default::default()
                    })
                    .into(),
                    preserve_all_comments: if compiler_options.removeComments.is_some() {
                        BoolConfig::new(Some(!compiler_options.removeComments.unwrap()))
                    } else {
                        BoolConfig::new(Some(true))
                    },
                    keep_class_names: BoolConfig::new(Some(true)),
                    target: convert_target_to_es_version(&compiler_options.target),
                    syntax: Some(Syntax::Typescript(TsConfig {
                        dts: enable_experimental_swc_declarations.unwrap_or(false)
                            && compiler_options.declaration.unwrap_or_default(),
                        decorators: compiler_options.experimentalDecorators.unwrap_or_default(),
                        ..Default::default()
                    })),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        }
    } else {
        swc::config::Options {
            config: swc::config::Config {
                minify: BoolConfig::from(minify_output),
                jsc: JscConfig {
                    syntax: Some(Syntax::Typescript(TsConfig {
                        dts: enable_experimental_swc_declarations.unwrap_or(false),
                        ..Default::default()
                    })),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

/// Transform a `tsconfig.json` into an SWC compatible `JSConfig`. Can fail if the
/// `tsconfig.json` is invalid or can't be fetched.
///
/// * `minify_output` - Tell SWC to minify the output bundle
/// * `enable_experimental_swc_declarations` - The internal `d.ts` behavior of SWC is weird,
/// you can disable this in most cases (there is probably a reason why it's not exposed to the NPM package)
pub fn convert_from_path(
    config_path: &Path,
    minify_output: Option<bool>,
    enable_experimental_swc_declarations: Option<bool>,
) -> Result<swc::config::Options, String> {
    let tsconfig = fetch_tsconfig(config_path)?;
    let options = convert_impl(
        &tsconfig,
        minify_output,
        enable_experimental_swc_declarations,
    );

    Ok(options)
}

pub fn convert_from_tsconfig(
    tsconfig: &TsConfigJson,
    minify_output: Option<bool>,
    enable_experimental_swc_declarations: Option<bool>,
) -> swc::config::Options {
    convert_impl(
        tsconfig,
        minify_output,
        enable_experimental_swc_declarations,
    )
}
