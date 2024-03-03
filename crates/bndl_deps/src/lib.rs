use log::debug;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::{collections::HashMap, env, path::PathBuf};
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct YarnConfig {
    pub packages: Vec<String>,
    pub nohoist: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum WorkspacesConfig {
    Npm(Vec<String>),
    Yarn(YarnConfig),
}

impl Default for WorkspacesConfig {
    fn default() -> Self {
        WorkspacesConfig::Npm(Vec::new())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct PackageJson {
    pub name: String,
    pub workspaces: Option<WorkspacesConfig>,
    pub dependencies: Option<HashMap<String, String>>,
}

impl PackageJson {
    pub fn from_path(path: &Path) -> Self {
        if !path.exists() {
            return PackageJson::default();
        }

        let package_json_str =
            fs::read_to_string(path).unwrap_or_else(|_| panic!("Unable to read {:?}", path));

        match serde_json::from_str(&package_json_str) {
            Ok(package_json) => package_json,
            Err(err) => {
                debug!("{err} for {:#?}", path);
                Self::default()
            }
        }
    }
}

fn find_workspace_root() -> Result<PathBuf, String> {
    let mut current_dir = env::current_dir().unwrap();

    loop {
        let package_json = PackageJson::from_path(&current_dir.join("package.json"));
        if package_json.workspaces.is_some() {
            debug!("Found workspace root at {:?}", current_dir);

            return Ok(current_dir);
        }

        if current_dir.parent().is_none() {
            break;
        }

        current_dir = current_dir.parent().unwrap().to_path_buf();
    }

    Err("Unable to find workspace root".to_string())
}

fn check_to_ignore_dir(entry: &DirEntry) -> bool {
    entry.file_type().is_dir()
        && (entry.file_name() == "node_modules" || entry.file_name() == "dist")
}

/// Fetches the internal packages of the monorepo with their name and path
pub fn fetch_packages() -> HashMap<String, PathBuf> {
    match find_workspace_root() {
        Ok(root) => {
            let mut packages = HashMap::new();
            let mut it = WalkDir::new(root).into_iter();

            loop {
                let entry = match it.next() {
                    None => break,
                    Some(Err(err)) => {
                        debug!("Error while walking directory: {:?}", err);
                        continue;
                    }
                    Some(Ok(entry)) => entry,
                };

                let path = entry.path();
                if path.is_dir() && check_to_ignore_dir(&entry) {
                    it.skip_current_dir();
                    continue;
                } else if path.is_symlink()
                    || path.is_dir()
                    || path.file_name().unwrap_or_default() != "package.json"
                {
                    continue;
                }

                let package_json = PackageJson::from_path(path);
                packages.insert(package_json.name, path.parent().unwrap().to_owned());
            }

            debug!("Identified monorepo packages: {:?}", packages);

            packages
        }
        Err(err) => {
            debug!("{err}");
            HashMap::new()
        }
    }
}

/// Fetches the internal packages of the monorepo that are used in the current package
pub fn fetch_used_dependencies() -> HashMap<String, PathBuf> {
    let package_json = PackageJson::from_path(Path::new("package.json"));
    let dependencies = package_json.dependencies.unwrap_or_default();

    let internal_dependencies = fetch_packages()
        .iter()
        .filter(|(name, _)| dependencies.contains_key(*name))
        .map(|(name, path)| (name.clone(), path.clone()))
        .collect();

    debug!(
        "Dependencies used by {}: {:?}",
        package_json.name, internal_dependencies
    );

    internal_dependencies
}
