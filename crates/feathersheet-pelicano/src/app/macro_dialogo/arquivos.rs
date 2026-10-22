//! Os arquivos que a macro pediu (`sheet.exportPdf`, `wb.saveSheetsAs`, o
//! `ExportAsFixedFormat` e o `Workbooks.Add` + `SaveAs` do VBA). A macro só
//! anota os pedidos; quem grava é o aplicativo, e só quando ela terminou bem.
//! Cada arquivo gravado deixa uma linha no Console (F12), direto no monitor
//! do app: quem grava é a interface, não o motor (o `diario` é do motor).
use super::*;
use feathersheet_core::celulas::refs::{key_col, key_row};
use feathersheet_core::celulas::values::Value;
use feathersheet_core::planilha::impressao::{AreaImpressao, ConfigPagina};
use feathersheet_macros::PedidoArquivo;
use std::time::Instant;

/// O PDF da aba: a área de impressão dela, senão a parte usada.
fn pdf_da_aba(wb: &Workbook, indice: usize, area: Option<Rect>) -> Result<Vec<u8>, String> {
    let aba = wb.aba(indice).ok_or(tr!("Aba inexistente"))?;
    let f = area.unwrap_or_else(|| {
        let u = aba.sheet.used_rect();
        Rect::new(0, 0, u.r1, u.c1)
    }).normalized();
    let area = AreaImpressao { linha0: f.r0, coluna0: f.c0, linha1: f.r1, coluna1: f.c1 };
    feathersheet_io::impressao_pdf::gerar(wb, indice, area, &ConfigPagina::default())
}

/// Uma pasta nova com cópias destas abas, na ordem. Fórmula que lia uma aba
/// que não veio junto daria erro no arquivo novo: vai o valor que ela tinha
/// (o Excel deixaria um vínculo para o arquivo de origem).
fn pasta_com_abas(wb: &Workbook, indices: &[usize]) -> Workbook {
    let mut nova = Workbook::vazia();
    for &i in indices {
        let Some(aba) = wb.aba(i) else { continue };
        let j = nova.criar_aba(aba.sheet.name.clone());
        if let Some(viva) = nova.aba_mut(j) { viva.sheet = aba.sheet.duplicar(aba.sheet.name.clone()); }
    }
    for (j, &i) in indices.iter().enumerate() {
        let Some(aba) = wb.aba(i) else { continue };
        let tudo = Rect::new(0, 0, u32::MAX - 1, u32::MAX - 1);
        let quebradas: Vec<(u32, u32, Value)> = aba.sheet.chaves_no_retangulo(&tudo).into_iter()
            .map(|k| (key_row(k), key_col(k)))
            .filter(|&(r, c)| aba.sheet.is_formula(r, c))
            .filter_map(|(r, c)| {
                let antes = wb.valor(i, r, c);
                let agora = nova.valor(j, r, c);
                (matches!(agora, Value::Error(_)) && !matches!(antes, Value::Error(_))).then_some((r, c, antes))
            })
            .collect();
        if quebradas.is_empty() { continue; }
        nova.editar(j, |s| {
            for (r, c, v) in quebradas {
                let estilo = s.snapshot(r, c).and_then(|x| x.style);
                s.apply(r, c, None, false);
                s.set_cell_valor(r, c, v);
                if let Some(e) = estilo { s.set_style(&Rect::new(r, c, r, c), &e); }
            }
        });
    }
    nova
}

/// Os bytes do arquivo novo, pelo formato da extensão (`.xlsx` ou `.fshz`).
fn bytes_da_pasta(nova: &Workbook, caminho: &Path) -> Result<Vec<u8>, String> {
    let ext = caminho.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "xlsx" => feathersheet_io::xlsx_write::gerar_xlsx(nova),
        "fshz" => feathersheet_io::da_workbook::pasta_gravar_bytes(nova, 0),
        _ => Err(trf!("Formato .{ext} não suportado (use .xlsx ou .fshz)")),
    }
}

fn gravar(caminho: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(pasta) = caminho.parent().filter(|p| !p.as_os_str().is_empty() && !p.is_dir()) {
        return Err(trf!("a pasta {} não existe", pasta.display()));
    }
    feathersheet_io::arquivo::gravar_atomico(caminho, bytes).map_err(|e| e.to_string())
}

impl App {
    /// Grava os arquivos pedidos pela macro, na ordem. Devolve as linhas de
    /// saída (o que foi gravado) e as falhas, que a pessoa vê numa mensagem.
    pub(super) fn gravar_arquivos_da_macro(&mut self, pedidos: &[PedidoArquivo]) -> (Vec<String>, Vec<String>) {
        let (mut gravados, mut falhas) = (Vec::new(), Vec::new());
        for pedido in pedidos {
            let t = Instant::now();
            let (tipo, caminho, bytes) = match pedido {
                PedidoArquivo::Pdf { aba, caminho } => {
                    let bytes = self.workbook.indice_do_id(*aba).ok_or_else(|| tr!("a aba não existe mais").to_string())
                        .and_then(|i| pdf_da_aba(&self.workbook, i, self.area_impressao_da_aba(*aba)));
                    ("PDF", caminho, bytes)
                }
                PedidoArquivo::Abas { abas, caminho } => {
                    let indices: Option<Vec<usize>> = abas.iter().map(|id| self.workbook.indice_do_id(*id)).collect();
                    let bytes = indices.ok_or_else(|| tr!("uma das abas não existe mais").to_string())
                        .and_then(|ix| bytes_da_pasta(&pasta_com_abas(&self.workbook, &ix), caminho));
                    ("abas", caminho, bytes)
                }
            };
            match bytes.and_then(|b| gravar(caminho, &b).map(|()| b.len())) {
                Ok(n) => {
                    let linha = format!("[macro] {tipo} gravado: {} · {} KB · {} ms",
                        caminho.display(), n.div_ceil(1024), t.elapsed().as_millis());
                    self.monitor.registrar(feather_ui::monitor::Nivel::Info, tr!("Macros"), &linha);
                    gravados.push(linha);
                }
                Err(e) => {
                    self.monitor.registrar(feather_ui::monitor::Nivel::Erro, tr!("Macros"),
                        &trf!("[macro] falhou ao gravar {}: {e}", caminho.display()));
                    falhas.push(trf!("Não foi possível gravar {}: {e}", caminho.display()));
                }
            }
        }
        (gravados, falhas)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pasta_temporaria(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("feathersheet-macro-arquivos-{}-{nome}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn macro_grava_pdf_e_xlsx_com_caminho_relativo() {
        let dir = pasta_temporaria("relativo");
        let mut app = App::default();
        app.caminho = Some(dir.join("Origem.fshz"));
        app.workbook.inserir_aba("Dados", 1);
        app.workbook.escrever_celula(1, 0, 0, "10".into());
        app.workbook.escrever_celula(1, 1, 0, "=A1*2".into());
        // Lê outra aba que não vai junto: no arquivo novo fica o valor.
        app.workbook.escrever_celula(0, 0, 0, "7".into());
        app.workbook.escrever_celula(1, 2, 0, "=Planilha1!A1+1".into());
        let id_dados = app.workbook.aba(1).unwrap().id;
        app.definir_area_impressao_da_aba(id_dados, Rect::new(0, 0, 1, 0));
        let r = app.rodar_macro(historico_macros::GatilhoMacro::manual("teste"), "M", r#"
            wb.sheet("Dados").exportPdf("saida\\relatorio");
            wb.saveSheetsAs(["Dados"], "abas.xlsx");
            wb.sheet("Dados").saveAs("abas.fshz");
        "#, vec![]).unwrap();
        assert!(r.erro.is_none(), "{:?}", r.erro);
        assert!(app.macro_mensagens.iter().any(|m| m.contains("saida")), "pasta inexistente vira mensagem: {:?}", app.macro_mensagens);
        assert!(!dir.join("saida/relatorio.pdf").exists());

        std::fs::create_dir_all(dir.join("saida")).unwrap();
        app.macro_mensagens.clear();
        let r = app.rodar_macro(historico_macros::GatilhoMacro::manual("teste"), "M", r#"wb.sheet("Dados").exportPdf("saida\\relatorio");"#, vec![]).unwrap();
        assert!(r.erro.is_none() && app.macro_mensagens.is_empty(), "{:?} {:?}", r.erro, app.macro_mensagens);
        let pdf = std::fs::read(dir.join("saida/relatorio.pdf")).unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        assert!(r.saida.iter().any(|l| l.contains("[macro] PDF gravado")), "{:?}", r.saida);
        assert!(app.monitor.console.registros().any(|l| l.mensagem.contains("[macro] PDF gravado")));

        let xlsx = std::fs::read(dir.join("abas.xlsx")).unwrap();
        // Lido pelo zip (o leitor de .xlsx anota no diário global do motor,
        // que os testes do Console leem em paralelo).
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(xlsx)).unwrap();
        let mut ler = |nome: &str| {
            let mut t = String::new();
            std::io::Read::read_to_string(&mut zip.by_name(nome).unwrap(), &mut t).unwrap();
            t
        };
        assert!(ler("xl/workbook.xml").contains("name=\"Dados\""));
        let aba = ler("xl/worksheets/sheet1.xml");
        assert!(aba.contains("<f>A1*2</f>"), "{aba}");
        // A fórmula que lia outra aba vai como valor (7 + 1).
        assert!(!aba.contains("Planilha1") && aba.contains("<v>8</v>"), "{aba}");
        assert!(std::fs::read(dir.join("abas.fshz")).unwrap().len() > 4);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn macro_com_erro_nao_grava_arquivo() {
        let dir = pasta_temporaria("erro");
        let mut app = App::default();
        app.caminho = Some(dir.join("Origem.fshz"));
        let r = app.rodar_macro(historico_macros::GatilhoMacro::manual("teste"), "M", r#"sheet.exportPdf("a.pdf"); throw new Error("x");"#, vec![]).unwrap();
        assert!(r.erro.is_some());
        assert!(!dir.join("a.pdf").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
