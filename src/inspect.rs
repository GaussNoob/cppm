//! Read-only views of the project's dependencies: `list`, `why`, `licenses`
//! and `outdated`. Everything except `outdated` works offline from the graph
//! saved by the last installation.

use crate::{
    conan::{self, Graph, InstallOptions, graph::Node, versions},
    packages,
    project::Project,
};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// The graph of the last installation, checked against the lockfile so a
/// stale view is never presented as the current resolution.
fn resolved(project: &Project, release: bool) -> Result<Value> {
    let hint = "run `cppm install` to resolve the dependency graph";
    let (graph, lock) = conan::resolved_graph(&project.root, release)
        .with_context(|| format!("no resolved dependencies for this configuration; {hint}"))?;
    let committed: Option<Value> = std::fs::read(project.root.join("cppm.lock"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());
    let current = committed.as_ref().is_some_and(|committed| {
        committed["conan"] == lock
            && committed["dependencies"] == json!(project.manifest.dependencies)
            && committed.get("dev-dependencies").unwrap_or(&json!({}))
                == &json!(project.manifest.dev_dependencies)
    });
    ensure!(
        current,
        "the resolved dependencies are older than cppm.toml or cppm.lock; {hint}"
    );
    Ok(graph)
}

fn label(node: &Node) -> String {
    format!(
        "{} {}",
        node.name().unwrap_or("?"),
        node.version().unwrap_or("?")
    )
}

/// Direct requirements of a node, sorted by name; build tools come last.
fn children<'a>(graph: Graph<'a>, node: &Node<'a>) -> Vec<(Node<'a>, bool)> {
    let mut children: Vec<_> = node
        .edges()
        .filter(|edge| edge.direct && !edge.test)
        .filter_map(|edge| Some((graph.node(edge.id)?, edge.build)))
        .collect();
    children.sort_by_key(|(child, build)| (*build, child.name().unwrap_or_default().to_owned()));
    children
}

/// A package already on the current path is not expanded again.
fn unvisited<'a>(graph: Graph<'a>, node: &Node<'a>, path: &[String]) -> Vec<(Node<'a>, bool)> {
    children(graph, node)
        .into_iter()
        .filter(|(child, _)| !path.iter().any(|id| id == child.id))
        .collect()
}

fn tree_json(graph: Graph, node: &Node, depth: usize, path: &mut Vec<String>) -> Vec<Value> {
    if depth == 0 {
        return Vec::new();
    }
    let mut entries = Vec::new();
    for (child, build) in unvisited(graph, node, path) {
        path.push(child.id.to_owned());
        let nested = tree_json(graph, &child, depth - 1, path);
        path.pop();
        let mut entry = json!({"name": child.name(), "version": child.version()});
        if build {
            entry["build"] = json!(true);
        }
        if !nested.is_empty() {
            entry["dependencies"] = json!(nested);
        }
        entries.push(entry);
    }
    entries
}

fn print_tree(graph: Graph, node: &Node, depth: usize, prefix: &str, path: &mut Vec<String>) {
    if depth == 0 {
        return;
    }
    let children = unvisited(graph, node, path);
    for (index, (child, build)) in children.iter().enumerate() {
        let last = index + 1 == children.len();
        println!(
            "{prefix}{} {}{}",
            if last { "└──" } else { "├──" },
            label(child),
            if *build { " (build tool)" } else { "" }
        );
        path.push(child.id.to_owned());
        let nested = format!("{prefix}{}", if last { "    " } else { "│   " });
        print_tree(graph, child, depth - 1, &nested, path);
        path.pop();
    }
}

/// `cppm list`: direct dependencies, or the resolved tree with `--depth`.
pub fn list(project: &Project, depth: usize, release: bool, as_json: bool) -> Result<()> {
    let manifest = &project.manifest;
    let sections = [
        ("dependencies", &manifest.dependencies),
        ("dev-dependencies", &manifest.dev_dependencies),
    ];
    // Depth 0 is answered by the manifest alone, even before installing.
    let report = if depth > 0 && manifest.has_requirements() {
        Some(resolved(project, release)?)
    } else {
        None
    };
    let graph = report.as_ref().map(Graph::new);
    let nested = |name: &str| graph.and_then(|graph| Some((graph, graph.host(name)?)));
    if as_json {
        let mut output = json!({
            "name": manifest.package.name,
            "version": manifest.package.version,
            "path": project.root,
        });
        for (section, dependencies) in sections {
            let entries: Vec<Value> = dependencies
                .iter()
                .map(|(name, dependency)| {
                    let mut entry = json!({"name": name, "version": dependency.version()});
                    if let Some((graph, node)) = nested(name) {
                        let children = tree_json(graph, &node, depth, &mut vec![node.id.into()]);
                        if !children.is_empty() {
                            entry["dependencies"] = json!(children);
                        }
                    }
                    entry
                })
                .collect();
            output[section] = json!(entries);
        }
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }
    println!(
        "{} {} {}",
        manifest.package.name,
        manifest.package.version,
        project.root.display()
    );
    if !manifest.has_requirements() {
        println!("No dependencies; add one with `cppm add <package>`.");
    }
    for (section, dependencies) in sections {
        if dependencies.is_empty() {
            continue;
        }
        println!("\n{section}:");
        for (index, (name, dependency)) in dependencies.iter().enumerate() {
            let last = index + 1 == dependencies.len();
            println!(
                "{} {name} {}",
                if last { "└──" } else { "├──" },
                dependency.version()
            );
            if let Some((graph, node)) = nested(name) {
                let prefix = if last { "    " } else { "│   " };
                print_tree(graph, &node, depth, prefix, &mut vec![node.id.into()]);
            }
        }
    }
    Ok(())
}

fn paths<'a>(
    graph: Graph<'a>,
    node: &Node<'a>,
    target: &str,
    path: &mut Vec<Node<'a>>,
    found: &mut BTreeSet<Vec<String>>,
) {
    for (child, _) in children(graph, node) {
        if path.iter().any(|seen| seen.id == child.id) {
            continue;
        }
        path.push(child);
        if child.name() == Some(target) {
            found.insert(path.iter().map(label).collect());
        } else {
            paths(graph, &child, target, path, found);
        }
        path.pop();
    }
}

/// `cppm why`: every chain of requirements that brings a package in.
pub fn why(project: &Project, package: &str, release: bool, as_json: bool) -> Result<()> {
    let name = packages::canonical_name(package);
    ensure!(
        project.manifest.has_requirements(),
        "this project has no dependencies"
    );
    let report = resolved(project, release)?;
    let graph = Graph::new(&report);
    let root = graph
        .root()
        .context("the resolved graph has no project node; run `cppm install`")?;
    let mut found = BTreeSet::new();
    paths(graph, &root, &name, &mut Vec::new(), &mut found);
    if found.is_empty() {
        let names: Vec<String> = graph
            .nodes()
            .filter_map(|node| node.name().map(str::to_owned))
            .collect();
        bail!(
            "'{name}' is not in the dependency graph{}",
            crate::suggest::hint(&name, &names)
        );
    }
    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"package": name, "paths": found}))?
        );
        return Ok(());
    }
    for path in found {
        let direct = path.len() == 1;
        println!(
            "{} > {}{}",
            project.manifest.package.name,
            path.join(" > "),
            if direct { "  (direct dependency)" } else { "" }
        );
    }
    Ok(())
}

/// `cppm licenses`: the licenses declared by every library in the graph.
pub fn licenses(project: &Project, release: bool, as_json: bool) -> Result<()> {
    if !project.manifest.has_requirements() {
        println!("{}", if as_json { "{}" } else { "No dependencies." });
        return Ok(());
    }
    let report = resolved(project, release)?;
    let mut by_license: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for node in Graph::new(&report)
        .nodes()
        .filter(|node| node.is_host() && !node.is_consumer())
    {
        let mut licenses = node.licenses();
        if licenses.is_empty() {
            licenses.push("unknown");
        }
        for license in licenses {
            by_license
                .entry(license.to_owned())
                .or_default()
                .insert(label(&node));
        }
    }
    if as_json {
        println!("{}", serde_json::to_string_pretty(&by_license)?);
        return Ok(());
    }
    for (license, packages) in by_license {
        println!("{license}");
        for package in packages {
            println!("  {package}");
        }
    }
    println!("\nLicenses are declared by each Conan recipe; verify them before distributing.");
    Ok(())
}

/// `cppm outdated`: newer versions of the direct dependencies.
pub fn outdated(project: &Project, options: &InstallOptions, as_json: bool) -> Result<()> {
    let entries: Vec<_> = versions::report(project, options)?
        .into_iter()
        .filter(|entry| entry.latest.is_some())
        .collect();
    if as_json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }
    if entries.is_empty() {
        println!("All direct dependencies are up to date.");
        return Ok(());
    }
    let rows: Vec<[String; 4]> = entries
        .iter()
        .map(|entry| {
            [
                format!("{}{}", entry.name, if entry.dev { " (dev)" } else { "" }),
                entry.current.clone(),
                entry
                    .wanted
                    .clone()
                    .unwrap_or_else(|| entry.current.clone()),
                entry.latest.clone().unwrap_or_default(),
            ]
        })
        .collect();
    let header = ["Package", "Current", "Wanted", "Latest"].map(String::from);
    let widths: Vec<usize> = (0..4)
        .map(|column| {
            rows.iter()
                .chain([&header])
                .map(|row| row[column].chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    for row in std::iter::once(&header).chain(&rows) {
        let line: Vec<String> = row
            .iter()
            .zip(&widths)
            .map(|(cell, width)| format!("{cell:<width$}"))
            .collect();
        println!("{}", line.join("  ").trim_end());
    }
    println!(
        "\n`cppm upgrade` moves to Wanted (same major version); `cppm upgrade --latest` moves to Latest."
    );
    Ok(())
}
