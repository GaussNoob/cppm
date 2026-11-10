# Site na VPS

Cada push em `main` verifica o codigo, compila o site no GitHub e envia somente
arquivos estaticos para a VPS. Nao ha processo Node publico nem build no servidor.
Dominio: https://cppm.gaussnoob.com.br. Nginx serve `/srv/cppm/current`.

O workflow usa `VPS_DEPLOY_ENABLED=true` e os secrets `VPS_HOST`, `VPS_PORT`,
`VPS_USER`, `VPS_SSH_KEY`, `VPS_KNOWN_HOSTS`. A chave de cada projeto e separada,
sem sudo, encaminhamento SSH ou acesso aos arquivos dos outros projetos.

O deploy valida SHA256, extrai uma nova versao e troca o link `current` de forma
atomica. Confere o SHA em `/RELEASE` e restaura o link anterior se falhar.
Mantenha as ultimas tres versoes para rollback. A primeira publicacao exige DNS
Cloudflare com proxy ligado apontando para `143.95.166.46` e certificado valido.
Use SSL/TLS Full (strict); nao use Flexible.

Os arquivos em `deploy/vps/` documentam a configuracao instalada pelo administrador;
alteracoes nesses arquivos nao concedem ao workflow acesso para modificar o Nginx.
Limites por IP se aplicam ao HTML, conexoes e downloads. Assets versionados tem cache
de um ano. O Nginx usa os IPs reais encaminhados apenas pelos proxies Cloudflare
confiaveis e estes dois hosts rejeitam conexoes fora da rede Cloudflare.

DNS e regras da Cloudflare sao gerenciados separadamente: os limites na VPS nao
substituem a mitigacao de DDoS na borda. Uma falha de CI preserva a versao publicada.
