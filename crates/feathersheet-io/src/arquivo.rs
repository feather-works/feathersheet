//! Substituição de arquivos sem expor conteúdo parcialmente escrito.
use std::{fs::{self, OpenOptions}, io::{self, Write}, path::Path, sync::atomic::{AtomicU64, Ordering}};

static SEQUENCIA: AtomicU64 = AtomicU64::new(0);

/// O temporário fica no mesmo diretório para que o rename seja atômico.
/// Uma falha antes do rename preserva o arquivo anterior.
pub fn gravar_atomico(destino: &Path, bytes: &[u8]) -> io::Result<()> {
    gravar_atomico_com(destino, |arquivo| arquivo.write_all(bytes))
}

/// Salva em streaming, sem criar um buffer com o arquivo todo.
/// Se a escrita falhar, o temporário é excluído e o destino é preservado.
pub fn gravar_atomico_com(
    destino: &Path,
    escrever: impl FnOnce(&mut fs::File) -> io::Result<()>,
) -> io::Result<()> {
    let nome = destino.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Arquivo sem nome"))?;
    apagar_parciais_velhos(destino);
    let mut tentativa = 0;
    let (temporario, mut arquivo) = loop {
        let mut nome_temporario = nome.to_os_string();
        nome_temporario.push(format!(".{}.{}.parcial", std::process::id(), SEQUENCIA.fetch_add(1, Ordering::Relaxed)));
        let temporario = destino.with_file_name(nome_temporario);
        match OpenOptions::new().write(true).create_new(true).open(&temporario) {
            Ok(arquivo) => break (temporario, arquivo),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists && tentativa < 16 => tentativa += 1,
            Err(e) => return Err(e),
        }
    };
    let resultado = (|| {
        if let Ok(meta) = fs::metadata(destino) { arquivo.set_permissions(meta.permissions())?; }
        escrever(&mut arquivo)?;
        arquivo.sync_all()?;
        drop(arquivo);
        fs::rename(&temporario, destino)?;
        // A troca de nome só fica no disco com a pasta sincronizada (queda de luz logo depois
        // voltava a versão anterior). No Windows abrir a pasta falha: lá o rename já é durável.
        if let Some(pasta) = destino.parent() { let _ = fs::File::open(pasta).and_then(|d| d.sync_all()); }
        Ok(())
    })();
    if resultado.is_err() { let _ = fs::remove_file(&temporario); }
    resultado
}

/// `.parcial` deste arquivo que sobrou de um salvamento interrompido (app fechado à força)
/// há mais de uma hora: tem o tamanho do arquivo e ficava para sempre na pasta da pessoa.
fn apagar_parciais_velhos(destino: &Path) {
    let (Some(pasta), Some(nome)) = (destino.parent(), destino.file_name().and_then(|n| n.to_str())) else { return };
    let prefixo = format!("{nome}.");
    let Ok(itens) = fs::read_dir(if pasta.as_os_str().is_empty() { Path::new(".") } else { pasta }) else { return };
    for item in itens.flatten() {
        let n = item.file_name();
        let Some(n) = n.to_str() else { continue };
        let velho = item.metadata().and_then(|m| m.modified()).ok()
            .and_then(|t| t.elapsed().ok()).is_some_and(|d| d.as_secs() > 3600);
        if n.starts_with(&prefixo) && n.ends_with(".parcial") && velho {
            let _ = fs::remove_file(item.path());
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn parcial_velho_de_salvamento_interrompido_some_no_proximo() {
        let dir = std::env::temp_dir().join(format!("feathersheet-io-p-{}-{}", std::process::id(), SEQUENCIA.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&dir).unwrap();
        let destino = dir.join("planilha.fshz");
        let velho = dir.join("planilha.fshz.999.0.parcial");
        let novo = dir.join("planilha.fshz.998.0.parcial");
        let outro = dir.join("outra.fshz.999.0.parcial");
        for p in [&velho, &novo, &outro] { fs::write(p, b"x").unwrap(); }
        let duas_horas = std::time::SystemTime::now() - std::time::Duration::from_secs(7200);
        for p in [&velho, &outro] { fs::File::options().write(true).open(p).unwrap().set_modified(duas_horas).unwrap(); }
        gravar_atomico(&destino, b"dados").unwrap();
        assert!(!velho.exists(), "o .parcial velho deste arquivo some");
        assert!(novo.exists(), "um salvamento recente (outro processo) não é tocado");
        assert!(outro.exists(), "parcial de outro arquivo não é tocado");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn substitui_e_remove_temporario_em_falha() {
        let dir = std::env::temp_dir().join(format!("feathersheet-io-{}-{}", std::process::id(), SEQUENCIA.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&dir).unwrap();
        let destino = dir.join("planilha.fshz");
        gravar_atomico(&destino, b"antes").unwrap();
        gravar_atomico(&destino, b"depois").unwrap();
        assert_eq!(fs::read(&destino).unwrap(), b"depois");
        assert!(gravar_atomico_com(&destino, |arquivo| {
            arquivo.write_all(b"incompleto")?;
            Err(io::Error::other("falha simulada do compressor"))
        }).is_err());
        assert_eq!(fs::read(&destino).unwrap(), b"depois");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1, "remove temporário após falha");
        let pasta = dir.join("pasta");
        fs::create_dir(&pasta).unwrap();
        assert!(gravar_atomico(&pasta, b"erro").is_err());
        assert!(pasta.is_dir());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);
        fs::remove_file(destino).unwrap();
        fs::remove_dir(pasta).unwrap();
        fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn falha_do_exportador_em_stream_preserva_destino() {
        let dir = std::env::temp_dir().join(format!(
            "feathersheet-stream-{}-{}",
            std::process::id(), SEQUENCIA.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&dir).unwrap();
        let destino = dir.join("dados.csv");
        gravar_atomico(&destino, b"original").unwrap();
        let falha = gravar_atomico_com(&destino, |arquivo| {
            arquivo.write_all(b"incompleto")?;
            Err(io::Error::new(io::ErrorKind::Other, "escrita interrompida"))
        });
        assert!(falha.is_err());
        assert_eq!(fs::read(&destino).unwrap(), b"original");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_file(destino).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
