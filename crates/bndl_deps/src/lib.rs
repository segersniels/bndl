#[macro_use]
extern crate lazy_static;

use log::debug;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::{collections::HashMap, env, path::PathBuf};
use walkdir::{DirEntry, WalkDir};

#[derive(Default)]
struct State {
    root: PathBuf,
    packages: HashMap<String, PathBuf>,
}

lazy_static! {
    static ref STATE: Mutex<State> = Mutex::new(State::default());
}

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

fn check_to_ignore_dir(entry: &DirEntry) -> bool {
    entry.file_type().is_dir()
        && (entry.file_name() == "node_modules" || entry.file_name() == "dist")
}

/// Responsible for managing the monorepo by determining the internal packages and their dependencies
pub struct Manager {
    pub root: PathBuf,
    pub packages: HashMap<String, PathBuf>,
}

impl Manager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let root = Self::find_workspace_root()?;
        let packages = Self::fetch_packages(&root);

        Ok(Manager { root, packages })
    }

    fn find_workspace_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
        // Check if the root has already been found before
        let mut state = STATE.lock().unwrap();
        if state.root.exists() {
            return Ok(state.root.clone());
        }

        let mut current_dir = env::current_dir()?;
        loop {
            let package_json = PackageJson::from_path(&current_dir.join("package.json"));
            if package_json.workspaces.is_some() {
                debug!("Found workspace root at {:?}", current_dir);

                // Cache the root
                state.root = current_dir.clone();

                return Ok(current_dir);
            }

            match current_dir.parent() {
                Some(parent) => current_dir = parent.to_path_buf(),
                None => break,
            }
        }

        Err("Unable to find workspace root".into())
    }

    /// Fetches the internal packages of the monorepo with their name and path
    fn fetch_packages(root: &PathBuf) -> HashMap<String, PathBuf> {
        // Check if the root has already been found before
        let mut state = STATE.lock().unwrap();
        if !state.packages.is_empty() {
            return state.packages.clone();
        }

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
            }

            if path.is_symlink()
                || path.is_dir()
                || path.file_name().unwrap_or_default() != "package.json"
            {
                continue;
            }

            let package_json = PackageJson::from_path(path);
            packages.insert(package_json.name, path.parent().unwrap().to_owned());
        }

        debug!("Identified monorepo packages: {:?}", packages);

        // Cache the packages
        state.packages = packages.clone();

        packages
    }

    /// Fetches the internal packages of the monorepo that are used in the requested package
    pub fn fetch_used_dependencies(&self, package_json_path: &Path) -> HashMap<String, PathBuf> {
        let package_json = PackageJson::from_path(package_json_path);
        let dependencies = package_json.dependencies.unwrap_or_default();

        let internal_dependencies = self
            .packages
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
}
