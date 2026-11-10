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
