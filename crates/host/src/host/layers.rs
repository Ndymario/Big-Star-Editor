//! What lies beneath a module: the bytes of a ROM path as the layers below one module supply them. A save compares
//! against it, so a file edited back to what the modules below (or the clean ROM) already give is dropped from the
//! module rather than kept there as an identical copy.
//!
//! Both readers start from the clean ROM, never the built one: a build is not necessarily current, and a file no
//! module supplies is the clean ROM's by definition.

use std::collections::HashMap;

use bigstar_patching::{FileManifest, PlanScope};
use bigstar_project::{EffectiveFilesystem, split_rom_address};

use super::*;

/// Readers over the clean ROM, built when first asked and dropped whenever the plan is.
#[derive(Default)]
pub(super) struct Layers {
    /// The clean ROM under the whole plan.
    current: Option<EffectiveFilesystem>,
    /// By module key: the clean ROM under `files plan --below-module`, or `None` when NCPatcher could not answer.
    below: HashMap<String, Option<EffectiveFilesystem>>,
}

/// The project's clean ROM, or before one is chosen, the ROM the project reads.
pub(super) fn clean_rom(open: &OpenProject) -> Option<RomReader> {
    let clean = clean_rom_file(&open.directory, &open.settings).filter(|_| open.settings.clean_rom_problem.is_none());
    clean.and_then(|file| RomReader::open(file).ok()).or_else(|| open.rom_file.as_ref().and_then(|file| RomReader::open(file).ok()))
}

fn reader(open: &OpenProject, plan: Option<FileManifest>) -> EffectiveFilesystem {
    let mut files = EffectiveFilesystem::new();
    files.set_rom(clean_rom(open));
    files.set_plan(&open.directory, plan);
    files
}

/// Who supplies `address` in a plan: a module's key, empty for the project's own files, or `None` for the ROM's own
/// bytes.
fn supplier(plan: &FileManifest, address: &str) -> Option<String> {
    let split = split_rom_address(address);
    let file = plan.find(&split.container)?;
    let (source, module) = match split.inside_archive() {
        true => file.members.iter().find(|member| member.path == split.member).map(|member| (&member.source, &member.module))?,
        false => (&file.source, &file.module),
    };
    (!source.is_empty()).then(|| module.clone())
}

impl Shared {
    /// What `address` reads as from the layers beneath `module`'s write layer; `None` when nothing there holds it, or
    /// when it cannot be told, which a caller treats the same: as different.
    pub(super) fn below_bytes(&self, open: &mut OpenProject, module: &str, address: &str) -> Option<Vec<u8>> {
        self.ensure_patcher(open);
        // When the file comes from the ROM, or from a module after this one, the whole plan already reads it from
        // below; only when this module or one above it supplies the file is NCPatcher asked what lies beneath.
        let from_below = match open.patcher.as_ref() {
            Some((graph, Some(plan))) => match supplier(plan, address) {
                None => true,
                Some(supplier) => {
                    let order = |key: &str| graph.modules.iter().position(|candidate| candidate.key == key);
                    matches!((order(&supplier), order(module)), (Some(supplier), Some(module)) if supplier > module)
                }
            },
            _ => true,
        };
        if from_below {
            if open.layers.current.is_none() {
                let plan = open.patcher.as_ref().and_then(|(_, plan)| plan.clone());
                open.layers.current = Some(reader(open, plan));
            }
            return open.layers.current.as_mut()?.read(address).ok();
        }
        if !open.layers.below.contains_key(module) {
            let scope = PlanScope::BelowModule(module.to_owned());
            let below = match self.ncpatcher(open).plan_files(&open.directory, &open.settings.active_variant, Some(&scope)) {
                Ok(plan) => Some(reader(open, Some(plan))),
                Err(error) => {
                    self.log(LogLevel::Warning, format!("Could not ask NCPatcher what lies below module {module}: {}", error.message));
                    None
                }
            };
            open.layers.below.insert(module.to_owned(), below);
        }
        open.layers.below.get_mut(module)?.as_mut()?.read(address).ok()
    }

    pub(super) fn matches_below(&self, open: &mut OpenProject, module: &str, address: &str, bytes: &[u8]) -> bool {
        self.below_bytes(open, module, address).is_some_and(|below| below == bytes)
    }
}

/// Moves `module`'s copy of `address` into the trash, when it has one, and says where it was.
pub(super) fn drop_override(open: &OpenProject, module: &ModuleInfo, address: &str) -> Result<Option<PathBuf>> {
    let target = module_file_target(module, &open.settings.active_variant, address, &[])?;
    if !target.is_file() {
        return Ok(None);
    }
    move_to_trash(&open.directory, address, &target)?;
    Ok(Some(target))
}

/// Moves a module file into `.big-star-trash`, under the time and its ROM path, kept for the person to restore by hand.
pub(super) fn move_to_trash(project: &Path, address: &str, source: &Path) -> Result<()> {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let trash = project.join(TRASH_DIR).join(stamp.to_string()).join(address.replace('!', "/"));
    if let Some(parent) = trash.parent() {
        std::fs::create_dir_all(parent).map_err(|_| Error::new(ErrorCode::IoError, "could not create the trash directory"))?;
    }
    std::fs::rename(source, &trash).map_err(|_| Error::new(ErrorCode::IoError, "could not move the module file to the trash"))
}
