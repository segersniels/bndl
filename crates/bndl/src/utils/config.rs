use serde::{Deserialize, Serialize};
use std::path::Path;
use std::{collections::HashMap, fs};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PackageJson {
    pub name: String,
    pub workspaces: Option<Vec<String>>,
    pub dependencies: Option<HashMap<String, String>>,
}

pub fn fetch_package_json(path: &Path) -> PackageJson {
    if !path.exists() {
        return PackageJson::default();
    }

    let package_json_str = fs::read_to_string(path).expect("Unable to read package.json");

    match serde_json::from_str(&package_json_str) {
        Ok(package_json) => package_json,
        Err(_) => PackageJson::default(),
    }
}
