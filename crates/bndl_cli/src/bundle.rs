use bndl_convert::{Converter, CreateConverterOptions};
use bndl_deps::Manager;
use log::debug;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::env;
use std::path::PathBuf;

use crate::utils::fs::copy_dir_all;

#[derive(Clone)]
/// In charge of bundling internal monorepo dependencies together
pub struct Bundler {
    manager: Manager,
}

impl Bundler {
    pub fn new(manager: Option<&Manager>) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Bundler {
            manager: match manager {
                Some(manager) => manager.clone(),
                None => Manager::new()?,
            },
        })
    }

    pub fn bundle(&self, app_out_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let app_dir = env::current_dir().unwrap_or(PathBuf::from("."));
        let dependencies: std::collections::HashMap<String, std::path::PathBuf> = self
            .manager
            .fetch_used_dependencies(&app_dir.join("package.json"));

        dependencies.into_par_iter().for_each(|(name, path)| {
            let config_path = path.join("tsconfig.json");
            let destination = app_dir.join(app_out_path).join("node_modules").join(name);

            let source = match Converter::from_path(
                &config_path,
                CreateConverterOptions {
                    minify_output: None,
                    enable_experimental_swc_declarations: None,
                    manager: Some(self.manager.clone()),
                },
            ) {
                Ok(ref converter) => {
                    // Don't assume all internal dependencies use the same output directory so we have to
                    // check the tsconfig.json of each dependency
                    let out_dir = converter.determine_out_dir(None);
                    let compiled_dependency_path = path.join(out_dir);

                    // Check if we have to copy over the compiled dependency or the source code directly
                    if compiled_dependency_path.exists() {
                        compiled_dependency_path
                    } else {
                        path.to_owned()
                    }
                }
                Err(err) => {
                    debug!("{err} for {:#?}", path);
                    path.to_owned()
                }
            };

            match copy_dir_all(&source, &destination) {
                Ok(_) => {
                    debug!("Copied {:?} to {:?}", source, destination);
                }
                Err(err) => {
                    debug!("Unable to copy {:?} to {:?}", source, destination);
                    debug!("{err}");
                }
            }
        });

        Ok(())
    }
}
