use crate::package::{ExecutableTarget, ResolvedModule, SourcePackage};

pub(super) struct PackageCheckTarget<'a> {
    pub(super) module: &'a ResolvedModule,
    pub(super) executable: bool,
}

pub(super) fn selected_executables<'a>(
    package: &'a SourcePackage,
    selected: Option<&str>,
) -> Result<Vec<&'a ExecutableTarget>, String> {
    match selected {
        Some(name) => package
            .executable(name)
            .map(|target| vec![target])
            .ok_or_else(|| {
                format!(
                    "package `{}` has no executable named `{name}`",
                    package.display_name()
                )
            }),
        None => Ok(package.executables().iter().collect()),
    }
}

pub(super) fn check_targets<'a>(
    package: &'a SourcePackage,
    selected: Option<&str>,
) -> Result<Vec<PackageCheckTarget<'a>>, String> {
    if selected.is_some() {
        return selected_executables(package, selected).map(|executables| {
            executables
                .into_iter()
                .map(|target| PackageCheckTarget {
                    module: target.entry(),
                    executable: true,
                })
                .collect()
        });
    }

    let mut targets = vec![PackageCheckTarget {
        module: package.root_module(),
        executable: false,
    }];
    for executable in package.executables() {
        if let Some(existing) = targets
            .iter_mut()
            .find(|target| target.module.id() == executable.entry().id())
        {
            existing.executable = true;
        } else {
            targets.push(PackageCheckTarget {
                module: executable.entry(),
                executable: true,
            });
        }
    }
    Ok(targets)
}
