//! Pasta inicial dos seletores nativos de arquivos do FeatherSheet.
//! Usa a pasta Documentos configurada pelo usuário (XDG no Linux).
use std::path::{Path, PathBuf};

/// Abre diálogos em Documentos/feathersheet sem restringir a navegação.
pub(super) fn pasta_padrao() -> PathBuf {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let configurado = std::env::var("XDG_DOCUMENTS_DIR").ok()
        .and_then(|s| resolver_documentos(&s, &home))
        .or_else(|| {
            let config = std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config"));
            let arquivo = std::fs::read_to_string(config.join("user-dirs.dirs")).ok()?;
            arquivo.lines().find_map(|linha| {
                let valor = linha.trim().strip_prefix("XDG_DOCUMENTS_DIR=")?;
                resolver_documentos(valor, &home)
            })
        });

    let documentos = configurado.unwrap_or_else(|| {
        for nome in ["Documentos", "Documents"] {
            let pasta = home.join(nome);
            if pasta.is_dir() {
                return pasta;
            }
        }
        let idioma = std::env::var("LANG").unwrap_or_default();
        home.join(if idioma.starts_with("pt") { "Documentos" } else { "Documents" })
    });

    // Mesmo nome da casca Tauri e dos outros apps (FeatherDocs, FeatherSlide):
    // com "feathersheet" surgiam duas pastas em Documentos.
    let pasta = documentos.join("FeatherSheet");
    if std::fs::create_dir_all(&pasta).is_ok() {
        pasta
    } else if documentos.is_dir() {
        documentos
    } else {
        home
    }
}

fn resolver_documentos(valor: &str, home: &Path) -> Option<PathBuf> {
    let valor = valor.trim().trim_matches('"').trim_matches('\'');
    if valor.is_empty() {
        return None;
    }
    if valor == "$HOME" || valor == "${HOME}" {
        return Some(home.to_path_buf());
    }
    if let Some(relativo) = valor.strip_prefix("$HOME/")
        .or_else(|| valor.strip_prefix("${HOME}/"))
    {
        return Some(home.join(relativo));
    }
    let caminho = PathBuf::from(valor);
    Some(if caminho.is_absolute() { caminho } else { home.join(caminho) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn respeita_xdg_documentos_localizado() {
        let home = Path::new("/home/teste");
        assert_eq!(
            resolver_documentos("\"$HOME/Documentos\"", home),
            Some(home.join("Documentos"))
        );
        assert_eq!(
            resolver_documentos("\"${HOME}/Documents\"", home),
            Some(home.join("Documents"))
        );
        assert_eq!(
            resolver_documentos("/dados/documentos", home),
            Some(PathBuf::from("/dados/documentos"))
        );
    }
}
