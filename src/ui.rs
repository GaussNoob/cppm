mod catalog;
mod view;

use crate::{conan::InstallOptions, manifest::FILE_NAME, packages, project::Project};
use anyhow::{Context, Result, ensure};
use catalog::{Entry, Fetch};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, enable_raw_mode},
};
use ratatui::{DefaultTerminal, widgets::ListState};
use std::{
    collections::BTreeMap,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

const TABS: &[&str] = &["Visão geral", "Explorar", "Dependências", "Comandos"];

#[derive(Clone)]
struct Request {
    title: String,
    args: Vec<String>,
    detail: String,
    project: bool,
}

impl Request {
    fn new(title: &str, args: &[&str], detail: &str, project: bool) -> Self {
        Self {
            title: title.into(),
            args: args.iter().map(|s| (*s).into()).collect(),
            detail: detail.into(),
            project,
        }
    }

    fn preview(&self) -> String {
        std::iter::once("cppm".to_string())
            .chain(self.args.iter().map(|arg| {
                if arg.chars().any(char::is_whitespace) {
                    format!("{arg:?}")
                } else {
                    arg.clone()
                }
            }))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

enum Modal {
    Confirm(Request),
    Add(String),
    Help,
}

struct App {
    cwd: PathBuf,
    project: Option<Project>,
    lock: Option<serde_json::Value>,
    entries: BTreeMap<String, Entry>,
    commands: Vec<Request>,
    tab: usize,
    selected: usize,
    list: ListState,
    query: String,
    searching: bool,
    category: usize,
    filter: usize,
    basket: BTreeMap<String, String>,
    modal: Option<Modal>,
    modal_scroll: u16,
    message: String,
    error: bool,
    fetch: Option<Fetch>,
    discovery_started: bool,
    discovery_status: String,
}

impl App {
    fn new(cwd: &Path) -> Result<Self> {
        let mut app = Self {
            cwd: cwd.into(),
            project: None,
            lock: None,
            entries: catalog::starter(),
            commands: Vec::new(),
            tab: 0,
            selected: 0,
            list: ListState::default(),
            query: String::new(),
            searching: false,
            category: 0,
            filter: 0,
            basket: BTreeMap::new(),
            modal: None,
            modal_scroll: 0,
            message: "Seu C++ em um só lugar. Tab muda de aba; ? mostra os atalhos.".into(),
            error: false,
            fetch: None,
            discovery_started: false,
            discovery_status: "Sugestões locais • abra Explorar para consultar o Conan".into(),
        };
        app.reload()?;
        Ok(app)
    }

    fn reload(&mut self) -> Result<()> {
        self.project = if self
            .cwd
            .ancestors()
            .any(|dir| dir.join(FILE_NAME).is_file())
        {
            Some(Project::discover(&self.cwd)?)
        } else {
            None
        };
        self.lock = self.project.as_ref().and_then(|project| {
            std::fs::read(project.root.join("cppm.lock"))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .filter(|value| value["format"] == 1)
        });
        if let Some(project) = &self.project {
            for name in project.manifest.dependencies.keys() {
                self.entries
                    .entry(name.clone())
                    .or_insert_with(|| Entry::new(name));
            }
        }
        self.commands = quick_commands(self.project.as_ref());
        self.clamp_selection();
        Ok(())
    }

    fn notify(&mut self, message: impl Into<String>, error: bool) {
        self.message = message
            .into()
            .chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .collect();
        self.error = error;
    }

    fn declared(&self, name: &str) -> bool {
        self.project
            .as_ref()
            .is_some_and(|p| p.manifest.dependencies.contains_key(name))
    }

    fn locked(&self, name: &str) -> bool {
        let dependency = self
            .project
            .as_ref()
            .and_then(|p| p.manifest.dependencies.get(name));
        dependency.is_some_and(|dep| {
            self.lock.as_ref().is_some_and(|lock| {
                serde_json::to_value(dep).ok().as_ref() == lock["dependencies"].get(name)
            })
        })
    }

    fn visible_packages(&self) -> Vec<&Entry> {
        self.entries
            .values()
            .filter(|entry| {
                (self.category == 0 || entry.category == catalog::CATEGORIES[self.category])
                    && match self.filter {
                        1 => entry.default_version().is_some(),
                        2 => entry.default_version().is_none(),
                        3 => !self.declared(&entry.name),
                        _ => true,
                    }
                    && entry.matches(&self.query)
            })
            .collect()
    }

    fn visible_dependencies(&self) -> Vec<(&str, &crate::manifest::Dependency)> {
        self.project
            .as_ref()
            .map(|p| {
                p.manifest
                    .dependencies
                    .iter()
                    .filter(|(name, _)| {
                        self.entries
                            .get(*name)
                            .is_some_and(|e| e.matches(&self.query))
                    })
                    .map(|(name, dep)| (name.as_str(), dep))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn visible_commands(&self) -> Vec<&Request> {
        let query = self.query.to_lowercase();
        self.commands
            .iter()
            .filter(|request| {
                let text = format!("{} {} {}", request.title, request.preview(), request.detail)
                    .to_lowercase();
                query.split_whitespace().all(|word| text.contains(word))
            })
            .collect()
    }

    fn len(&self) -> usize {
        match self.tab {
            0 => 4,
            1 => self.visible_packages().len(),
            2 => self.visible_dependencies().len(),
            _ => self.visible_commands().len(),
        }
    }

    fn clamp_selection(&mut self) {
        self.selected = self.selected.min(self.len().saturating_sub(1));
        self.list.select((self.len() > 0).then_some(self.selected));
    }

    fn reset_selection(&mut self) {
        self.selected = 0;
        self.list = ListState::default();
        self.clamp_selection();
    }

    fn switch_tab(&mut self, tab: usize, options: &InstallOptions) {
        self.tab = tab % TABS.len();
        self.query.clear();
        self.searching = false;
        self.reset_selection();
        if self.tab == 1 && !self.discovery_started {
            self.start_fetch(options);
        }
    }

    fn start_fetch(&mut self, options: &InstallOptions) {
        if self.fetch.is_some() {
            return;
        }
        self.discovery_started = true;
        match Fetch::start(options) {
            Ok(fetch) => {
                self.fetch = Some(fetch);
                self.discovery_status = if options.offline {
                    "Consultando receitas no cache local…"
                } else {
                    "Consultando todos os remotos Conan…"
                }
                .into();
                self.notify(
                    "Você pode continuar navegando enquanto o catálogo carrega.",
                    false,
                );
            }
            Err(error) => {
                self.discovery_status =
                    "Sugestões locais • r tenta consultar o Conan novamente".into();
                self.notify(format!("{error:#}"), true);
            }
        }
    }

    fn poll_fetch(&mut self) -> bool {
        let Some(fetch) = &mut self.fetch else {
            return false;
        };
        match fetch.poll() {
            Ok(None) => return false,
            Ok(Some(discovery)) => {
                let selected = self
                    .visible_packages()
                    .get(self.selected)
                    .map(|e| e.name.clone());
                let count = discovery.entries.len();
                for (name, entry) in discovery.entries {
                    let previous = self
                        .entries
                        .entry(name)
                        .or_insert_with(|| Entry::new(&entry.name));
                    previous.versions = entry.versions;
                    previous.sources = entry.sources;
                }
                self.discovery_status = format!(
                    "{count} receitas descobertas • r atualiza • versões resolvidas ao instalar"
                );
                self.notify(if discovery.warnings.is_empty() {
                    format!("Catálogo atualizado: {count} receitas, além das sugestões locais. Use / para pesquisar.")
                } else { discovery.warnings.join("; ") }, !discovery.warnings.is_empty());
                self.fetch = None;
                if self.tab == 1 {
                    self.selected = selected
                        .and_then(|name| {
                            self.visible_packages().iter().position(|e| e.name == name)
                        })
                        .unwrap_or(0);
                    self.clamp_selection();
                }
            }
            Err(error) => {
                self.fetch = None;
                self.discovery_status =
                    "Consulta indisponível • catálogo anterior preservado • r tenta novamente"
                        .into();
                self.notify(format!("{error:#}"), true);
            }
        }
        true
    }

    fn confirm(&mut self, request: Request, options: &InstallOptions) {
        if request.project && self.project.is_none() {
            self.notify(
                "Abra a interface dentro de um projeto ou crie um na aba Comandos.",
                true,
            );
        } else if options.frozen
            && matches!(
                request.args.first().map(String::as_str),
                Some("add" | "remove")
            )
        {
            self.notify("O lockfile está congelado. Reabra sem --frozen-lockfile para alterar dependências.", true);
        } else {
            self.modal_scroll = 0;
            self.modal = Some(Modal::Confirm(request));
        }
    }

    fn install(&mut self, options: &InstallOptions) {
        if self
            .project
            .as_ref()
            .is_some_and(|p| !p.manifest.package.target.is_native())
        {
            self.notify("Este projeto usa WebAssembly; dependências Conan nativas ainda não são suportadas.", true);
            return;
        }
        let specs = if self.basket.is_empty() {
            let Some(entry) = self.visible_packages().get(self.selected).copied() else {
                return;
            };
            if self.declared(&entry.name) {
                self.notify(
                    "Já está no manifesto. Use a aba Dependências ou a para informar outra versão.",
                    false,
                );
                return;
            }
            vec![entry.name.clone()]
        } else {
            self.basket.values().cloned().collect()
        };
        let mut request = Request::new(
            "Instalar bibliotecas",
            &["add"],
            "Resolve o conjunto e instala em uma operação. O manifesto e o lockfile são gravados após a instalação das dependências.",
            true,
        );
        request.args.extend(specs);
        self.confirm(request, options);
    }

    fn add_specs(&mut self, text: &str) -> Result<()> {
        ensure!(
            !text.trim().is_empty(),
            "Informe um pacote, como fmt, sdl3 ou pacote@versão."
        );
        let mut additions = BTreeMap::new();
        for spec in text.split_whitespace() {
            let (name, version) = packages::parse_spec(spec)?;
            let spec = version.map_or_else(|| name.clone(), |v| format!("{name}@{v}"));
            if let Some(previous) = additions.insert(name, spec.clone()) {
                ensure!(previous == spec, "Versões conflitantes na seleção.");
            }
        }
        for name in additions.keys() {
            self.entries
                .entry(name.clone())
                .or_insert_with(|| Entry::new(name));
        }
        self.basket.extend(additions);
        self.notify(
            format!(
                "{} pacote(s) na seleção. Enter instala; x limpa a seleção.",
                self.basket.len()
            ),
            false,
        );
        Ok(())
    }

    /// Returns an action only after the user accepts its command preview.
    fn key(&mut self, key: KeyEvent, options: &InstallOptions) -> (bool, Option<Request>) {
        if key.kind == KeyEventKind::Release {
            return (false, None);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return (true, None);
        }
        if let Some(modal) = self.modal.take() {
            match modal {
                Modal::Help => {
                    match key.code {
                        KeyCode::Down | KeyCode::PageDown => {
                            self.modal_scroll = (self.modal_scroll + 1).min(18)
                        }
                        KeyCode::Up | KeyCode::PageUp => {
                            self.modal_scroll = self.modal_scroll.saturating_sub(1)
                        }
                        _ => {}
                    }
                    if !matches!(
                        key.code,
                        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('?' | 'q')
                    ) {
                        self.modal = Some(Modal::Help);
                    }
                }
                Modal::Confirm(request) => match key.code {
                    KeyCode::Enter | KeyCode::Char('y' | 'Y') => return (false, Some(request)),
                    KeyCode::Esc | KeyCode::Char('n' | 'N' | 'q') => {}
                    KeyCode::Down | KeyCode::PageDown => {
                        self.modal_scroll = self.modal_scroll.saturating_add(1);
                        self.modal = Some(Modal::Confirm(request));
                    }
                    KeyCode::Up | KeyCode::PageUp => {
                        self.modal_scroll = self.modal_scroll.saturating_sub(1);
                        self.modal = Some(Modal::Confirm(request));
                    }
                    _ => self.modal = Some(Modal::Confirm(request)),
                },
                Modal::Add(mut text) => {
                    match key.code {
                        KeyCode::Esc => return (false, None),
                        KeyCode::Enter => match self.add_specs(&text) {
                            Ok(()) => return (false, None),
                            Err(error) => self.notify(format!("{error:#}"), true),
                        },
                        KeyCode::Backspace => {
                            text.pop();
                        }
                        KeyCode::Char(c)
                            if !key
                                .modifiers
                                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                                && text.len() < 512 =>
                        {
                            text.push(c)
                        }
                        _ => {}
                    }
                    self.modal = Some(Modal::Add(text));
                }
            }
            return (false, None);
        }
        if self.searching {
            match key.code {
                KeyCode::Enter | KeyCode::Esc => self.searching = false,
                KeyCode::Backspace => {
                    self.query.pop();
                    self.reset_selection();
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.query.clear();
                    self.reset_selection();
                }
                KeyCode::Char(c)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                        && self.query.len() < 256 =>
                {
                    self.query.push(c);
                    self.reset_selection();
                }
                KeyCode::Down => {
                    self.selected = (self.selected + 1).min(self.len().saturating_sub(1));
                }
                KeyCode::Up => self.selected = self.selected.saturating_sub(1),
                _ => {}
            }
            self.clamp_selection();
            return (false, None);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            if key.code == KeyCode::Char('p') {
                self.switch_tab(3, options);
                self.searching = true;
            }
            return (false, None);
        }
        match key.code {
            KeyCode::Char('q') => return (true, None),
            KeyCode::Char('?') | KeyCode::F(1) => { self.modal_scroll = 0; self.modal = Some(Modal::Help); }
            KeyCode::Tab | KeyCode::Right => self.switch_tab(self.tab + 1, options),
            KeyCode::BackTab | KeyCode::Left => self.switch_tab((self.tab + TABS.len() - 1) % TABS.len(), options),
            KeyCode::Char(c @ '1'..='4') => self.switch_tab((c as u8 - b'1') as usize, options),
            KeyCode::Char('/') => {
                if self.tab == 0 { self.switch_tab(3, options); }
                self.searching = true;
            }
            KeyCode::Esc => { self.query.clear(); self.reset_selection(); }
            KeyCode::Down | KeyCode::Char('j') => { self.selected = (self.selected + 1).min(self.len().saturating_sub(1)); }
            KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
            KeyCode::PageDown => self.selected = (self.selected + 10).min(self.len().saturating_sub(1)),
            KeyCode::PageUp => self.selected = self.selected.saturating_sub(10),
            KeyCode::Home => self.selected = 0,
            KeyCode::End => self.selected = self.len().saturating_sub(1),
            KeyCode::Char('c') if self.tab == 1 => { self.category = (self.category + 1) % catalog::CATEGORIES.len(); self.reset_selection(); }
            KeyCode::Char('f') if self.tab == 1 => { self.filter = (self.filter + 1) % 4; self.reset_selection(); }
            KeyCode::Char('a') if self.tab == 1 => self.modal = Some(Modal::Add(String::new())),
            KeyCode::Char('x') if self.tab == 1 => { self.basket.clear(); self.notify("Seleção limpa.", false); }
            KeyCode::Char(' ') if self.tab == 1 => {
                if let Some(name) = self.visible_packages().get(self.selected).map(|p| p.name.clone())
                    && self.basket.remove(&name).is_none() {
                        if self.declared(&name) {
                            self.notify("Já declarada no projeto. Use a para escolher outra versão.", false);
                        } else { self.basket.insert(name.clone(), name); }
                }
            }
            KeyCode::Char('r') if self.tab == 1 => self.start_fetch(options),
            KeyCode::Char('r') => match self.reload() {
                Ok(()) => self.notify("Projeto atualizado a partir do disco.", false),
                Err(error) => self.notify(format!("{error:#}"), true),
            },
            KeyCode::Char('d') if self.tab == 2 => {
                if let Some((name, _)) = self.visible_dependencies().get(self.selected) {
                    let request = Request::new("Remover dependência", &["remove", name], "Remove a dependência direta do manifesto e atualiza o lockfile.", true);
                    self.confirm(request, options);
                }
            }
            KeyCode::Enter | KeyCode::Char('i') => match self.tab {
                0 => match self.selected {
                    0 => self.switch_tab(1, options),
                    1 => self.switch_tab(2, options),
                    2 => self.switch_tab(3, options),
                    _ => self.confirm(Request::new("Diagnosticar ambiente", &["doctor"], "Verifica as ferramentas e compila e executa um pequeno programa C++.", false), options),
                },
                1 => self.install(options),
                2 => self.confirm(Request::new("Sincronizar dependências", &["install"], "Instala as dependências do manifesto e atualiza o lockfile e a integração CMake.", true), options),
                _ => {
                    if let Some(request) = self.visible_commands().get(self.selected).cloned().cloned() { self.confirm(request, options); }
                }
            },
            _ => {}
        }
        self.clamp_selection();
        (false, None)
    }
}

fn quick_commands(project: Option<&Project>) -> Vec<Request> {
    let mut requests = Vec::new();
    if let Some(project) = project {
        requests.extend(project.manifest.scripts.iter().map(|(name, body)| {
            Request::new(
                &format!("Script: {name}"),
                &["run", name],
                &body.to_string(),
                true,
            )
        }));
    } else {
        requests.push(Request::new("Criar projeto nesta pasta", &["init", "."], "Cria um aplicativo C++ com CMake, scripts e teste. Arquivos existentes são preservados.", false));
    }
    requests.extend([
        Request::new(
            "Compilar · Debug",
            &["build"],
            "Compila o projeto para desenvolvimento.",
            true,
        ),
        Request::new(
            "Compilar · Release",
            &["build", "--release"],
            "Compila com otimizações para distribuição.",
            true,
        ),
        Request::new(
            "Compilar e executar",
            &["exec"],
            "Compila e executa a aplicação com o ambiente de suas bibliotecas.",
            true,
        ),
        Request::new(
            "Executar testes",
            &["test"],
            "Compila e executa os testes com CTest.",
            true,
        ),
        Request::new(
            "Sincronizar dependências",
            &["install"],
            "Instala as dependências declaradas e atualiza o lockfile.",
            true,
        ),
        Request::new(
            "Configurar CMake e editor",
            &["configure"],
            "Atualiza o CMake e compile_commands.json para o editor.",
            true,
        ),
        Request::new(
            "Reconfigurar do zero",
            &["configure", "--fresh"],
            "Revalida as dependências e recria o cache CMake do projeto.",
            true,
        ),
        Request::new(
            "Simular limpeza dos builds",
            &["clean", "--dry-run"],
            "Mostra o espaço dos builds que podem ser removidos; preserva fontes e dependências.",
            true,
        ),
        Request::new(
            "Preparar VS Code",
            &["vscode"],
            "Cria tarefas para compilar e executar pelo VS Code.",
            true,
        ),
        Request::new(
            "Diagnosticar ambiente",
            &["doctor"],
            "Verifica ferramentas e compila e executa um teste para o target do projeto.",
            false,
        ),
        Request::new(
            "Ver armazenamento",
            &["store", "status"],
            "Mostra receitas, binários e espaço compartilhado entre projetos.",
            false,
        ),
        Request::new(
            "Ver projetos registrados",
            &["store", "projects"],
            "Lista projetos que usam o armazenamento compartilhado.",
            false,
        ),
        Request::new(
            "Simular limpeza",
            &["store", "prune", "--dry-run"],
            "Mostra bibliotecas sem uso e sobras que podem ser removidas.",
            false,
        ),
        Request::new(
            "Compartilhar arquivos idênticos",
            &["store", "dedupe"],
            "Economiza espaço compartilhando arquivos idênticos por hard links.",
            false,
        ),
    ]);
    for (script, label) in [
        ("dev", "Acompanhar desenvolvimento"),
        ("test", "Acompanhar testes"),
    ] {
        if project.is_some_and(|project| project.manifest.scripts.contains_key(script)) {
            requests.push(Request::new(
                label,
                &["watch", script],
                "Reinicia o script ao salvar fontes ou configuração. Ctrl+C encerra os processos.",
                true,
            ));
        }
    }
    if project.is_none_or(|p| p.manifest.package.target.is_native()) {
        requests.push(Request::new(
            "Compilar · Protegido (obf)",
            &["build", "obf"],
            "Ofuscador LLVM incluído: strings, fluxo de controle, substituição de instruções e divisão de blocos. Requer Clang e o SDK LLVM correspondente; prepara o plugin automaticamente.",
            true,
        ));
        requests.push(Request::new(
            "Compilar · Reduzir metadados",
            &["build", "obf", "--obf-basic"],
            "Release com LTO, símbolos reduzidos e remapeamento de caminhos. Dispensa o SDK LLVM; não aplica passes de ofuscação.",
            true,
        ));
    }
    if project.is_some_and(|p| !p.manifest.package.target.is_native()) {
        requests.push(Request::new(
            "Servir WebAssembly",
            &["serve"],
            "Compila e inicia o servidor de desenvolvimento em localhost:8080.",
            true,
        ));
    }
    requests
}

/// Each action reuses the CLI, including its validation, locks and exit handling.
fn action_command(request: &Request, cwd: &Path, options: &InstallOptions) -> Result<Command> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .current_dir(cwd)
        .arg("--profile")
        .arg(&options.profile);
    if let Some(store) = &options.store_dir {
        command.arg("--store-dir").arg(store);
    }
    if options.offline {
        command.arg("--offline");
    }
    if options.frozen {
        command.arg("--frozen-lockfile");
    }
    if options.verbose {
        command.arg("--verbose");
    }
    command.args(&request.args);
    Ok(command)
}

struct Session {
    terminal: DefaultTerminal,
}

impl Session {
    fn start() -> Result<Self> {
        match ratatui::try_init() {
            Ok(terminal) => Ok(Self { terminal }),
            Err(error) => {
                ratatui::restore();
                Err(error.into())
            }
        }
    }

    fn execute(&mut self, request: &Request, cwd: &Path, options: &InstallOptions) -> Result<bool> {
        let mut command = action_command(request, cwd, options)?;
        ratatui::restore();
        println!("\n  > {}\n", request.preview());
        let result = command
            .status()
            .context("não foi possível iniciar o comando");
        match &result {
            Ok(status) if status.success() => println!("\n  Concluído."),
            Ok(status) => println!(
                "\n  O comando terminou com status {}.",
                status.code().unwrap_or(130)
            ),
            Err(error) => println!("\n  {error:#}"),
        }
        print!("  Pressione Enter para voltar ao cppm…");
        io::stdout().flush()?;
        io::stdin().read_line(&mut String::new())?;
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        // Rebuild the fullscreen buffers without querying the cursor: some
        // terminals do not answer cursor-position reports after a subprocess.
        self.terminal.resize(self.terminal.size()?.into())?;
        Ok(result.is_ok_and(|status| status.success()))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

pub fn run(cwd: &Path, options: &InstallOptions) -> Result<()> {
    ensure!(
        io::stdin().is_terminal() && io::stdout().is_terminal(),
        "a interface interativa precisa de um terminal; execute `cppm ui` diretamente no terminal (sem pipes)"
    );
    ensure!(
        std::env::var("TERM").as_deref() != Ok("dumb"),
        "este terminal não suporta a interface; use um terminal com suporte a cores e cursor"
    );
    let mut app = App::new(cwd)?;
    let mut session = Session::start()?;
    let mut redraw = true;
    loop {
        // Subprocesses can consume a resize notification while the UI is
        // suspended. Check the actual dimensions as well as terminal events.
        let size = session.terminal.size()?;
        if session.terminal.get_frame().area().as_size() != size {
            session.terminal.resize(size.into())?;
            redraw = true;
        }
        redraw |= app.poll_fetch();
        if redraw {
            session
                .terminal
                .draw(|frame| view::draw(frame, &mut app, options))?;
            redraw = false;
        }
        if !event::poll(Duration::from_millis(120))? {
            continue;
        }
        let event = event::read()?;
        if matches!(event, Event::Resize(_, _)) {
            redraw = true;
        }
        if let Event::Key(key) = event {
            redraw = true;
            let (quit, request) = app.key(key, options);
            if quit {
                break;
            }
            if let Some(request) = request {
                // No catalog child should remain alive while a command mutates Conan's cache.
                if app.fetch.take().is_some() {
                    app.discovery_status =
                        "Consulta interrompida para executar a ação • r atualiza o catálogo".into();
                }
                let success = session.execute(&request, &app.cwd, options)?;
                if success && request.args.first().is_some_and(|s| s == "add") {
                    app.basket.clear();
                }
                app.notify(
                    if success {
                        format!("Concluído: {}", request.preview())
                    } else {
                        format!(
                            "Falhou: {}. A saída do comando foi exibida no terminal.",
                            request.preview()
                        )
                    },
                    !success,
                );
                if let Err(error) = app.reload() {
                    app.notify(format!("{error:#}"), true);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
