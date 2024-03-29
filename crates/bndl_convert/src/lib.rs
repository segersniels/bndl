#[macro_use]
extern crate lazy_static;

use bndl_deps::Manager;
use globset::{Glob, GlobSet, GlobSetBuilder};
use log::debug;
use serde::{Deserialize, Serialize};
use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::{env, fs};
use swc::config::{Config, ModuleConfig, Options, SourceMapsConfig};
use swc::{
    config::{JscConfig, Paths},
    BoolConfig,
};
use swc_ecma_parser::{Syntax, TsConfig};
use swc_ecma_transforms_module::{amd, common_js, umd};

lazy_static! {
    static ref TSCONFIG_CONTENT: Mutex<HashMap<PathBuf, String>> = Mutex::new(HashMap::new());
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
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
    pub resolveJsonModule: Option<bool>,
    pub esModuleInterop: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct TsConfigJson {
    pub extends: Option<String>,
    pub compilerOptions: Option<CompilerOptions>,
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
}

impl TsConfigJson {
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
                    resolveJsonModule: child_options
                        .resolveJsonModule
                        .or(base_options.resolveJsonModule),
                    esModuleInterop: child_options
                        .esModuleInterop
                        .or(base_options.esModuleInterop),
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
        cache: &mut MutexGuard<HashMap<PathBuf, String>>,
    ) -> String {
        // Check if we have already fetched this config
        if let Some(content) = cache.get(config_path) {
            return content.clone();
        }

        // Check if we can read the file and cache it
        if let Ok(content) = fs::read_to_string(config_path) {
            cache.insert(config_path.to_path_buf(), content.clone());

            return content;
        }

        let mut relative_path_to_append = Vec::new();
        let mut path = config_path.to_str().unwrap();
        let mut content = String::from("{\"compilerOptions\": {}}");

        // Go through the path and strip the last part until we find a match in the internal packages
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

                content = Self::fetch_config_content(&full_path, internal_packages, cache);

                // Cache the content for future use
                cache.insert(config_path.to_path_buf(), content.clone());

                break;
            }

            path = package_name;
        }

        content
    }

    fn load_and_merge_tsconfig(
        config_path: &Path,
        internal_packages: &HashMap<String, PathBuf>,
        cache: &mut MutexGuard<HashMap<PathBuf, String>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let config_str = Self::fetch_config_content(config_path, internal_packages, cache);
        let mut tsconfig: Self = serde_json::from_str(&config_str)?;

        if let Some(ref extends) = tsconfig.extends {
            let base_config_path = if extends.starts_with('.') {
                // Resolve the path of the base configuration relative to the child configuration
                config_path.parent().unwrap().join(extends)
            } else {
                // Assume `extends` is a path from the root or a module (not implemented here)
                PathBuf::from(extends)
            };

            let base_tsconfig =
                Self::load_and_merge_tsconfig(&base_config_path, internal_packages, cache)?;

            tsconfig.compilerOptions = Self::merge_compiler_options(
                &base_tsconfig.compilerOptions,
                &tsconfig.compilerOptions,
            );
        }

        Ok(tsconfig)
    }

    pub fn from_path(
        config_path: &Path,
        manager: &Manager,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if !config_path.exists() {
            return Ok(Self::default());
        }

        // We fetch the cache beforehand to avoid causing a deadlock trying to lock the mutex multiple times
        let mut content_cache = TSCONFIG_CONTENT.lock().unwrap();

        Self::load_and_merge_tsconfig(config_path, &manager.packages, content_cache.borrow_mut())
    }
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

impl From<Config> for SerializableConfig {
    fn from(internal: Config) -> Self {
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
        let internal = internal.clone();

        SerializableOptions {
            config: SerializableConfig::from(internal.config),
            source_maps: internal.source_maps,
            source_root: internal.source_root,
            output_path: internal.output_path,
            swcrc: internal.swcrc,
        }
    }
}

#[derive(Debug)]
pub struct GlobSetConfig {
    pub include: GlobSet,
    pub exclude: GlobSet,
}

#[derive(Default)]
pub struct CreateConverterOptions {
    pub manager: Option<Manager>,
    pub minify_output: Option<bool>,
    pub enable_experimental_swc_declarations: Option<bool>,
}

/// Convert a `tsconfig.json` into a `swc::config::Options`
/// This is a builder pattern to allow for easy configuration of the `swc::config::Options`
/// based on the `tsconfig.json`
#[derive(Clone)]
pub struct Converter {
    minify_output: Option<bool>,
    enable_experimental_swc_declarations: Option<bool>,
    pub tsconfig: TsConfigJson,
}

impl Converter {
    fn construct_glob_set(&self, glob_candidates: Option<Vec<String>>) -> GlobSet {
        let mut builder = GlobSetBuilder::new();
        let app_dir = env::current_dir().unwrap_or(PathBuf::from("."));

        if let Some(inputs) = glob_candidates {
            for input in inputs {
                let mut glob = input.to_owned();

                if glob.ends_with('/') {
                    glob = glob[0..glob.len() - 1].to_string();
                }

                // Absolute paths can't be matched so ensure we hit all references through a general glob
                if !glob.starts_with("./") && !glob.starts_with('*') {
                    if Path::new(&glob).extension().is_some() {
                        // If the glob has an extension, we can assume it's a file
                        builder.add(Glob::new(format!("*/{glob}").as_str()).unwrap());
                    } else {
                        // If the glob doesn't have an extension, we can assume it's a directory
                        builder.add(Glob::new(format!("{glob}/**").as_str()).unwrap());
                        builder.add(Glob::new(format!("*/{glob}/**").as_str()).unwrap());
                        builder.add(
                            Glob::new(format!("{}/{glob}/**", app_dir.to_str().unwrap()).as_str())
                                .unwrap(),
                        );
                    }
                }

                builder.add(Glob::new(glob.as_str()).unwrap());
            }
        }

        builder.build().expect("Failed to build glob set")
    }

    /// Transforms the `tsconfig.json` includes and excludes into a `GlobSetConfig`
    /// which is the format that `swc` uses for includes and excludes
    pub fn construct_globset(&self) -> GlobSetConfig {
        let tsconfig = self.tsconfig.clone();
        let include = self.construct_glob_set(tsconfig.include);
        let exclude = self.construct_glob_set(tsconfig.exclude);

        GlobSetConfig { include, exclude }
    }

    fn convert_target_to_es_version(
        &self,
        target: &Option<String>,
    ) -> Option<swc_ecma_ast::EsVersion> {
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

    fn convert_module(
        &self,
        module: Option<String>,
        es_module_interop: Option<bool>,
    ) -> Option<swc::config::ModuleConfig> {
        Some(match module {
            Some(module) => match module.to_lowercase().as_str() {
                "amd" => swc::config::ModuleConfig::Amd(amd::Config {
                    config: common_js::Config {
                        no_interop: if let Some(interop) = es_module_interop {
                            !interop
                        } else {
                            false
                        },
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                "commonjs" => swc::config::ModuleConfig::CommonJs(common_js::Config {
                    no_interop: if let Some(interop) = es_module_interop {
                        !interop
                    } else {
                        false
                    },
                    ..Default::default()
                }),
                "cjs" => swc::config::ModuleConfig::CommonJs(Default::default()),
                "es6" => swc::config::ModuleConfig::Es6(Default::default()),
                "es2015" => swc::config::ModuleConfig::Es6(Default::default()),
                "nodenext" => swc::config::ModuleConfig::NodeNext(Default::default()),
                "umd" => swc::config::ModuleConfig::Umd(umd::Config {
                    config: common_js::Config {
                        no_interop: if let Some(interop) = es_module_interop {
                            !interop
                        } else {
                            false
                        },
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                "system" => swc::config::ModuleConfig::SystemJs(Default::default()),
                _ => swc::config::ModuleConfig::CommonJs(Default::default()),
            },
            None => swc::config::ModuleConfig::CommonJs(Default::default()),
        })
    }

    fn determine_base_url(&self, base_url: Option<String>) -> PathBuf {
        if base_url.is_none() {
            return PathBuf::from("");
        }

        let current_dir = env::current_dir().unwrap_or_default();

        current_dir.join(base_url.unwrap().trim_start_matches("./"))
    }

    fn determine_paths(&self, base_url: &Path, paths: Option<Paths>) -> Paths {
        if base_url.to_string_lossy().len() == 0 {
            return Default::default();
        }

        paths.unwrap_or_default()
    }

    /// Based on a given `tsconfig.json` determine the compiled output directory
    pub fn determine_out_dir(&self, override_out_dir: Option<PathBuf>) -> PathBuf {
        if let Some(out_dir) = override_out_dir {
            out_dir
        } else if let Some(compiler_options) = self.tsconfig.clone().compilerOptions {
            PathBuf::from(&compiler_options.outDir.unwrap_or(String::from("dist")))
        } else {
            PathBuf::from("dist")
        }
    }

    pub fn convert(&self) -> swc::config::Options {
        if let Some(compiler_options) = self.tsconfig.compilerOptions.clone() {
            let base_url = self.determine_base_url(compiler_options.baseUrl);
            let paths = self.determine_paths(&base_url, compiler_options.paths);
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
                    minify: BoolConfig::from(self.minify_output),
                    module: self
                        .convert_module(compiler_options.module, compiler_options.esModuleInterop),
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
                        target: self.convert_target_to_es_version(&compiler_options.target),
                        syntax: Some(Syntax::Typescript(TsConfig {
                            dts: self.enable_experimental_swc_declarations.unwrap_or(false)
                                && compiler_options.declaration.unwrap_or_default(),
                            decorators: compiler_options.experimentalDecorators.unwrap_or_default(),
                            ..Default::default()
                        })),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                swcrc: true, // Needs to be forced to true since it otherwise defaults to `false`
                ..Default::default()
            }
        } else {
            swc::config::Options {
                config: swc::config::Config {
                    minify: BoolConfig::from(self.minify_output),
                    jsc: JscConfig {
                        syntax: Some(Syntax::Typescript(TsConfig {
                            dts: self.enable_experimental_swc_declarations.unwrap_or(false),
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

    pub fn from_path(
        config_path: &Path,
        options: CreateConverterOptions,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let manager = match options.manager {
            Some(manager) => manager.clone(),
            None => Manager::new()?,
        };

        Ok(Self {
            tsconfig: TsConfigJson::from_path(config_path, &manager)?,
            minify_output: options.minify_output,
            enable_experimental_swc_declarations: options.enable_experimental_swc_declarations,
        })
    }

    pub fn from_tsconfig(
        tsconfig: &TsConfigJson,
        options: CreateConverterOptions,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            tsconfig: tsconfig.clone(),
            minify_output: options.minify_output,
            enable_experimental_swc_declarations: options.enable_experimental_swc_declarations,
        })
    }
}
