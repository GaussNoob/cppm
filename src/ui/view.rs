use super::{App, Modal, Request, TABS, catalog};
use crate::{conan::InstallOptions, packages};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Clear, List, ListItem, Padding, Paragraph, Tabs, Wrap},
};

const BG: Color = Color::Rgb(12, 17, 29);
const PANEL: Color = Color::Rgb(17, 25, 40);
const BORDER: Color = Color::Rgb(43, 58, 80);
const FG: Color = Color::Rgb(222, 231, 243);
const MUTED: Color = Color::Rgb(139, 157, 182);
const CYAN: Color = Color::Rgb(93, 218, 235);
const GREEN: Color = Color::Rgb(122, 226, 183);
const PURPLE: Color = Color::Rgb(182, 163, 246);
const YELLOW: Color = Color::Rgb(245, 200, 114);
const RED: Color = Color::Rgb(255, 139, 151);
const SELECTED: Color = Color::Rgb(29, 48, 70);

fn panel(title: impl Into<Line<'static>>) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(BORDER))
        .style(Style::new().bg(PANEL).fg(FG))
        .title(title)
        .padding(Padding::horizontal(1))
}

fn text(value: impl Into<String>, color: Color) -> Span<'static> {
    Span::styled(value.into(), Style::new().fg(color))
}

fn heading(title: &str, subtitle: &str) -> Text<'static> {
    Text::from(vec![
        Line::from(text(title, FG).bold()),
        Line::from(text(subtitle, MUTED)),
    ])
}

pub(super) fn draw(frame: &mut Frame, app: &mut App, options: &InstallOptions) {
    let area = frame.area();
    frame.render_widget(Block::new().style(Style::new().bg(BG).fg(FG)), area);
    if area.width < 52 || area.height < 19 {
        frame.render_widget(
            Paragraph::new(
                "cppm\n\nAmplie o terminal para pelo menos 52 × 19.\n\nq ou Ctrl+C para sair.",
            )
            .style(Style::new().fg(CYAN))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
            area.inner(Margin::new(1, 1)),
        );
        return;
    }
    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Min(6),
        Constraint::Length(3),
        Constraint::Length(1),
    ])
    .margin(1)
    .split(area);
    let header = Layout::horizontal([Constraint::Length(22), Constraint::Min(15)]).split(chunks[0]);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            text(" ◈ cppm ", CYAN).bold(),
            text(format!("v{}", env!("CARGO_PKG_VERSION")), MUTED),
        ])),
        header[0],
    );
    let context = app.project.as_ref().map_or_else(
        || "nenhum projeto aberto".into(),
        |p| {
            format!(
                "{}  /  C++{}  /  {}",
                p.manifest.package.name,
                p.manifest.package.cpp_standard,
                if p.manifest.package.target.is_native() {
                    "nativo"
                } else {
                    "wasm"
                }
            )
        },
    );
    frame.render_widget(
        Paragraph::new(text(context, MUTED)).alignment(Alignment::Right),
        header[1],
    );
    let labels: Vec<Line> = TABS
        .iter()
        .enumerate()
        .map(|(i, name)| {
            Line::from(format!(
                "{} {}",
                i + 1,
                if area.width < 76 {
                    ["Início", "Explorar", "Deps", "Comandos"][i]
                } else {
                    name
                }
            ))
        })
        .collect();
    frame.render_widget(
        Tabs::new(labels)
            .select(app.tab)
            .divider(" ")
            .style(Style::new().fg(MUTED))
            .highlight_style(
                Style::new()
                    .fg(CYAN)
                    .bg(SELECTED)
                    .add_modifier(Modifier::BOLD),
            )
            .block(
                Block::bordered()
                    .borders(ratatui::widgets::Borders::BOTTOM)
                    .border_style(Style::new().fg(BORDER)),
            ),
        chunks[1],
    );
    match app.tab {
        0 => overview(frame, app, chunks[2], options),
        1 => explore(frame, app, chunks[2]),
        2 => dependencies(frame, app, chunks[2]),
        _ => commands(frame, app, chunks[2]),
    }
    frame.render_widget(
        Paragraph::new(app.message.as_str())
            .wrap(Wrap { trim: true })
            .style(Style::new().fg(if app.error { RED } else { MUTED }))
            .block(Block::new().padding(Padding::top(1))),
        chunks[3],
    );
    let hint = if app.searching {
        " Digite para filtrar   Enter concluir   Esc voltar   Ctrl+U limpar"
    } else {
        match app.tab {
            1 => {
                " / buscar  ↑↓ navegar  Espaço selecionar  Enter instalar  a pacote  ? ajuda  q sair"
            }
            2 => " / buscar  ↑↓ navegar  Enter sincronizar  d remover  Tab abas  ? ajuda  q sair",
            3 => " / buscar  ↑↓ navegar  Enter executar  Tab abas  ? ajuda  q sair",
            _ => " Tab / 1–4 abas   ↑↓ navegar   Enter abrir   Ctrl+P comandos   ? ajuda   q sair",
        }
    };
    frame.render_widget(Paragraph::new(hint).style(Style::new().fg(CYAN)), chunks[4]);
    if let Some(modal) = &app.modal {
        modal_view(frame, modal, app.modal_scroll, options);
    }
}

fn render_list(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    title: String,
    items: Vec<ListItem<'static>>,
) {
    if items.is_empty() {
        frame.render_widget(
            Paragraph::new(
                "\nNenhum resultado.\n\nUse Esc para limpar a busca ou altere os filtros.",
            )
            .wrap(Wrap { trim: true })
            .style(Style::new().fg(MUTED))
            .block(panel(title)),
            area,
        );
    } else {
        frame.render_stateful_widget(
            List::new(items)
                .block(panel(title))
                .highlight_style(Style::new().bg(SELECTED).fg(FG))
                .highlight_symbol("▌ "),
            area,
            &mut app.list,
        );
    }
}

fn overview(frame: &mut Frame, app: &mut App, area: Rect, options: &InstallOptions) {
    let compact = area.height < 20;
    let chunks = Layout::vertical([
        Constraint::Length(if compact { 2 } else { 3 }),
        Constraint::Length(if compact { 4 } else { 5 }),
        Constraint::Length(if compact { 0 } else { 1 }),
        Constraint::Min(3),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(heading(
            "Seu próximo projeto começa aqui.",
            "Descubra bibliotecas, cuide das dependências e execute seus comandos.",
        )),
        chunks[0],
    );
    let stats = Layout::horizontal([
        Constraint::Percentage(34),
        Constraint::Percentage(33),
        Constraint::Percentage(33),
    ])
    .spacing(1)
    .split(chunks[1]);
    let deps = app
        .project
        .as_ref()
        .map_or(0, |p| p.manifest.dependencies.len());
    let scripts = app.project.as_ref().map_or(0, |p| p.manifest.scripts.len());
    for (rect, label, value, subtitle, color) in [
        (
            stats[0],
            " BIBLIOTECAS ",
            app.entries.len(),
            "para explorar",
            CYAN,
        ),
        (stats[1], " DEPENDÊNCIAS ", deps, "no manifesto", GREEN),
        (
            stats[2],
            " SCRIPTS ",
            scripts,
            "prontos para executar",
            PURPLE,
        ),
    ] {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(text(value.to_string(), color).bold()),
                Line::from(text(subtitle, MUTED)),
            ])
            .block(panel(label)),
            rect,
        );
    }
    let panes = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .spacing(1)
        .split(chunks[3]);
    let items = [
        (
            "Explorar bibliotecas",
            "Catálogo, categorias e pesquisa no Conan",
        ),
        (
            "Cuidar das dependências",
            "Manifesto, versões e sincronização",
        ),
        (
            "Abrir paleta de comandos",
            "Scripts, builds, testes e armazenamento",
        ),
        (
            "Diagnosticar o ambiente",
            "Verifique ferramentas e compilação C++",
        ),
    ]
    .into_iter()
    .map(|(title, sub)| {
        if panes[0].height < 14 {
            ListItem::new(Line::from(text(title, FG)))
        } else {
            ListItem::new(vec![
                Line::from(text(title, FG)),
                Line::from(text(sub, MUTED)),
                Line::from(""),
            ])
        }
    })
    .collect();
    render_list(frame, app, panes[0], " COMECE POR AQUI ".into(), items);
    let mut lines = vec![];
    if let Some(project) = &app.project {
        lines.extend([
            Line::from(text(&project.manifest.package.name, GREEN).bold()),
            Line::from(text(project.root.display().to_string(), MUTED)),
            Line::from(""),
            Line::from(vec![
                text("Lockfile  ", MUTED),
                text(
                    if app.lock.is_some() {
                        "presente"
                    } else {
                        "ausente ou inválido"
                    },
                    if app.lock.is_some() { GREEN } else { YELLOW },
                ),
            ]),
        ]);
    } else {
        lines.extend([
            Line::from(text("Vamos criar algo?", GREEN).bold()),
            Line::from(text(
                "Abra Comandos para criar um projeto nesta pasta, ou use cppm -C pasta ui.",
                MUTED,
            )),
            Line::from(""),
            Line::from(text(app.cwd.display().to_string(), MUTED)),
        ]);
    }
    lines.extend([
        Line::from(""),
        Line::from(vec![text("Perfil    ", MUTED), text(&options.profile, FG)]),
        Line::from(vec![
            text("Rede      ", MUTED),
            text(
                if options.offline {
                    "offline · cache local"
                } else {
                    "remotos habilitados"
                },
                FG,
            ),
        ]),
        Line::from(vec![
            text("Lockfile  ", MUTED),
            text(
                if options.frozen {
                    "congelado"
                } else {
                    "atualizável"
                },
                FG,
            ),
        ]),
    ]);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(panel(" SEU WORKSPACE ")),
        panes[1],
    );
}

fn search(frame: &mut Frame, app: &App, area: Rect, placeholder: &str) {
    let content = if app.query.is_empty() && !app.searching {
        placeholder.to_string()
    } else {
        format!("{}{}", app.query, if app.searching { "▏" } else { "" })
    };
    frame.render_widget(
        Paragraph::new(text(
            content,
            if app.query.is_empty() && !app.searching {
                MUTED
            } else {
                FG
            },
        ))
        .block(
            panel(if app.searching {
                " BUSCAR · Enter conclui "
            } else {
                " / BUSCAR "
            })
            .border_style(Style::new().fg(if app.searching { CYAN } else { BORDER })),
        ),
        area,
    );
}

fn content_panes(area: Rect) -> (Rect, Option<Rect>) {
    if area.width >= 92 {
        let panes = Layout::horizontal([Constraint::Percentage(56), Constraint::Percentage(44)])
            .spacing(1)
            .split(area);
        (panes[0], Some(panes[1]))
    } else if area.height >= 19 {
        let panes = Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)])
            .spacing(1)
            .split(area);
        (panes[0], Some(panes[1]))
    } else {
        (area, None)
    }
}

fn explore(frame: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Min(3),
        Constraint::Length(2),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(heading(
            "Encontre sua próxima biblioteca.",
            &app.discovery_status,
        )),
        chunks[0],
    );
    search(
        frame,
        app,
        chunks[1],
        "Nome, alias ou assunto: json, postgres, jogos, imagens…",
    );
    let filter = [
        "Todas",
        "Com padrão cppm",
        "Sem padrão cppm",
        "Fora do projeto",
    ][app.filter];
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            text(" c categoria: ", MUTED),
            text(catalog::CATEGORIES[app.category], CYAN),
            text("    f filtro: ", MUTED),
            text(filter, PURPLE),
            text(
                if app.fetch.is_some() {
                    "    ◌ carregando"
                } else {
                    "    r atualizar"
                },
                MUTED,
            ),
        ])),
        chunks[2],
    );
    let (list_area, details_area) = content_panes(chunks[3]);
    let entries = app.visible_packages();
    let count = entries.len();
    let items = entries
        .iter()
        .map(|entry| {
            let chosen = app.basket.contains_key(&entry.name);
            let declared = app.declared(&entry.name);
            ListItem::new(Line::from(vec![
                text(
                    if chosen { "[✓] " } else { "[ ] " },
                    if chosen { GREEN } else { MUTED },
                ),
                text(format!("{:<24} ", entry.name), FG),
                text(
                    if declared {
                        "no projeto"
                    } else if entry.default_version().is_some() {
                        "padrão cppm"
                    } else {
                        "Conan"
                    },
                    if declared { GREEN } else { MUTED },
                ),
            ]))
        })
        .collect();
    if let Some(rect) = details_area {
        let lines = entries.get(app.selected).map(|entry| {
            let mut lines = vec![
                Line::from(text(&entry.name, CYAN).bold()),
                Line::from(text(entry.category, PURPLE)), Line::from(""),
                Line::from(text(entry.description, FG)), Line::from(""),
            ];
            if let Some(version) = entry.default_version() {
                lines.push(Line::from(vec![text("Padrão cppm  ", MUTED), text(version, GREEN)]));
            } else {
                lines.push(Line::from(text("Sem versão padrão no cppm. O Conan resolve a versão e os metadados CMake ao instalar.", MUTED)));
            }
            let aliases = packages::ALIASES.iter().filter(|(_, name)| *name == entry.name).map(|(alias, _)| *alias).collect::<Vec<_>>();
            if !aliases.is_empty() { lines.push(Line::from(vec![text("Aliases  ", MUTED), text(aliases.join(", "), FG)])); }
            if !entry.versions.is_empty() {
                lines.push(Line::from(text(format!("{} versão(ões) · {}", entry.versions.len(), entry.sources.iter().cloned().collect::<Vec<_>>().join(", ")), MUTED)));
                let versions = entry.versions.iter().take(8).cloned().collect::<Vec<_>>().join(", ");
                lines.push(Line::from(text(format!("Amostra: {versions}"), MUTED)));
            }
            lines.extend([
                Line::from(""), Line::from(text(format!("cppm add {}", entry.name), GREEN)),
                Line::from(text("Espaço seleciona · Enter instala", MUTED)),
                Line::from(text("a informa nome@versão ou qualquer receita", MUTED)),
            ]);
            lines
        }).unwrap_or_else(|| vec![Line::from(text("Tente outro nome, alias ou categoria. Use a para adicionar um pacote por nome.", MUTED))]);
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: true })
                .block(panel(" SOBRE A BIBLIOTECA ")),
            rect,
        );
    }
    render_list(
        frame,
        app,
        list_area,
        format!(" BIBLIOTECAS · {count} "),
        items,
    );
    let basket = if app.basket.is_empty() {
        if details_area.is_none() {
            app.visible_packages().get(app.selected).map_or_else(
                || "a adiciona qualquer receita por nome ou versão.".into(),
                |entry| format!("{}: {}", entry.name, entry.description),
            )
        } else {
            "Selecione com Espaço para instalar várias bibliotecas juntas. a adiciona por nome/versão.".into()
        }
    } else {
        format!(
            "Seleção ({}): {}  ·  Enter instala · x limpa",
            app.basket.len(),
            app.basket.values().cloned().collect::<Vec<_>>().join(", ")
        )
    };
    frame.render_widget(
        Paragraph::new(text(basket, GREEN)).wrap(Wrap { trim: true }),
        chunks[4],
    );
}

fn dependencies(frame: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(3),
    ])
    .split(area);
    frame.render_widget(Paragraph::new(heading("O que seu projeto usa.", "Dependências diretas do cppm.toml. O lockfile registra a resolução; binários variam por perfil e configuração."))
        .wrap(Wrap { trim: true }), chunks[0]);
    search(frame, app, chunks[1], "Filtrar dependências do projeto…");
    if app
        .project
        .as_ref()
        .is_none_or(|p| p.manifest.dependencies.is_empty())
    {
        frame.render_widget(Paragraph::new(vec![
            Line::from(""), Line::from(text(if app.project.is_none() { "Nenhum projeto aberto." } else { "Este projeto ainda não tem dependências externas." }, FG)),
            Line::from(""), Line::from(text("Explore o catálogo na aba 2. Selecione bibliotecas com Espaço e instale com Enter.", MUTED)),
        ]).wrap(Wrap { trim: true }).block(panel(" DEPENDÊNCIAS ")), chunks[2]);
        return;
    }
    let (list_area, details_area) = content_panes(chunks[2]);
    let deps = app.visible_dependencies();
    let count = deps.len();
    let items = deps
        .iter()
        .map(|(name, dep)| {
            if list_area.height < 6 {
                return ListItem::new(Line::from(vec![
                    text(format!("{name}  "), FG),
                    text(dep.version(), CYAN),
                ]));
            }
            ListItem::new(vec![
                Line::from(vec![
                    text(format!("{name}  "), FG),
                    text(dep.version(), CYAN),
                ]),
                Line::from(text(
                    if app.locked(name) {
                        "No lockfile"
                    } else {
                        "Pendente de sincronização"
                    },
                    if app.locked(name) { GREEN } else { YELLOW },
                )),
                Line::from(""),
            ])
        })
        .collect();
    if let Some(rect) = details_area {
        let lines = deps.get(app.selected).map(|(name, dep)| {
            let mut lines = vec![
                Line::from(text(*name, CYAN).bold()),
                Line::from(text(format!("Versão declarada: {}", dep.version()), FG)),
                Line::from(""), Line::from(text(if app.locked(name) {
                    "A declaração corresponde ao lockfile. Use sincronizar para preparar as bibliotecas neste ambiente."
                } else { "A declaração não corresponde ao lockfile, ou o lockfile está ausente/inválido. Enter sincroniza as dependências." }, MUTED)),
                Line::from(""),
            ];
            if let crate::manifest::Dependency::Detailed(details) = dep {
                lines.push(Line::from(text(format!("CMake: {}", details.cmake_package), PURPLE)));
                lines.push(Line::from(text(details.targets.join(", "), MUTED)));
            } else if let Some(package) = packages::CATALOG.iter().find(|p| p.name == *name) {
                lines.push(Line::from(text(format!("CMake padrão: {}", package.cmake_package), PURPLE)));
                lines.push(Line::from(text(package.target, MUTED)));
            }
            lines.extend([
                Line::from(""), Line::from(text("Enter  sincronizar o manifesto", GREEN)),
                Line::from(text("d      remover esta dependência", YELLOW)),
                Line::from(text("r      reler arquivos do projeto", MUTED)),
            ]);
            lines
        }).unwrap_or_default();
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: true })
                .block(panel(" CONFIGURAÇÃO ")),
            rect,
        );
    }
    render_list(
        frame,
        app,
        list_area,
        format!(" NO PROJETO · {count} "),
        items,
    );
}

fn commands(frame: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Min(3),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(heading(
            "Menos memória. Mais código.",
            "Scripts do projeto e ações prontas. Escolha uma ação para revisar o comando.",
        )),
        chunks[0],
    );
    search(
        frame,
        app,
        chunks[1],
        "Procurar: dev, release, testes, editor, armazenamento…",
    );
    let (list_area, details_area) = content_panes(chunks[2]);
    let commands = app.visible_commands();
    let count = commands.len();
    let items = commands
        .iter()
        .map(|request| {
            if list_area.height < 6 {
                return ListItem::new(Line::from(text(&request.title, FG)));
            }
            ListItem::new(vec![
                Line::from(text(
                    &request.title,
                    if request.project && app.project.is_none() {
                        MUTED
                    } else {
                        FG
                    },
                )),
                Line::from(text(request.preview(), CYAN)),
                Line::from(""),
            ])
        })
        .collect();
    if let Some(rect) = details_area {
        let lines = commands.get(app.selected).map(|request| vec![
            Line::from(text(&request.title, PURPLE).bold()), Line::from(""),
            Line::from(text(request.preview(), GREEN)), Line::from(""),
            Line::from(text(&request.detail, FG)), Line::from(""),
            Line::from(text(if request.project && app.project.is_none() { "Requer um projeto. Crie um nesta pasta ou abra cppm -C pasta ui." } else { "Enter abre a prévia antes de executar. A saída aparece no terminal; Enter retorna à interface." }, MUTED)),
        ]).unwrap_or_default();
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: true })
                .block(panel(" PRÉVIA DA AÇÃO ")),
            rect,
        );
    }
    render_list(frame, app, list_area, format!(" AÇÕES · {count} "), items);
}

fn popup(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(4));
    let height = height.min(area.height.saturating_sub(2));
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}

fn modal_view(frame: &mut Frame, modal: &Modal, scroll: u16, options: &InstallOptions) {
    let (title, lines, height) = match modal {
        Modal::Help => (
            " ATALHOS ",
            vec![
                Line::from(text("Navegação", CYAN).bold()),
                Line::from("Tab / Shift+Tab / ← →  mudar de aba"),
                Line::from("1–4  abrir aba     ↑↓ / j k  navegar"),
                Line::from("PgUp / PgDn  página     Home / End  início/fim"),
                Line::from("/  buscar     Esc  limpar busca     Ctrl+P  comandos"),
                Line::from(""),
                Line::from(text("Catálogo", CYAN).bold()),
                Line::from("c  categoria     f  filtro     r  atualizar Conan"),
                Line::from("Espaço  selecionar     x  limpar seleção"),
                Line::from("a  adicionar por nome ou nome@versão"),
                Line::from("Enter / i  instalar seleção ou biblioteca atual"),
                Line::from(""),
                Line::from(text("Dependências e ações", CYAN).bold()),
                Line::from("Enter  sincronizar / executar     d  remover dependência"),
                Line::from("r  reler projeto     q / Ctrl+C  sair"),
                Line::from(""),
                Line::from(text("Enter ou Esc fecha esta ajuda.", GREEN)),
            ],
            21,
        ),
        Modal::Add(value) => (
            " ADICIONAR À SELEÇÃO ",
            vec![
                Line::from(text("Qual biblioteca você quer usar?", CYAN).bold()),
                Line::from(""),
                Line::from(text(format!("{value}▏"), FG)),
                Line::from(""),
                Line::from(text("Exemplos: fmt json sdl3  ou  pacote@1.2.3", MUTED)),
                Line::from(text(
                    "Aceita receitas fora do catálogo e uma versão explícita.",
                    MUTED,
                )),
                Line::from(""),
                Line::from(text("Enter adiciona à seleção · Esc cancela", GREEN)),
            ],
            12,
        ),
        Modal::Confirm(request) => confirmation(request, options),
    };
    let rect = popup(frame.area(), 80, height);
    frame.render_widget(Clear, rect);
    let block = panel(title)
        .border_style(Style::new().fg(CYAN))
        .padding(Padding::new(2, 2, 1, 1));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);
    let footer = match modal {
        Modal::Confirm(_) => "Enter / y executa · Esc / n cancela · ↑↓ rolar",
        Modal::Help => "Enter / Esc fecha · ↑↓ rolar",
        Modal::Add(_) => "Enter adiciona à seleção · Esc cancela",
    };
    // Keep the action keys visible even on small terminals and long batch previews.
    let content = Text::from(lines[..lines.len().saturating_sub(1)].to_vec());
    let paragraph = Paragraph::new(content).wrap(Wrap { trim: true });
    let max_scroll = paragraph
        .line_count(chunks[0].width)
        .saturating_sub(chunks[0].height as usize) as u16;
    frame.render_widget(paragraph.scroll((scroll.min(max_scroll), 0)), chunks[0]);
    frame.render_widget(Paragraph::new(text(footer, CYAN)), chunks[1]);
}

fn confirmation(
    request: &Request,
    options: &InstallOptions,
) -> (&'static str, Vec<Line<'static>>, u16) {
    (
        " EXECUTAR AÇÃO ",
        vec![
            Line::from(text(&request.title, CYAN).bold()),
            Line::from(""),
            Line::from(text(request.preview(), GREEN)),
            Line::from(""),
            Line::from(text(&request.detail, FG)),
            Line::from(""),
            Line::from(text(
                format!(
                    "Perfil: {}  ·  {}  ·  {}",
                    options.profile,
                    if options.offline { "offline" } else { "online" },
                    if options.frozen {
                        "lockfile congelado"
                    } else {
                        "lockfile atualizável"
                    }
                ),
                MUTED,
            )),
            Line::from(""),
            Line::from(text("Enter / y executa · Esc / n cancela", CYAN).bold()),
        ],
        17,
    )
}

use ratatui::style::Stylize;
