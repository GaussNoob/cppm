# Builds sem alterações

Medição local em Linux x86_64, em 7 de novembro de 2026. Cada projeto foi
compilado uma vez para aquecimento, seguido de três execuções de `cppm build`
com fontes inalteradas e dependências já disponíveis no armazenamento Conan.
Os valores abaixo são medianas do tempo total do comando em Debug.

| Projeto | Antes | Depois | Redução do tempo |
| --- | ---: | ---: | ---: |
| Aplicação básica | 0,3145 s | 0,0194 s | 93,8% (16,2×) |
| API Drogon | 4,8323 s | 0,5197 s | 89,2% (9,3×) |

Antes, cada chamada executava a preparação Conan e a configuração CMake com
`--fresh`. Agora, instalações válidas são reutilizadas após conferir perfis,
lockfile, ambiente, geradores e manifests dos pacotes. A configuração CMake
válida é reutilizada e o Ninja verifica fontes, headers e regras de build.

Amostras em segundos:

| Projeto | Antes | Depois |
| --- | --- | --- |
| Aplicação básica | 0,3205; 0,2998; 0,3145 | 0,0194; 0,0193; 0,0194 |
| API Drogon | 4,8116; 4,8323; 4,8566 | 0,5408; 0,5197; 0,5125 |

## Builds sem alterações, com dependências, sem iniciar o Conan

Medição local em Linux x86_64 (2 vCPUs), em 10 de novembro de 2026, com um
projeto que depende de uma biblioteca estática de uma receita local. Cada
comando foi aquecido uma vez; os valores são medianas de sete execuções.

| Comando | Antes | Depois | Redução do tempo |
| --- | ---: | ---: | ---: |
| `cppm build` com uma dependência | 1,071 s | 0,022 s | 97,9% (48×) |
| `cppm exec` com uma dependência | 1,389 s | 0,024 s | 98,3% (57×) |
| `cppm build` sem dependências | 0,034 s | 0,021 s | 38% (1,6×) |

Antes, a validação da instalação iniciava o Conan quatro vezes em sequência
(`--version`, `config home`, `profile detect`, `profile show`) e `exec` uma
quinta, para localizar o armazenamento do lease. Nesta máquina cada início do
Conan custa cerca de 0,27 s. Agora uma chave calculada só com leituras de
arquivos prova que nada do que o Conan leria mudou; veja
[Reuso sem iniciar o Conan](architecture.md#reuso-sem-iniciar-o-conan).
O ganho sem dependências vem de localizar `cmake` e `ninja` no PATH em vez de
executar `--version` em cada build.

O teste `unchanged_builds_do_not_start_conan_and_every_input_change_is_noticed`
usa um launcher que registra cada início do Conan: zero inícios em `build`,
`exec` e `test` sem alterações, e ao menos um após mudar um perfil incluído,
`global.conf`, o próprio launcher, ou ao usar `--fresh`.

`cppm store dedupe` em dois pacotes com 12 mil headers e 350 MB de bibliotecas:
3,56 s → 3,28 s na primeira execução e 0,87 s → 0,50 s quando os arquivos já
estão compartilhados. O hashing usa todos os núcleos (dois nesta medição).

Para observar as etapas de um projeto, execute `cppm build --verbose`.
Para repetir a medição em outro ambiente, aqueça o build e cronometre o comando:

```sh
cppm build
python3 - <<'PY'
import statistics
import subprocess
import time

samples = []
for _ in range(3):
    start = time.perf_counter()
    subprocess.run(['cppm', 'build'], check=True, stdout=subprocess.DEVNULL)
    samples.append(time.perf_counter() - start)
print('Amostras:', samples)
print('Mediana:', statistics.median(samples))
PY
```

Os testes de [workflow](../tests/workflow.rs) verificam que mudanças em fontes,
headers, CMake, flags e perfis incluídos chegam ao executável, e que `--fresh`
força a preparação completa. Os testes de templates também verificam alterações
no shell HTML e reinício HTTP de Drogon/WASM na mesma porta.
