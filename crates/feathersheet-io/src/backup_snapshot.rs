//! Cópias de segurança manuais do último arquivo .fshz salvo.
//!
//! Não serializa o Workbook nem carrega os bytes na RAM: usa streaming entre
//! dois arquivos e substituição atômica. O backup é intencionalmente separado
//! do autosave; não executa cópias periódicas de pastas com milhões de células.

use std::{fs, io, path::{Path, PathBuf}, sync::atomic::{AtomicU64, Ordering}, time::{SystemTime, UNIX_EPOCH}};

static SEQUENCIA: AtomicU64 = AtomicU64::new(0);

/// Grava na pasta de recuperação uma cópia única do FSHZ persistido.
/// O original nunca é escrito. A gravação não cria arquivos finais parciais.
pub fn criar_backup(origem: &Path, destino_dir: &Path) -> Result<PathBuf, String> {
    if !origem.extension().and_then(|v| v.to_str())
        .is_some_and(|v| v.eq_ignore_ascii_case("fshz"))
    {
        return Err(feathersheet_core::tr!("O backup manual requer um arquivo .fshz salvo.").into());
    }
    let meta = fs::metadata(origem)
        .map_err(|e| feathersheet_core::trf!("Não consegui acessar {}: {e}", origem.display()))?;
    if !meta.is_file() {
        return Err(feathersheet_core::tr!("O arquivo de origem não é uma planilha regular.").into());
    }
    fs::create_dir_all(destino_dir)
        .map_err(|e| feathersheet_core::trf!("Não consegui criar a pasta de backups: {e}"))?;

    let nome = origem.file_stem().and_then(|s| s.to_str()).unwrap_or("Planilha");
    let nome: String = nome.chars().take(48)
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let tempo = SystemTime::now().duration_since(UNIX_EPOCH)
        .unwrap_or_default().as_nanos();
    let sequencia = SEQUENCIA.fetch_add(1, Ordering::Relaxed);
    let destino = destino_dir.join(format!(
        "Backup-{}-{}-{}-{}.fshz",
        if nome.is_empty() { "Planilha" } else { &nome },
        tempo, std::process::id(), sequencia
    ));

    let mut arquivo_origem = fs::File::open(origem)
        .map_err(|e| feathersheet_core::trf!("Não consegui abrir a planilha salva: {e}"))?;
    super::arquivo::gravar_atomico_com(&destino, |arquivo| {
        io::copy(&mut arquivo_origem, arquivo).map(|_| ())
    }).map_err(|e| feathersheet_core::trf!("Falha ao gravar a cópia de segurança: {e}"))?;
    Ok(destino)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pasta_de_teste() -> PathBuf {
        std::env::temp_dir().join(format!(
            "feathersheet-backup-snapshot-{}-{}",
            std::process::id(), SEQUENCIA.fetch_add(1, Ordering::Relaxed),
        ))
    }

    #[test]
    fn copia_ultimo_arquivo_salvo_sem_alterar_original() {
        let pasta = pasta_de_teste();
        fs::create_dir_all(&pasta).unwrap();
        let origem = pasta.join("Meu Orçamento.fshz");
        let bytes = b"FSH2 dados simulados de teste";
        fs::write(&origem, bytes).unwrap();
        let destino_dir = pasta.join("Temporarios");

        let primeiro = criar_backup(&origem, &destino_dir).unwrap();
        let segundo = criar_backup(&origem, &destino_dir).unwrap();
        assert_ne!(primeiro, segundo);
        assert!(primeiro.starts_with(&destino_dir));
        assert_eq!(primeiro.extension().and_then(|s| s.to_str()), Some("fshz"));
        assert_eq!(fs::read(&primeiro).unwrap(), bytes);
        assert_eq!(fs::read(&segundo).unwrap(), bytes);
        assert_eq!(fs::read(&origem).unwrap(), bytes);
        assert_eq!(fs::read_dir(&destino_dir).unwrap().count(), 2);
        fs::remove_dir_all(pasta).unwrap();
    }

    #[test]
    fn recusa_arquivo_nao_fshz_ou_inexistente() {
        let pasta = pasta_de_teste();
        fs::create_dir_all(&pasta).unwrap();
        let csv = pasta.join("valores.csv");
        fs::write(&csv, b"a,b").unwrap();
        assert!(criar_backup(&csv, &pasta.join("copias")).is_err());
        assert!(criar_backup(&pasta.join("ausente.fshz"), &pasta.join("copias")).is_err());
        assert!(!pasta.join("copias").exists());
        fs::remove_dir_all(pasta).unwrap();
    }
}
