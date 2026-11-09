use super::*;
use crate::manifest::{Dependency, Manifest};
use ratatui::{Terminal, backend::TestBackend};

fn options() -> InstallOptions {
    InstallOptions {
        profile: "default".into(),
        store_dir: None,
        offline: true,
        frozen: false,
        verbose: false,
    }
}

fn press(app: &mut App, code: KeyCode) -> (bool, Option<Request>) {
    app.key(KeyEvent::new(code, KeyModifiers::NONE), &options())
}

fn project_app() -> (tempfile::TempDir, App) {
    let temp = tempfile::tempdir().unwrap();
    let mut manifest = Manifest::new("example".into());
    manifest
        .dependencies
        .insert("fmt".into(), Dependency::Version("11.0.2".into()));
    std::fs::write(
        temp.path().join(FILE_NAME),
        toml::to_string(&manifest).unwrap(),
    )
    .unwrap();
    let mut app = App::new(temp.path()).unwrap();
    // Rendering/state tests never need to start Conan or touch a user cache.
    app.discovery_started = true;
    (temp, app)
}

#[test]
fn discovery_merges_remotes_and_reports_partial_failures() {
    let discovery = catalog::parse(
        br#"{
        "center": {"fmt/11.0.2": {}, "novel-package/2.0": {}, "novel-package/1.0": {}},
        "other": {"novel-package/2.0": {}, "private/1.0@user/channel": {}},
        "broken": {"error": "connection refused"}
    }"#,
    )
    .unwrap();
    assert_eq!(discovery.entries.len(), 2);
    let novel = &discovery.entries["novel-package"];
    assert_eq!(novel.versions.len(), 2);
    assert_eq!(novel.sources.len(), 2);
    assert!(novel.default_version().is_none());
    assert_eq!(discovery.warnings, ["broken: connection refused"]);
    assert!(catalog::parse(b"not JSON").is_err());
    assert!(catalog::parse(b"[]").is_err());
}

#[test]
fn search_finds_aliases_and_subjects_and_never_executes_hotkeys_while_typing() {
    let (_temp, mut app) = project_app();
    press(&mut app, KeyCode::Char('2'));
    press(&mut app, KeyCode::Char('/'));
    for c in "postgres".chars() {
        assert!(!press(&mut app, KeyCode::Char(c)).0);
    }
    assert_eq!(
        app.visible_packages()
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        ["libpq"]
    );
    assert!(app.basket.is_empty());
    assert!(app.modal.is_none());
    assert_eq!(app.tab, 1);
    // Finishing a search does not also install its result.
    assert!(press(&mut app, KeyCode::Enter).1.is_none());
    assert!(app.modal.is_none());
    press(&mut app, KeyCode::Esc);
    assert!(app.visible_packages().len() > 20);
    assert!(app.entries["raylib"].matches("JOGOS gráficos"));
    assert!(app.entries["nlohmann_json"].matches("json"));
}

#[test]
fn selection_keeps_alias_versions_and_validates_the_entire_batch() {
    let (_temp, mut app) = project_app();
    app.add_specs("json sdl2 unusual-package@1.2.3").unwrap();
    assert_eq!(app.basket["nlohmann_json"], "nlohmann_json@3.11.3");
    assert_eq!(app.basket["sdl"], "sdl@2.32.10");
    assert!(app.entries.contains_key("unusual-package"));
    let previous = app.basket.clone();
    assert!(app.add_specs("fmt unsafe;command").is_err());
    assert_eq!(app.basket, previous);
    assert!(app.add_specs("fmt@1.0 fmt@2.0").is_err());
    assert_eq!(app.basket, previous);
    app.install(&options());
    let Some(Modal::Confirm(request)) = &app.modal else {
        panic!("missing preview")
    };
    assert_eq!(
        request.args,
        [
            "add",
            "nlohmann_json@3.11.3",
            "sdl@2.32.10",
            "unusual-package@1.2.3"
        ]
    );
    assert!(press(&mut app, KeyCode::Esc).1.is_none());
    assert_eq!(app.basket, previous);
    app.install(&options());
    assert_eq!(press(&mut app, KeyCode::Enter).1.unwrap().args[0], "add");
}

#[test]
fn dependencies_compare_full_declarations_and_reload_from_disk() {
    let (temp, mut app) = project_app();
    assert!(!app.locked("fmt"));
    std::fs::write(
        temp.path().join("cppm.lock"),
        br#"{"format":1,"dependencies":{"fmt":"11.0.2"}}"#,
    )
    .unwrap();
    app.reload().unwrap();
    assert!(app.locked("fmt"));
    let mut project = Project::discover(temp.path()).unwrap();
    project
        .manifest
        .dependencies
        .insert("fmt".into(), Dependency::Version("12.0.0".into()));
    project.save_manifest().unwrap();
    app.reload().unwrap();
    assert!(!app.locked("fmt"));
    app.tab = 2;
    press(&mut app, KeyCode::Char('d'));
    let request = press(&mut app, KeyCode::Enter).1.unwrap();
    assert_eq!(request.args, ["remove", "fmt"]);
    // The interactive layer does not change the manifest before execution.
    assert!(
        Project::discover(temp.path())
            .unwrap()
            .manifest
            .dependencies
            .contains_key("fmt")
    );
}

#[test]
fn frozen_and_wasm_projects_cannot_queue_incompatible_mutations() {
    let (_temp, mut app) = project_app();
    app.add_specs("json").unwrap();
    let mut frozen = options();
    frozen.frozen = true;
    app.install(&frozen);
    assert!(app.modal.is_none());
    assert!(app.message.contains("congelado"));
    app.project.as_mut().unwrap().manifest.package.target = crate::manifest::Target::Wasm;
    app.install(&options());
    assert!(app.modal.is_none());
    assert!(app.message.contains("WebAssembly"));
}

#[test]
fn commands_include_scripts_and_preserve_global_context_as_separate_arguments() {
    let (temp, app) = project_app();
    let script = app
        .commands
        .iter()
        .find(|c| c.args == ["run", "dev"])
        .unwrap();
    let opts = InstallOptions {
        profile: "profile with spaces".into(),
        store_dir: Some(temp.path().join("shared store")),
        offline: true,
        frozen: true,
        verbose: true,
    };
    let command = action_command(script, temp.path(), &opts).unwrap();
    let args = command
        .get_args()
        .map(|s| s.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        args,
        vec![
            "--profile",
            "profile with spaces",
            "--store-dir",
            opts.store_dir.as_ref().unwrap().to_str().unwrap(),
            "--offline",
            "--frozen-lockfile",
            "--verbose",
            "run",
            "dev"
        ]
    );
    assert_eq!(command.get_current_dir(), Some(temp.path()));
    assert!(
        app.commands
            .iter()
            .any(|r| r.args == ["store", "prune", "--dry-run"])
    );
    assert!(app.commands.iter().any(|r| r.args == ["build", "obf"]));
    assert!(app.commands.iter().any(|r| r.args == ["watch", "dev"]));
    assert!(app.commands.iter().any(|r| r.args == ["watch", "test"]));
    assert!(
        app.commands
            .iter()
            .any(|r| r.args == ["clean", "--dry-run"])
    );
}

#[test]
fn catalog_filters_distinguish_cppm_defaults_and_project_membership() {
    let (_temp, mut app) = project_app();
    app.tab = 1;
    app.filter = 1;
    assert!(
        app.visible_packages()
            .iter()
            .all(|e| e.default_version().is_some())
    );
    app.filter = 2;
    assert!(app.visible_packages().iter().any(|e| e.name == "raylib"));
    assert!(
        app.visible_packages()
            .iter()
            .all(|e| e.default_version().is_none())
    );
    app.filter = 3;
    assert!(app.visible_packages().iter().all(|e| e.name != "fmt"));
    app.query = "no-such-library".into();
    app.selected = 900;
    app.clamp_selection();
    assert_eq!(app.list.selected(), None);
    assert_eq!(app.selected, 0);
}

#[test]
fn views_render_at_wide_compact_and_minimum_sizes_with_empty_results_and_modals() {
    let (_temp, mut app) = project_app();
    app.update_notice =
        Some("Atualização disponível: 0.4.2 → 0.4.3. Execute `cppm update`.".into());
    for (width, height) in [(140, 42), (100, 32), (80, 24), (76, 26), (52, 19), (20, 8)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        for tab in 0..4 {
            app.tab = tab;
            app.reset_selection();
            for query in ["", "no-such-result"] {
                app.query = query.into();
                terminal
                    .draw(|frame| view::draw(frame, &mut app, &options()))
                    .unwrap();
                let buffer = terminal.backend().buffer();
                let screen = buffer
                    .content
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>();
                assert!(screen.contains("cppm"));
                if width >= 100 {
                    assert!(
                        screen.contains("cppm update"),
                        "update notice must remain visible on every tab"
                    );
                }
                if width >= 76 && height >= 24 && query.is_empty() && tab == 0 {
                    assert!(
                        screen.contains("Explorar bibliotecas"),
                        "dashboard actions must fit a normal terminal"
                    );
                }
            }
        }
        for modal in [
            Modal::Help,
            Modal::Add("fmt json".into()),
            Modal::Confirm(Request::new(
                "test",
                &["add", &"some-library ".repeat(90)],
                "details",
                true,
            )),
        ] {
            app.modal = Some(modal);
            for scroll in [0, 10, 300] {
                app.modal_scroll = scroll;
                terminal
                    .draw(|frame| view::draw(frame, &mut app, &options()))
                    .unwrap();
            }
        }
        app.modal = None;
    }
}

#[test]
fn browsing_without_a_project_is_read_only_and_creation_is_explicit() {
    let temp = tempfile::tempdir().unwrap();
    let mut app = App::new(temp.path()).unwrap();
    app.discovery_started = true;
    assert!(app.project.is_none());
    assert!(app.commands.iter().any(|r| r.args == ["init", "."]));
    app.switch_tab(1, &options());
    app.add_specs("json").unwrap();
    app.install(&options());
    assert!(app.modal.is_none());
    assert!(app.error);
    assert!(!temp.path().join(FILE_NAME).exists());
    assert!(!temp.path().join(".cppm").exists());
}
