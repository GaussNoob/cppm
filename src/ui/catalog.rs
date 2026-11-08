use crate::{manifest::valid_name, packages, platform, store};
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    process::{Child, Stdio},
    time::{Duration, Instant},
};

pub const CATEGORIES: &[&str] = &[
    "Todas",
    "Utilitários",
    "Dados",
    "Web e rede",
    "Interface",
    "Jogos",
    "Ciência",
    "Testes",
    "Conan",
];

pub struct Entry {
    pub name: String,
    pub category: &'static str,
    pub description: &'static str,
    pub versions: BTreeSet<String>,
    pub sources: BTreeSet<String>,
}

impl Entry {
    pub fn new(name: &str) -> Self {
        let (category, description) = metadata(name);
        Self {
            name: name.into(),
            category,
            description,
            versions: BTreeSet::new(),
            sources: BTreeSet::new(),
        }
    }

    pub fn default_version(&self) -> Option<&'static str> {
        packages::CATALOG
            .iter()
            .find(|p| p.name == self.name)
            .map(|p| p.version)
    }

    pub fn matches(&self, query: &str) -> bool {
        let aliases = packages::ALIASES
            .iter()
            .filter(|(_, name)| *name == self.name)
            .map(|(alias, _)| *alias)
            .collect::<Vec<_>>()
            .join(" ");
        let haystack = format!(
            "{} {} {} {}",
            self.name, self.category, self.description, aliases
        )
        .to_lowercase();
        query
            .to_lowercase()
            .split_whitespace()
            .all(|word| haystack.contains(word))
    }
}

pub fn starter() -> BTreeMap<String, Entry> {
    packages::CATALOG
        .iter()
        .map(|p| p.name)
        .chain([
            "libcurl",
            "openssl",
            "sdl",
            "sfml",
            "raylib",
            "glfw",
            "drogon",
            "libpng",
            "libjpeg-turbo",
            "yaml-cpp",
            "tomlplusplus",
            "cli11",
            "cxxopts",
            "benchmark",
            "doctest",
            "poco",
            "libuv",
            "cpr",
            "stb",
            "freetype",
            "bullet3",
            "fftw",
            "xtensor",
            "ceres-solver",
        ])
        .map(|name| (name.into(), Entry::new(name)))
        .collect()
}

fn metadata(name: &str) -> (&'static str, &'static str) {
    match name {
        "fmt" => (
            "Utilitários",
            "Formatação de texto rápida e segura, com sintaxe semelhante a std::format.",
        ),
        "spdlog" => (
            "Utilitários",
            "Logs com níveis, cores, arquivos e escrita assíncrona.",
        ),
        "boost" => (
            "Utilitários",
            "Coleção de bibliotecas: algoritmos, containers, rede e muito mais.",
        ),
        "zlib" => (
            "Utilitários",
            "Compressão e descompressão de dados com DEFLATE.",
        ),
        "cli11" | "cxxopts" => (
            "Utilitários",
            "Opções, argumentos e ajuda para aplicações de linha de comando.",
        ),
        "onetbb" => (
            "Ciência",
            "Paralelismo, tarefas e estruturas de dados concorrentes.",
        ),
        "nlohmann_json" => (
            "Dados",
            "Leia, escreva e transforme JSON com uma API moderna de C++.",
        ),
        "sqlite3" => (
            "Dados",
            "Banco SQL embarcado, em arquivo ou memória; dispensa servidor.",
        ),
        "mariadb-connector-c" => (
            "Dados",
            "Cliente para MySQL e MariaDB. O servidor é configurado separadamente.",
        ),
        "libpq" => (
            "Dados",
            "Cliente PostgreSQL. O servidor é configurado separadamente.",
        ),
        "hiredis" => (
            "Dados",
            "Cliente Redis. O servidor é configurado separadamente.",
        ),
        "mongo-c-driver" => (
            "Dados",
            "Cliente MongoDB em C. O servidor é configurado separadamente.",
        ),
        "protobuf" => (
            "Dados",
            "Serialização de dados com Protocol Buffers e geração de código.",
        ),
        "yaml-cpp" => ("Dados", "Leitura e escrita de YAML em C++."),
        "tomlplusplus" => ("Dados", "Parser de TOML para arquivos de configuração."),
        "cpp-httplib" => ("Web e rede", "Cliente e servidor HTTP leves em C++."),
        "asio" => (
            "Web e rede",
            "Rede e operações de entrada e saída assíncronas.",
        ),
        "libcurl" => (
            "Web e rede",
            "Cliente para HTTP, HTTPS e outros protocolos de transferência.",
        ),
        "openssl" => ("Web e rede", "Criptografia, certificados e conexões TLS."),
        "drogon" => (
            "Web e rede",
            "Framework para APIs HTTP em C++. Também disponível em cppm init --template drogon.",
        ),
        "poco" => (
            "Web e rede",
            "Bibliotecas para rede, serviços e aplicações multiplataforma.",
        ),
        "libuv" => (
            "Web e rede",
            "Event loop e operações assíncronas multiplataforma.",
        ),
        "cpr" => (
            "Web e rede",
            "API C++ para requisições HTTP baseada em libcurl.",
        ),
        "qt" => (
            "Interface",
            "Framework de interfaces gráficas e aplicações multiplataforma.",
        ),
        "imgui" => (
            "Interface",
            "Dear ImGui: interfaces imediatas para ferramentas, editores e depuração.",
        ),
        "freetype" => ("Interface", "Renderização de fontes e texto."),
        "sdl" => (
            "Jogos",
            "Janelas, áudio e entrada. Use sdl2 ou sdl3 para escolher a geração.",
        ),
        "sfml" => (
            "Jogos",
            "Gráficos 2D, áudio, janelas e entrada para jogos e aplicações.",
        ),
        "raylib" => ("Jogos", "API simples para jogos, gráficos 2D e 3D e áudio."),
        "glfw" => (
            "Jogos",
            "Janelas, contextos OpenGL/Vulkan e dispositivos de entrada.",
        ),
        "glm" => (
            "Jogos",
            "Vetores, matrizes e transformações para gráficos 3D.",
        ),
        "bullet3" => ("Jogos", "Simulação de física, colisões e corpos rígidos."),
        "stb" => (
            "Jogos",
            "Pequenas bibliotecas para imagens, fontes e outros recursos.",
        ),
        "libpng" | "libjpeg-turbo" => (
            "Interface",
            "Leitura e escrita de imagens para aplicações nativas.",
        ),
        "opencv" => (
            "Ciência",
            "Visão computacional, processamento de imagens e captura de câmera.",
        ),
        "eigen" => (
            "Ciência",
            "Álgebra linear com matrizes, vetores e métodos numéricos.",
        ),
        "fftw" => ("Ciência", "Transformadas rápidas de Fourier."),
        "xtensor" => (
            "Ciência",
            "Arrays multidimensionais e operações numéricas em C++.",
        ),
        "ceres-solver" => (
            "Ciência",
            "Otimização numérica e mínimos quadrados não lineares.",
        ),
        "catch2" => (
            "Testes",
            "Testes unitários e benchmarks com uma API expressiva.",
        ),
        "gtest" => ("Testes", "Google Test: testes unitários e mocks para C++."),
        "doctest" => (
            "Testes",
            "Framework leve para testes próximos ao código de produção.",
        ),
        "benchmark" => (
            "Testes",
            "Google Benchmark: medição de desempenho e microbenchmarks.",
        ),
        _ => (
            "Conan",
            "Receita descoberta no Conan. Versão e integração CMake são resolvidas durante a instalação.",
        ),
    }
}

pub struct Discovery {
    pub entries: BTreeMap<String, Entry>,
    pub warnings: Vec<String>,
}

pub fn parse(source: &[u8]) -> Result<Discovery> {
    let value: Value = serde_json::from_slice(source).context("resposta JSON inválida do Conan")?;
    let remotes = value
        .as_object()
        .context("formato de catálogo inválido do Conan")?;
    let mut discovery = Discovery {
        entries: BTreeMap::new(),
        warnings: Vec::new(),
    };
    for (remote, recipes) in remotes {
        let Some(recipes) = recipes.as_object() else {
            continue;
        };
        if let Some(error) = recipes.get("error").and_then(Value::as_str) {
            discovery.warnings.push(format!("{remote}: {error}"));
        }
        for reference in recipes.keys() {
            let Some((name, version)) = reference.split_once('/') else {
                continue;
            };
            // cppm manifests model name/version, without user/channel references.
            if !valid_name(name)
                || version.contains(['@', '#', ':'])
                || crate::manifest::validate_version(version).is_err()
            {
                continue;
            }
            let entry = discovery
                .entries
                .entry(name.into())
                .or_insert_with(|| Entry::new(name));
            entry.versions.insert(version.into());
            entry.sources.insert(remote.clone());
        }
    }
    Ok(discovery)
}

/// Output goes to files: a large remote catalog cannot fill a pipe and block the UI.
pub struct Fetch {
    child: Child,
    stdout: tempfile::NamedTempFile,
    stderr: tempfile::NamedTempFile,
    started: Instant,
}

impl Fetch {
    pub fn start(options: &crate::conan::InstallOptions) -> Result<Self> {
        let stdout = tempfile::NamedTempFile::new()?;
        let stderr = tempfile::NamedTempFile::new()?;
        let mut command = platform::command("conan");
        store::configure(&mut command, options);
        command
            .args([
                if options.offline { "list" } else { "search" },
                "*",
                "--format=json",
                "-vwarning",
                "-cc",
                "core:non_interactive=True",
            ])
            .stdin(Stdio::null())
            .stdout(stdout.reopen()?)
            .stderr(stderr.reopen()?);
        let child = command
            .spawn()
            .context("Conan 2 não encontrado; instale com `uv tool install conan`")?;
        Ok(Self {
            child,
            stdout,
            stderr,
            started: Instant::now(),
        })
    }

    pub fn poll(&mut self) -> Result<Option<Discovery>> {
        if let Some(status) = self.child.try_wait()? {
            let errors = fs::read_to_string(self.stderr.path())?;
            ensure!(
                status.success(),
                "consulta ao Conan falhou: {}",
                errors.trim()
            );
            return parse(&fs::read(self.stdout.path())?).map(Some);
        }
        ensure!(
            self.started.elapsed() < Duration::from_secs(45),
            "a consulta ao Conan excedeu 45s; pressione r para tentar novamente"
        );
        Ok(None)
    }
}

impl Drop for Fetch {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
