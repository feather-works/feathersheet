//! Ponte entre CSV/TSV compartilhado e a pasta colunar nativa do FeatherSheet.
//!
//! Somente a leitura tabular reutiliza featheroffice-io::csv_lotes.
//! XLSX e FSHZ continuam com seus leitores, escritores e validações próprios.
//! A callback drena os lotes para a aba, evitando acumular a matriz inteira.

use crate::csv::{dividir_csv_em_lotes, escolher_separador, LINHAS_POR_LOTE};
use feathersheet_core::{
    celulas::refs::{MAX_COLS, MAX_ROWS},
    planilha::workbook::Workbook,
};

/// Importação do disco: o texto completo nunca coexiste com a pasta viva.
/// Vai por colunas (ver `csv_colunas`); TSV é sempre TAB, e no CSV o
/// separador do idioma só vale se dividir o cabeçalho.
pub fn importar_csv_tsv_arquivo(
    caminho: &std::path::Path,
    tsv: bool,
    separador: Option<char>,
) -> Result<Workbook, String> {
    let arquivo = std::fs::File::open(caminho).map_err(|e| e.to_string())?;
    let mut workbook = Workbook::new();
    let aba = &mut workbook.aba_mut(0).unwrap().sheet;
    let inicio = std::time::Instant::now();
    let leitor = std::io::BufReader::with_capacity(64 * 1024, crate::progresso::Contador(arquivo));
    let (delim, leitor) = crate::csv_colunas::separador_e_leitor(leitor, tsv, separador)
        .map_err(|e| format!("Não consegui ler CSV/TSV UTF-8: {e}"))?;
    if !delim.is_ascii() {
        importar_celula_a_celula(leitor, delim, aba)?;
        return Ok(workbook);
    }
    let celulas = crate::csv_colunas::importar(leitor, delim as u8, aba)?;
    feathersheet_core::anotar!(
        "[abrir] CSV: {celulas} células em {:.1} s · {} núcleos · por colunas",
        inicio.elapsed().as_secs_f64(), feathersheet_core::paralelismo::nucleos());
    Ok(workbook)
}

/// Separador fora do ASCII (só por quem chama a biblioteca; a interface
/// passa `,` ou `;`): o caminho célula a célula.
fn importar_celula_a_celula(
    leitor: impl std::io::BufRead,
    delim: char,
    aba: &mut feathersheet_core::planilha::sheet::Sheet,
) -> Result<(), String> {
    let mut linha = 0u32;
    let mut excedeu = false;
    let entregar = |lote: &mut Vec<Vec<String>>| {
        for campos in lote.drain(..) {
            if linha >= MAX_ROWS || campos.len() > MAX_COLS as usize {
                excedeu = true;
                continue;
            }
            for (coluna, valor) in campos.into_iter().enumerate() {
                if !valor.is_empty() {
                    aba.set_cell_raw(linha, coluna as u32, valor, None);
                }
            }
            linha += 1;
        }
    };
    feather_io::csv_lotes::dividir_csv_leitor(leitor, Some(delim), 512, entregar)
        .map_err(|e| format!("Não consegui ler CSV/TSV UTF-8: {e}"))?;
    if excedeu {
        return Err("CSV/TSV excede os limites de linhas ou colunas da planilha".into());
    }
    Ok(())
}

/// Abre texto CSV/TSV UTF-8 em uma pasta colunar, sem converter valores para
/// uma tabela genérica: assim fórmulas, a engine de células e os formatos
/// nativos permanecem sob responsabilidade do FeatherSheet.
///
/// `tsv` força separador TAB; CSV detecta a partir da primeira linha.
/// A importação é transacional para o chamador: erro descarta a pasta local.
pub fn importar_csv_tsv(bytes: Vec<u8>, tsv: bool) -> Result<Workbook, String> {
    importar_csv_tsv_com_separador(bytes, tsv, None)
}

/// `None` detecta o arquivo; `Some` usa o separador escolhido na UI.
pub fn importar_csv_tsv_com_separador(
    bytes: Vec<u8>,
    tsv: bool,
    separador: Option<char>,
) -> Result<Workbook, String> {
    let conteudo = String::from_utf8(bytes).map_err(|_| "CSV/TSV precisa ser UTF-8".to_string())?;
    let separador = if tsv {
        '\t'
    } else {
        escolher_separador(&conteudo, separador)
    };
    let mut workbook = Workbook::new();
    let aba = &mut workbook
        .aba_mut(0)
        .expect("Workbook::new cria a primeira aba")
        .sheet;
    let mut linha: u32 = 0;
    let mut excedeu = false;

    // O parser comum retira BOM e entrega linhas completas mesmo quando
    // um campo citado contém CRLF. O motor nativo armazena só não vazios.
    dividir_csv_em_lotes(&conteudo, separador, LINHAS_POR_LOTE, |lote| {
        for campos in lote.drain(..) {
            if linha >= MAX_ROWS || campos.len() > MAX_COLS as usize {
                excedeu = true;
                continue;
            }
            for (coluna, valor) in campos.into_iter().enumerate() {
                if !valor.is_empty() {
                    aba.set_cell_raw(linha, coluna as u32, valor, None);
                }
            }
            linha += 1;
        }
    });
    if excedeu {
        return Err("CSV/TSV excede os limites de linhas ou colunas da planilha".into());
    }
    Ok(workbook)
}

/// Escreve apenas a aba ativa, linha a linha, sem materializar uma matriz ou
/// um único String gigante. Fórmulas são exportadas pelo VALOR calculado,
/// como o texto exibido no TypeScript; não pelo texto "=..." da fórmula.
///
/// CSV usa vírgula; TSV usa TAB. Aspas, quebras de linha e o separador
/// recebem escape RFC 4180, válido também para o parser TSV da importação.
pub fn exportar_delimitado<W: std::io::Write>(
    workbook: &Workbook, indice: usize, tsv: bool, destino: &mut W,
) -> std::io::Result<()> {
    use feathersheet_core::Value;
    let sheet = &workbook.aba(indice).ok_or_else(|| std::io::Error::new(
        std::io::ErrorKind::NotFound, "Aba para exportação não encontrada",
    ))?.sheet;
    if sheet.is_empty() { return Ok(()); }
    let utilizado = sheet.used_rect();
    let separador = if tsv { '\t' } else { ',' };
    let sep = [separador as u8];
    for linha in 0..=utilizado.r1 {
        for coluna in 0..=utilizado.c1 {
            if coluna > 0 { destino.write_all(&sep)?; }
            let celula = match workbook.valor(indice, linha, coluna) {
                Value::Empty => String::new(),
                // A mesma escrita da tela (1e21, 1e-7), não a do Rust.
                Value::Number(v) => feathersheet_core::celulas::number::to_js_string(v),
                Value::Text(s) => s,
                Value::Bool(true) => "VERDADEIRO".to_owned(),
                Value::Bool(false) => "FALSO".to_owned(),
                Value::Error(e) => e.to_string(),
            };
            crate::csv::escrever_campo(destino, &celula, separador)?;
        }
        // Quebra de linha igual à exportação TypeScript.
        if linha < utilizado.r1 { destino.write_all(b"\n")?; }
    }
    Ok(())
}

/// Gravação atômica em streaming: não altera o .fshz aberto e preserva
/// o destino anterior caso ocorra qualquer erro de exportação.
pub fn gravar_csv_tsv(
    workbook: &Workbook, indice: usize, path: &std::path::Path, tsv: bool,
) -> std::io::Result<()> {
    use std::io::{BufWriter, Write};
    crate::arquivo::gravar_atomico_com(path, |arquivo| {
        let mut escrita = BufWriter::with_capacity(64 * 1024, arquivo);
        exportar_delimitado(workbook, indice, tsv, &mut escrita)?;
        escrita.flush()
    })
}


#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn csv_com_bom_locale_e_aspas_multilinha_vira_pasta_colunar() {
        let dados = "\u{feff}Nome;Valor;Observação\r\nJoão;1.234,56;\"Primeira\r\nSegunda\"\r\nAna;-0,25;\"\"\r\n";
        let pasta = importar_csv_tsv(dados.as_bytes().to_vec(), false).unwrap();
        let aba = &pasta.aba(0).unwrap().sheet;
        assert_eq!(aba.get_raw(0, 0), "Nome");
        assert_eq!(aba.get_raw(1, 0), "João");
        // O motor normaliza a representação textual dos números importados.
        assert_eq!(
            aba.get_value(1, 1),
            feathersheet_core::Value::Number(1234.56)
        );
        assert_eq!(aba.get_raw(1, 2), "Primeira\r\nSegunda");
        assert_eq!(aba.get_value(2, 1), feathersheet_core::Value::Number(-0.25));
        assert_eq!(aba.get_raw(2, 2), "");
    }

    #[test]
    fn tsv_forca_tab_e_preserva_formulas_sem_trocar_engine() {
        let pasta = importar_csv_tsv(b"5\t=A1*2\n8\t=A2*2".to_vec(), true).unwrap();
        let aba = &pasta.aba(0).unwrap().sheet;
        assert_eq!(aba.get_raw(0, 1), "=A1*2");
        assert_eq!(aba.get_raw(1, 1), "=A2*2");
        assert_eq!(aba.get_raw(1, 0), "8");
    }

    #[test]
    fn csv_multiplos_lotes_mantem_linha_absoluta() {
        let mut dados = String::from("ID;Nome\n");
        for i in 0..(LINHAS_POR_LOTE + 3) {
            dados.push_str(&format!("{i};linha{i}\n"));
        }
        let pasta = importar_csv_tsv(dados.into_bytes(), false).unwrap();
        let aba = &pasta.aba(0).unwrap().sheet;
        assert_eq!(aba.get_raw(1, 0), "0");
        assert_eq!(
            aba.get_raw(LINHAS_POR_LOTE as u32, 1),
            format!("linha{}", LINHAS_POR_LOTE - 1)
        );
        assert_eq!(
            aba.get_raw((LINHAS_POR_LOTE + 3) as u32, 1),
            format!("linha{}", LINHAS_POR_LOTE + 2)
        );
    }

    #[test]
    fn substituto_do_worker_importa_mais_de_dez_mil_linhas_com_aspas_multilinha() {
        // O worker TS cortava por quebras físicas e já não tinha seu
        // import delimited.ts. O caminho de arquivo Rust não pode fazer isso.
        let caminho=std::env::temp_dir().join(format!(
            "feathersheet-csv-worker-{}.csv",std::process::id()
        ));
        let mut dados=String::from("\u{feff}Código;Descrição;Estado\r\n");
        for i in 0..10_019 {
            let descricao=if i==9_999 {
                "\"linha\r\nseguinte; ç\""
            } else { "normal" };
            dados.push_str(&format!("{i};{descricao};ok\r\n"));
        }
        std::fs::write(&caminho,&dados).unwrap();
        let disco=importar_csv_tsv_arquivo(&caminho,false,None).unwrap();
        let buffer=importar_csv_tsv(dados.into_bytes(),false).unwrap();
        let a=&disco.aba(0).unwrap().sheet;
        let b=&buffer.aba(0).unwrap().sheet;
        assert_eq!(a.get_raw(0,0),"Código");
        assert_eq!(a.get_raw(10_000,1),"linha\r\nseguinte; ç");
        assert_eq!(a.get_raw(10_019,0),"10018");
        assert_eq!(a.max_row,b.max_row);
        for r in [0,1,9_999,10_000,10_001,10_019] {
            for c in 0..3 {
                assert_eq!(a.get_raw(r,c),b.get_raw(r,c),"linha {r}, coluna {c}");
            }
        }
        std::fs::remove_file(&caminho).unwrap();
    }

    #[test]
    fn arquivo_incremental_equivale_a_importacao_inteira() {
        let caminho =
            std::env::temp_dir().join(format!("feathersheet-csv-fluxo-{}.csv", std::process::id()));
        let mut texto = String::from("\u{feff}Nome;Valor;Fórmula;Nota\r\n");
        for i in 0..1030 {
            texto.push_str(&format!(
                "João{i};1.234,56;=B{}*2;\"ação\r\nç;\"\"fim\"\"\"\r\n",
                i + 2
            ));
        }
        std::fs::write(&caminho, &texto).unwrap();
        for separador in [None, Some(';'), Some(',')] {
            let antiga =
                importar_csv_tsv_com_separador(texto.as_bytes().to_vec(), false, separador)
                    .unwrap();
            let nova = importar_csv_tsv_arquivo(&caminho, false, separador).unwrap();
            let a = &antiga.aba(0).unwrap().sheet;
            let b = &nova.aba(0).unwrap().sheet;
            assert_eq!((a.max_row, a.max_col), (b.max_row, b.max_col));
            for r in 0..=a.max_row {
                for c in 0..=a.max_col {
                    assert_eq!(a.get_raw(r, c), b.get_raw(r, c));
                }
            }
        }
        std::fs::write(&caminho, "ação\t2\r\nç\t3").unwrap();
        let tsv = importar_csv_tsv_arquivo(&caminho, true, Some(';')).unwrap();
        assert_eq!(tsv.aba(0).unwrap().sheet.get_raw(1, 1), "3");
        std::fs::write(&caminho, [b'a', 0xff]).unwrap();
        assert!(importar_csv_tsv_arquivo(&caminho, false, None).is_err());
        std::fs::remove_file(caminho).unwrap();
    }

    #[test]
    fn csv_invalido_falha_sem_aceitar_bytes_corrompidos() {
        let erro = match importar_csv_tsv(vec![b'A', b',', 0xFF], false) {
            Ok(_) => panic!("deveria recusar CSV inválido"),
            Err(e) => e,
        };
        assert!(erro.contains("UTF-8"));
    }

    #[test]
    fn formatos_nativos_continuam_com_codecs_originais() {
        let mut wb = Workbook::new();
        wb.escrever_celula(0, 0, 0, "9".into());
        wb.escrever_celula(0, 0, 1, "=A1*2".into());

        // O parser CSV não participa do FSHZ: fórmulas e formato colunar
        // devem continuar sendo persistidos pelo codec nativo.
        let fshz = crate::da_workbook::pasta_gravar_bytes(&wb, 0).unwrap();
        let lida = crate::fshz_read::ler_fshz(&fshz).unwrap();
        let volta = crate::para_workbook::para_workbook(&lida);
        assert_eq!(volta.aba(0).unwrap().sheet.get_raw(0, 1), "=A1*2");

        // O caminho XLSX também continua usando o gravador/leitor próprios.
        let xlsx = crate::xlsx_write::gerar_xlsx(&wb).unwrap();
        let json = crate::xlsx_read::ler_xlsx_bytes(xlsx).unwrap();
        assert_eq!(json["sheets"][0]["name"], "Planilha1");
        assert_eq!(json["sheets"][0]["cells"]["0"]["raw"], "9");
    }

    #[test]
    fn csv_escapa_aspas_virgulas_linhas_e_valores_de_formula() {
        let mut livro = Workbook::new();
        livro.escrever_celula(0, 0, 0, "Nome".into());
        livro.escrever_celula(0, 0, 1, "Observação".into());
        livro.escrever_celula(0, 1, 0, "João".into());
        livro.escrever_celula(0, 1, 1, "Com, vírgula e \"aspas\"".into());
        livro.escrever_celula(0, 2, 0, "2".into());
        livro.escrever_celula(0, 2, 1, "=A3*10".into());
        livro.escrever_celula(0, 3, 0, "Ana\nSilva".into());
        let mut bytes = Vec::new();
        exportar_delimitado(&livro, 0, false, &mut bytes).unwrap();
        let texto = String::from_utf8(bytes).unwrap();
        assert_eq!(texto,
            "Nome,Observação\nJoão,\"Com, vírgula e \"\"aspas\"\"\"\n2,20\n\"Ana\nSilva\",");
        // A exportação não altera a fórmula original.
        assert_eq!(livro.aba(0).unwrap().sheet.get_raw(2, 1), "=A3*10");
    }

    #[test]
    fn tsv_cita_tab_sem_corromper_aspas_ou_quebras() {
        let mut livro = Workbook::new();
        livro.escrever_celula(0, 0, 0, "A\tB".into());
        livro.escrever_celula(0, 0, 1, "Sim".into());
        let mut bytes = Vec::new();
        exportar_delimitado(&livro, 0, true, &mut bytes).unwrap();
        assert_eq!(String::from_utf8(bytes).unwrap(), "\"A\tB\"\tSim");
    }

    #[test]
    fn csv_aba_vazia_e_aba_inexistente() {
        let livro=Workbook::new();
        let mut bytes=Vec::new();
        exportar_delimitado(&livro,0,false,&mut bytes).unwrap();
        assert!(bytes.is_empty());
        assert_eq!(exportar_delimitado(&livro,42,false,&mut bytes)
            .unwrap_err().kind(),std::io::ErrorKind::NotFound);
    }

    #[test]
    fn escrita_atomica_csv_e_tsv_preserva_workbook() {
        use std::sync::atomic::{AtomicU64,Ordering};
        static SEQ:AtomicU64=AtomicU64::new(0);
        let dir=std::env::temp_dir().join(format!(
            "csv-workbook-{}-{}",std::process::id(),SEQ.fetch_add(1,Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        let path=dir.join("arquivo.csv");
        let mut workbook=Workbook::new();
        workbook.escrever_celula(0,0,0,"Nome".into());
        workbook.escrever_celula(0,1,0,"Ana".into());
        gravar_csv_tsv(&workbook,0,&path,false).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(),"Nome\nAna");
        gravar_csv_tsv(&workbook,0,&path,true).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(),"Nome\nAna");
        assert_eq!(workbook.aba(0).unwrap().sheet.get_raw(1,0),"Ana");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(),1);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
