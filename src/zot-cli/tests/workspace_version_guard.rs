use std::fs;
use std::path::{Path, PathBuf};

use toml::{Table, Value};

#[test]
fn workspace_internal_dependencies_are_centralized() {
    /*
     * ========================================================================
     * 步骤1：读取工作区依赖源
     * ========================================================================
     * 目标：
     * 1) 找到根 Cargo.toml 的 workspace.dependencies
     * 2) 确认内部 crate 只在根清单集中声明
     */
    eprintln!("开始校验工作区内部依赖源...");

    // 1.1 解析工作区根目录
    let workspace_root = workspace_root();

    // 1.2 读取根清单
    let root_manifest = read_manifest(&workspace_root.join("Cargo.toml"));
    let workspace_dependencies = workspace_dependencies(&root_manifest);

    // 1.3 校验内部 crate 只保留 path，不再手写 version
    assert_workspace_dependency(workspace_dependencies, "zot-core", "src/zot-core");
    assert_workspace_dependency(workspace_dependencies, "zot-desktop", "src/zot-desktop");
    assert_workspace_dependency(workspace_dependencies, "zot-local", "src/zot-local");
    assert_workspace_dependency(workspace_dependencies, "zot-remote", "src/zot-remote");

    eprintln!("工作区内部依赖源校验完成");

    /*
     * ========================================================================
     * 步骤2：校验成员 crate 继承方式
     * ========================================================================
     * 目标：
     * 1) 确认成员 crate 通过 `.workspace = true` 继承内部依赖
     * 2) 防止重新写回 path + version 造成版本漂移
     */
    eprintln!("开始校验成员 crate 的依赖继承方式...");

    // 2.1 校验 zot-local 的内部依赖声明
    assert_member_dependencies(&workspace_root, "src/zot-local/Cargo.toml", &["zot-core"]);

    // 2.2 校验 zot-remote 的内部依赖声明
    assert_member_dependencies(&workspace_root, "src/zot-remote/Cargo.toml", &["zot-core"]);

    // 2.3 校验 zot-cli 的内部依赖声明
    assert_member_dependencies(
        &workspace_root,
        "src/zot-cli/Cargo.toml",
        &["zot-core", "zot-desktop", "zot-local", "zot-remote"],
    );

    assert_member_dependencies(&workspace_root, "src/zot-desktop/Cargo.toml", &["zot-core"]);

    eprintln!("成员 crate 的依赖继承方式校验完成");
}

#[test]
fn workspace_members_inherit_lints_and_changelog_matches_version() {
    let workspace_root = workspace_root();
    let root_manifest = read_manifest(&workspace_root.join("Cargo.toml"));
    let workspace = match root_manifest.get("workspace").and_then(Value::as_table) {
        Some(workspace) => workspace,
        None => panic!("根 Cargo.toml 缺少 [workspace]"),
    };
    let members = match workspace.get("members").and_then(Value::as_array) {
        Some(members) => members,
        None => panic!("根 Cargo.toml 缺少 workspace.members"),
    };

    assert_eq!(members.len(), 5, "workspace 应包含五个 crate");
    for member in members {
        let member = match member.as_str() {
            Some(member) => member,
            None => panic!("workspace member 必须是字符串"),
        };
        let manifest_rel_path = format!("{member}/Cargo.toml");
        let manifest = read_manifest(&workspace_root.join(&manifest_rel_path));
        let inherits_lints = manifest
            .get("lints")
            .and_then(Value::as_table)
            .and_then(|lints| lints.get("workspace"))
            .and_then(Value::as_bool);
        assert_eq!(
            inherits_lints,
            Some(true),
            "{manifest_rel_path} 应使用 `[lints] workspace = true`",
        );
    }

    let version = match workspace
        .get("package")
        .and_then(Value::as_table)
        .and_then(|package| package.get("version"))
        .and_then(Value::as_str)
    {
        Some(version) => version,
        None => panic!("根 Cargo.toml 缺少 workspace.package.version"),
    };
    let changelog_path = workspace_root.join("CHANGELOG.md");
    let changelog = match fs::read_to_string(&changelog_path) {
        Ok(changelog) => changelog,
        Err(error) => panic!("读取 {} 失败: {error}", changelog_path.display()),
    };
    let expected_heading = format!("## [{version}]");
    assert!(
        changelog
            .lines()
            .any(|line| line.starts_with(&expected_heading)),
        "CHANGELOG.md 缺少当前 workspace 版本 heading: {expected_heading}",
    );
}

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    match manifest_dir.ancestors().nth(2) {
        Some(path) => path.to_path_buf(),
        None => panic!("无法从 {manifest_dir:?} 推导工作区根目录"),
    }
}

fn read_manifest(path: &Path) -> Value {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) => panic!("读取清单失败 {}: {error}", path.display()),
    };

    match toml::from_str::<Value>(&content) {
        Ok(value) => value,
        Err(error) => panic!("解析清单失败 {}: {error}", path.display()),
    }
}

fn workspace_dependencies(root_manifest: &Value) -> &Table {
    let workspace = match root_manifest.get("workspace").and_then(Value::as_table) {
        Some(workspace) => workspace,
        None => panic!("根 Cargo.toml 缺少 [workspace]"),
    };

    match workspace.get("dependencies").and_then(Value::as_table) {
        Some(dependencies) => dependencies,
        None => panic!("根 Cargo.toml 缺少 [workspace.dependencies]"),
    }
}

fn dependency_table<'a>(dependencies: &'a Table, name: &str) -> &'a Table {
    match dependencies.get(name).and_then(Value::as_table) {
        Some(dependency) => dependency,
        None => panic!("依赖 {name} 不是表，或不存在"),
    }
}

fn assert_workspace_dependency(dependencies: &Table, name: &str, expected_path: &str) {
    let dependency = dependency_table(dependencies, name);
    let actual_path = dependency.get("path").and_then(Value::as_str);

    assert_eq!(
        actual_path,
        Some(expected_path),
        "根 Cargo.toml 中 {name} 的 path 应为 {expected_path}",
    );
    assert!(
        !dependency.contains_key("version"),
        "根 Cargo.toml 中 {name} 不应再手写 version",
    );
}

fn assert_member_dependencies(
    workspace_root: &Path,
    manifest_rel_path: &str,
    dependency_names: &[&str],
) {
    let manifest = read_manifest(&workspace_root.join(manifest_rel_path));
    let dependencies = match manifest.get("dependencies").and_then(Value::as_table) {
        Some(dependencies) => dependencies,
        None => panic!("{manifest_rel_path} 缺少 [dependencies]"),
    };

    for dependency_name in dependency_names {
        let dependency = dependency_table(dependencies, dependency_name);
        let inherits_from_workspace = dependency.get("workspace").and_then(Value::as_bool);

        assert_eq!(
            inherits_from_workspace,
            Some(true),
            "{manifest_rel_path} 中 {dependency_name} 应使用 `.workspace = true`",
        );
        assert_eq!(
            dependency.len(),
            1,
            "{manifest_rel_path} 中 {dependency_name} 不应混入额外字段",
        );
    }
}
