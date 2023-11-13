use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::{env, io};
use walkdir::WalkDir;

use super::config::{fetch_package_json, PackageJson};

fn find_workspace_root() -> Result<PathBuf, String> {
    let mut current_dir = env::current_dir().unwrap();

    loop {
        let package_json_path = current_dir.join("package.json");
        if package_json_path.exists() {
            let package_json = fetch_package_json(&package_json_path);
            if package_json.workspaces.is_some() {
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

fn extract_name_from_scoped_package_name(name: &str) -> &str {
    if !name.starts_with('@') {
        name
    } else {
        name.split('/').nth(1).unwrap_or(name)
    }
}

fn determine_scoped_package_name(name: &str, scope: &str) -> String {
    if name.starts_with('@') {
        name.to_string()
    } else {
        format!("{}/{}", scope, name)
    }
}

// Assume that the workspace name follows the format @prefix/name
fn get_workspace_scope(package_json: &PackageJson) -> String {
    return package_json
        .name
        .split('/')
        .next()
        .unwrap_or(&package_json.name)
        .to_string();
}

fn determine_internal_packages(
    package_json: &PackageJson,
    root: &Path,
) -> Result<HashMap<String, PathBuf>, String> {
    let mut paths_by_package = HashMap::new();

    let scope = get_workspace_scope(package_json);
    let packages_dir = root.join("packages");

    let packages: HashSet<_> = WalkDir::new(&packages_dir)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir())
        .map(|e| determine_scoped_package_name(e.file_name().to_str().unwrap(), &scope))
        .collect();

    if let Some(dependencies) = &package_json.dependencies {
        for dep in dependencies.keys() {
            if packages.contains(dep) {
                let dist_path = packages_dir
                    .join(extract_name_from_scoped_package_name(dep))
                    .join("dist");
                let path_to_set = if dist_path.exists() {
                    dist_path
                } else {
                    packages_dir.join(extract_name_from_scoped_package_name(dep))
                };
                paths_by_package.insert(dep.clone(), path_to_set);
            }
        }
    }

    Ok(paths_by_package)
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

pub fn bundle(out_dir: &str) {
    let root = find_workspace_root().unwrap();
    let package_json = fetch_package_json(Path::new("package.json"));

    match determine_internal_packages(&package_json, &root) {
        Ok(paths_by_package) => {
            for (name, path) in paths_by_package.iter() {
                let package_name = extract_name_from_scoped_package_name(&package_json.name);
                let destination = Path::new(&root)
                    .join("apps")
                    .join(package_name)
                    .join(out_dir)
                    .join("node_modules")
                    .join(name);

                copy_dir_all(path, destination).expect("Unable to copy");
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    };
}
