use log::debug;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::{collections::HashMap, env, path::PathBuf};
use walkdir::{DirEntry, WalkDir};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct PackageJson {
    pub name: String,
    pub workspaces: Option<Vec<String>>,
    pub dependencies: Option<HashMap<String, String>>,
}

fn fetch_package_json(path: &Path) -> PackageJson {
    if !path.exists() {
        return PackageJson::default();
    }

    let package_json_str = fs::read_to_string(path).expect("Unable to read package.json");

    match serde_json::from_str(&package_json_str) {
        Ok(package_json) => package_json,
        Err(_) => PackageJson::default(),
    }
}

fn find_workspace_root() -> Result<PathBuf, String> {
    let mut current_dir = env::current_dir().unwrap();

    loop {
        let package_json_path = current_dir.join("package.json");
        if package_json_path.exists() {
            let package_json = fetch_package_json(&package_json_path);
            if package_json.workspaces.is_some() {
                debug!("Found workspace root at {:?}", current_dir);

                return Ok(current_dir);
            }
        }

        if current_dir.parent().is_none() {
            break;
        }

        current_dir = current_dir.parent().unwrap().to_path_buf();
    }

    Err("Unable to find workspace root".to_string())
}

/// Function to check if a directory entry is a 'node_modules' directory
/// and skip it. We aren't interested in any of these contents and are only
/// interested in the internal packages of the monorepo
fn is_node_modules(entry: &DirEntry) -> bool {
    entry.file_type().is_dir() && entry.file_name() == "node_modules"
}

/// Fetches the internal packages of the monorepo with their name and path
pub fn fetch_packages() -> HashMap<String, PathBuf> {
    match find_workspace_root() {
        Ok(root) => {
            let mut packages = HashMap::new();
            let walker = WalkDir::new(root)
                .min_depth(1)
                .into_iter()
                .filter_entry(|e| !is_node_modules(e))
                .filter_map(|e| e.ok());

            for entry in walker {
                let path = entry.path();

                // Only interested in package.json files
                if path.is_dir() || path.file_name().unwrap_or_default() != "package.json" {
                    continue;
                }

                let package_json = fetch_package_json(path);
                packages.insert(package_json.name, path.parent().unwrap().to_owned());
            }

            debug!("Packages: {:?}", packages);

            packages
        }
        Err(_) => HashMap::new(),
    }
}

/// Fetches the internal packages of the monorepo that are used in the current package
pub fn fetch_used_dependencies() -> HashMap<String, PathBuf> {
    let package_json = fetch_package_json(Path::new("package.json"));
    let dependencies = package_json.dependencies.unwrap_or_default();

    let internal_dependencies = fetch_packages()
        .iter()
        .filter(|(name, _)| dependencies.contains_key(*name))
        .map(|(name, path)| (name.clone(), path.clone()))
        .collect();

    debug!("Used dependencies: {:?}", internal_dependencies);

    internal_dependencies
}
