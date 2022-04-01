// Copyright (c) Aptos
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]

use move_binary_format::file_format::CompiledModule;
use move_command_line_common::files::{extension_equals, find_filenames, MOVE_EXTENSION};
use move_compiler::{
    compiled_unit::{CompiledUnit, NamedCompiledModule},
    shared::{NumberFormat, NumericalAddress},
};
use move_package::compilation::compiled_package::CompiledPackage;
use once_cell::sync::Lazy;
use std::{collections::BTreeMap, path::PathBuf};
use tempfile::tempdir;

pub mod natives;
pub mod release;

const CORE_MODULES_DIR: &str = "core/sources";
const APTOS_MODULES_DIR: &str = "aptos-framework/sources";

static APTOS_FRAMEWORK_PKG: Lazy<CompiledPackage> = Lazy::new(|| package("aptos-framework"));

pub fn path_in_crate<S>(relative: S) -> PathBuf
where
    S: Into<String>,
{
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(relative.into());
    path
}

pub fn core_modules_full_path() -> String {
    format!("{}/{}", env!("CARGO_MANIFEST_DIR"), CORE_MODULES_DIR)
}

pub fn aptos_modules_full_path() -> String {
    format!("{}/{}", env!("CARGO_MANIFEST_DIR"), APTOS_MODULES_DIR)
}

pub fn aptos_files_no_dependencies() -> Vec<String> {
    let mut files = move_files_in_path(APTOS_MODULES_DIR);
    files.extend(move_files_in_path(CORE_MODULES_DIR));
    files
}

fn move_files_in_path(path: &str) -> Vec<String> {
    let modules_path = path_in_crate(path);
    find_filenames(&[modules_path], |p| extension_equals(p, MOVE_EXTENSION)).unwrap()
}

pub fn aptos_files() -> Vec<String> {
    let mut files = move_stdlib::move_stdlib_files();
    files.extend(aptos_files_no_dependencies());
    files
}

pub fn aptos_framework_named_addresses() -> BTreeMap<String, NumericalAddress> {
    named_addresses(&*APTOS_FRAMEWORK_PKG)
}

fn named_addresses(pkg: &CompiledPackage) -> BTreeMap<String, NumericalAddress> {
    pkg.compiled_package_info
        .address_alias_instantiation
        .iter()
        .map(|(name, addr)| {
            (
                name.to_string(),
                NumericalAddress::new(addr.into_bytes(), NumberFormat::Hex),
            )
        })
        .collect()
}

fn package(name: &str) -> CompiledPackage {
    let build_config = move_package::BuildConfig {
        install_dir: Some(tempdir().unwrap().path().to_path_buf()),
        ..Default::default()
    };
    build_config
        .compile_package(&path_in_crate(name), &mut Vec::new())
        .unwrap()
}

fn module_blobs(pkg: &CompiledPackage) -> Vec<Vec<u8>> {
    pkg.transitive_compiled_units()
        .iter()
        .filter_map(|unit| match unit {
            CompiledUnit::Module(NamedCompiledModule { module, .. }) => {
                let mut bytes = vec![];
                module.serialize(&mut bytes).unwrap();
                Some(bytes)
            }
            CompiledUnit::Script(_) => None,
        })
        .collect()
}

pub fn aptos_module_blobs() -> Vec<Vec<u8>> {
    module_blobs(&*APTOS_FRAMEWORK_PKG)
}

pub fn aptos_modules() -> Vec<CompiledModule> {
    APTOS_FRAMEWORK_PKG
        .transitive_compiled_units()
        .iter()
        .filter_map(|unit| match unit {
            CompiledUnit::Module(NamedCompiledModule { module, .. }) => Some(module.clone()),
            CompiledUnit::Script(_) => None,
        })
        .collect()
}
