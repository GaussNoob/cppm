import { catalogGroups, libraryCatalog, type CatalogGroup } from "@/lib/catalog";
import { CopyButton } from "./ui";

export function LibraryCatalog({ groups }: { groups?: readonly CatalogGroup[] }) {
  const selected = groups ?? catalogGroups;
  const entries = libraryCatalog.filter((library) => selected.includes(library.group));
  return <div className="library-catalog">
    {selected.map((group) => <section className="catalog-group" key={group} aria-label={group}>
      <h4>{group}<span>{entries.filter((library) => library.group === group).length} pacotes</span></h4>
      <div className="doc-table-scroll" tabIndex={0} role="region" aria-label={`Pacotes: ${group}`}><table className="doc-table catalog-table"><thead><tr><th>Biblioteca</th><th>Versão</th><th>Instalar</th></tr></thead><tbody>{entries.filter((library) => library.group === group).map((library) => <tr key={library.name}><td><strong>{library.label}</strong><small>{library.description}</small><code className="catalog-header">&lt;{library.header}&gt;</code></td><td><code>{library.version}</code></td><td><div className="catalog-command"><code>{library.command}</code><CopyButton text={library.command} label={`Copiar: ${library.command}`} /></div>{library.aliases.length > 0 && <small className="catalog-aliases">Aliases: {library.aliases.join(", ")}</small>}</td></tr>)}</tbody></table></div>
    </section>)}
    <details className="catalog-cmake"><summary>Pacotes e targets CMake do catálogo</summary><div className="doc-table-scroll" tabIndex={0} role="region" aria-label="Referência dos targets CMake"><table className="doc-table"><thead><tr><th>Receita Conan</th><th>find_package</th><th>Target</th></tr></thead><tbody>{entries.map((library) => <tr key={library.name}><td><code>{library.name}</code></td><td><code>{library.cmakePackage}</code></td><td><code>{library.target}</code></td></tr>)}</tbody></table></div><p>A integração usa os metadados da receita instalada. Targets e opções podem variar conforme a versão e o perfil.</p></details>
  </div>;
}
