import project from "@/generated/project.json";

export const catalogGroups = ["Bancos de dados", "Rede e HTTP", "Interfaces", "Matemática e visão", "Serialização", "Testes", "Utilitários"] as const;
export type CatalogGroup = typeof catalogGroups[number];

const details: Record<string, { label: string; command?: string; header: string; group: CatalogGroup; description: string }> = {
  sqlite3: { label: "SQLite", command: "sqlite", header: "sqlite3.h", group: "Bancos de dados", description: "Banco SQL local, em arquivo ou em memória." },
  "mariadb-connector-c": { label: "MySQL / MariaDB", command: "mysql", header: "mysql.h", group: "Bancos de dados", description: "Driver cliente compatível com os dois servidores." },
  libpq: { label: "PostgreSQL", command: "postgres", header: "libpq-fe.h", group: "Bancos de dados", description: "Cliente libpq para conectar ao PostgreSQL." },
  hiredis: { label: "Redis", command: "redis", header: "hiredis/hiredis.h", group: "Bancos de dados", description: "Cliente e parser do protocolo Redis." },
  "mongo-c-driver": { label: "MongoDB", command: "mongodb", header: "mongoc/mongoc.h", group: "Bancos de dados", description: "Cliente MongoDB e documentos BSON." },
  "cpp-httplib": { label: "cpp-httplib", header: "httplib.h", group: "Rede e HTTP", description: "Servidor e cliente HTTP para C++." },
  asio: { label: "Asio", header: "asio.hpp", group: "Rede e HTTP", description: "I/O assíncrono em uma biblioteca independente do Boost." },
  imgui: { label: "Dear ImGui", header: "imgui.h", group: "Interfaces", description: "Interface imediata; escolha os backends de janela e renderização." },
  qt: { label: "Qt 6", header: "QCoreApplication", group: "Interfaces", description: "Framework de aplicações, interfaces e ferramentas." },
  opencv: { label: "OpenCV", header: "opencv2/core.hpp", group: "Matemática e visão", description: "Processamento de imagens e visão computacional." },
  eigen: { label: "Eigen", header: "Eigen/Dense", group: "Matemática e visão", description: "Matrizes, vetores e álgebra linear." },
  glm: { label: "GLM", header: "glm/glm.hpp", group: "Matemática e visão", description: "Matemática para aplicações gráficas." },
  onetbb: { label: "oneTBB", header: "oneapi/tbb/parallel_for.h", group: "Matemática e visão", description: "Algoritmos e tarefas em paralelo." },
  protobuf: { label: "Protobuf", header: "google/protobuf/message.h", group: "Serialização", description: "Mensagens tipadas e serialização Protocol Buffers." },
  nlohmann_json: { label: "nlohmann/json", command: "json", header: "nlohmann/json.hpp", group: "Serialização", description: "Leitura, construção e escrita de JSON." },
  catch2: { label: "Catch2", header: "catch2/catch_test_macros.hpp", group: "Testes", description: "Casos de teste e assertions para C++." },
  gtest: { label: "Google Test", command: "googletest", header: "gtest/gtest.h", group: "Testes", description: "Framework de testes do Google, incluindo Google Mock." },
  boost: { label: "Boost", header: "boost/filesystem.hpp", group: "Utilitários", description: "Bibliotecas de uso geral, de Filesystem a Boost.Asio." },
  fmt: { label: "fmt", header: "fmt/format.h", group: "Utilitários", description: "Formatação de texto." },
  spdlog: { label: "spdlog", header: "spdlog/spdlog.h", group: "Utilitários", description: "Logging para aplicações C++." },
  zlib: { label: "zlib", header: "zlib.h", group: "Utilitários", description: "Compressão de dados." },
};

export const libraryCatalog = project.catalog.map((entry) => {
  const detail = details[entry.name];
  if (!detail) throw new Error(`Missing documentation for native package ${entry.name}`);
  return { ...entry, ...detail, command: `cppm install ${detail.command ?? entry.name}`, aliases: project.aliases.filter((alias) => alias.package === entry.name).map((alias) => alias.name) };
});
