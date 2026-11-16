# {name}

API HTTP em C++ com [Drogon](https://github.com/drogonframework/drogon), criada com
`cppm init --template drogon`.

```sh
cppm run dev                    # inicia em http://127.0.0.1:8080
cppm run dev -- --port 9090     # outra porta
cppm watch                      # reinicia a API sempre que você salva
cppm run test                   # sobe um servidor temporário e testa /health
```

`GET /` e `GET /health` respondem JSON. O primeiro build compila o Drogon e pode
demorar alguns minutos; os seguintes reaproveitam tudo.
O teste usa Python 3.

## Estrutura

```text
{name}/
  cppm.toml         # dependências (drogon) e scripts
  CMakeLists.txt    # build e testes
  src/main.cpp      # porta, rotas iniciais e início do servidor
  tests/smoke.py    # teste HTTP: sobe a API e chama /health
```

## Próximos passos

### 1. Adicionar uma rota rápida

Em `src/main.cpp`, antes de `drogon::app().addListener(...)`:

```cpp
drogon::app().registerHandler(
    "/ola/{nome}",
    [](const drogon::HttpRequestPtr&,
       std::function<void(const drogon::HttpResponsePtr&)>&& callback,
       const std::string& nome) {
        Json::Value body;
        body["mensagem"] = "Olá, " + nome;
        callback(drogon::HttpResponse::newHttpJsonResponse(body));
    },
    {drogon::Get});
```

Teste com `curl http://127.0.0.1:8080/ola/Ana`. `{nome}` vira o parâmetro da função.
Query string: `req->getParameter("pagina")`.

### 2. Organizar em controllers

Quando as rotas crescerem, mova-as para classes. Arquivos em `src/` entram no build
automaticamente e o Drogon registra o controller sozinho — não precisa mexer no `main.cpp`.

```cpp
// src/tarefas.h
#pragma once
#include <drogon/HttpController.h>

class Tarefas : public drogon::HttpController<Tarefas> {
public:
    METHOD_LIST_BEGIN
    ADD_METHOD_TO(Tarefas::listar, "/tarefas", drogon::Get);
    ADD_METHOD_TO(Tarefas::buscar, "/tarefas/{id}", drogon::Get);
    ADD_METHOD_TO(Tarefas::criar, "/tarefas", drogon::Post);
    METHOD_LIST_END

    using Callback = std::function<void(const drogon::HttpResponsePtr&)>;
    void listar(const drogon::HttpRequestPtr& req, Callback&& callback);
    void buscar(const drogon::HttpRequestPtr& req, Callback&& callback, int id);
    void criar(const drogon::HttpRequestPtr& req, Callback&& callback);
};
```

```cpp
// src/tarefas.cpp
#include "tarefas.h"

void Tarefas::listar(const drogon::HttpRequestPtr&, Callback&& callback) {
    Json::Value lista(Json::arrayValue);
    lista.append("estudar drogon");
    callback(drogon::HttpResponse::newHttpJsonResponse(lista));
}

void Tarefas::buscar(const drogon::HttpRequestPtr&, Callback&& callback, int id) {
    Json::Value body;
    body["id"] = id;
    callback(drogon::HttpResponse::newHttpJsonResponse(body));
}

// POST com corpo JSON e validação
void Tarefas::criar(const drogon::HttpRequestPtr& req, Callback&& callback) {
    const auto json = req->getJsonObject();
    if (!json || !(*json)["titulo"].isString()) {
        auto resp = drogon::HttpResponse::newHttpJsonResponse(
            Json::Value("campo 'titulo' obrigatório"));
        resp->setStatusCode(drogon::k400BadRequest);
        return callback(resp);
    }
    Json::Value body;
    body["titulo"] = (*json)["titulo"];
    auto resp = drogon::HttpResponse::newHttpJsonResponse(body);
    resp->setStatusCode(drogon::k201Created);
    callback(resp);
}
```

```sh
curl -X POST http://127.0.0.1:8080/tarefas \
     -H 'Content-Type: application/json' -d '{"titulo": "comprar pão"}'
```

### 3. Servir um site (HTML, CSS, JS)

Crie `public/index.html` e, no `CMakeLists.txt`, informe a pasta ao código:

```cmake
target_compile_definitions(${PROJECT_NAME} PRIVATE
    PUBLIC_DIR="${CMAKE_CURRENT_SOURCE_DIR}/public")
```

Em `src/main.cpp`, antes de `addListener`:

```cpp
drogon::app().setDocumentRoot(PUBLIC_DIR);
```

Remova a rota `/` do `main.cpp` para que `http://127.0.0.1:8080/` mostre o `index.html`.
O front-end pode chamar a API com `fetch('/tarefas')`.
Em produção, copie a pasta `public/` junto com o executável e ajuste o caminho.

### 4. Usar outras bibliotecas

```sh
cppm add spdlog         # logs
cppm add sqlite3        # banco de dados embutido
cppm add libpq          # PostgreSQL
cppm packages           # tudo que tem configuração pronta
```

As bibliotecas ficam disponíveis sem editar o `CMakeLists.txt`. Veja exemplos de banco
de dados em https://github.com/GaussNoob/cppm/blob/main/docs/packages.md.

### 5. Testar novas rotas

`tests/smoke.py` mostra o padrão: sobe o servidor em uma porta livre, espera ficar
pronto e faz requisições com `urllib`. Para testar uma rota nova, adicione outra
chamada a `urlopen` logo depois da verificação de `/health`, por exemplo:

```python
with urlopen(f'http://127.0.0.1:{port}/tarefas', timeout=1) as response:
    assert json.load(response) == ['estudar drogon']
```

### 6. Colocar no ar

```sh
cppm run build    # Release em .cppm/build/release/bin/{name}
```

O servidor escuta só em `127.0.0.1`. Para aceitar conexões de outras máquinas,
troque para `"0.0.0.0"` em `addListener` e, em produção, deixe um proxy
(nginx, Caddy) na frente cuidando de HTTPS.

Referência do Drogon: https://drogonframework.github.io/drogon-docs/

