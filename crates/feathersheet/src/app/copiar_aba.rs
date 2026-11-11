//! Copiar a aba ativa para outro arquivo, novo ou existente, sem trocar o documento aberto
//! (o "Mover ou copiar › Para o livro" do Excel). A leitura e a gravação do destino rodam em
//! segundo plano.
use super::*;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum Evento {
    /// Abre a janela de arquivo (`true`: arquivo novo).
    Escolher(bool),
    Destino(Option<PathBuf>, bool),
    /// O nome que a aba ganhou no destino, ou o erro.
    Pronto(PathBuf, Result<String, String>),
}

impl App {
    pub(super) fn evento_copiar_aba(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Escolher(novo) => {
                self.contexto = None;
                if !self.confirmar() { return Task::none(); }
                let nome = format!("{}.fshz", self.workbook.nomes().get(self.folha).cloned().unwrap_or_default());
                Task::perform(async move {
                    let janela = rfd::AsyncFileDialog::new().set_directory(dialogos::pasta_padrao());
                    let escolhido = if novo {
                        janela.add_filter("FeatherSheet", &["fshz"]).add_filter("Excel", &["xlsx"])
                            .set_file_name(&nome).save_file().await
                    } else {
                        janela.add_filter(tr!("Planilhas"), &["fshz", "xlsx", "xlsm"]).pick_file().await
                    };
                    escolhido.map(|f| f.path().to_path_buf())
                }, move |p| Msg::CopiarAba(Evento::Destino(p, novo)))
            }
            Evento::Destino(None, _) => Task::none(),
            Evento::Destino(Some(path), novo) => {
                let path = if novo && documento_operacoes::formato_excel(&path).is_none() {
                    match documento_operacoes::com_extensao_fshz(path) {
                        Ok(p) => p,
                        Err(e) => { self.status = e; return Task::none(); }
                    }
                } else { path };
                if self.caminho.as_ref() == Some(&path) {
                    self.status = tr!("Para copiar dentro deste arquivo, use Duplicar aba.").into();
                    return Task::none();
                }
                let Some(aba) = self.workbook.aba(self.folha) else { return Task::none() };
                let mut so_a_aba = Workbook::vazia();
                so_a_aba.receber_aba(&aba.sheet);
                self.status = tr!("Copiando a aba…").into();
                let destino = path.clone();
                Task::perform(async move {
                    tokio::task::spawn_blocking(move || copiar_para(so_a_aba, &destino, novo)).await
                        .unwrap_or_else(|e| Err(e.to_string()))
                }, move |r| Msg::CopiarAba(Evento::Pronto(path.clone(), r)))
            }
            Evento::Pronto(path, resultado) => {
                let arquivo = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                self.status = match resultado {
                    Ok(nome) => trf!("Aba copiada para {arquivo} como {nome}."),
                    Err(e) => trf!("Não deu para copiar a aba para {arquivo}: {e}"),
                };
                Task::none()
            }
        }
    }
}

/// Grava a aba (a única de `so_a_aba`) em `path`: arquivo novo só com ela, ou acrescentada
/// no fim do arquivo que existe. Devolve o nome que ela ganhou lá.
fn copiar_para(so_a_aba: Workbook, path: &Path, novo: bool) -> Result<String, String> {
    let mut alvo = if novo { so_a_aba } else {
        if senha_abrir::tem_senha(path) {
            return Err(tr!("o arquivo tem senha; abra-o e cole a aba nele").into());
        }
        // Regravar o .xlsx apagaria o que o FeatherSheet não converte (segmentação, Power Pivot…).
        if documento_operacoes::formato_excel(path).is_some() {
            let lista = std::fs::read(path).map_err(|e| e.to_string())
                .and_then(feathersheet_io::xlsx_read::nao_convertidos::nao_convertidos)?;
            if !lista.is_empty() {
                let itens = lista.iter().map(super::monitor::descrever).collect::<Vec<_>>().join("; ");
                return Err(trf!("regravar o arquivo apagaria dele: {itens}"));
            }
        }
        let (mut alvo, _) = App::decodificar_caminho(path, None)?;
        let origem = so_a_aba.aba(0).ok_or_else(|| "aba vazia".to_string())?;
        alvo.receber_aba(&origem.sheet);
        alvo
    };
    alvo.calcular_em_paralelo();
    let nome = alvo.nomes().last().cloned().unwrap_or_default();
    documento_operacoes::gravar_retrato(&feathersheet_io::da_workbook::da_workbook(&alvo, 0), path, None)?;
    Ok(nome)
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::celulas::values::Value;

    #[test]
    fn aba_vai_para_arquivo_existente_e_para_arquivo_novo() {
        let pasta = std::env::temp_dir().join(format!("fs_copiar_aba_{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let mut origem = Workbook::vazia();
        let i = origem.criar_aba("Dados");
        origem.aba_mut(i).unwrap().sheet.set_cell(0, 0, "21");
        origem.aba_mut(i).unwrap().sheet.set_cell(0, 1, "=A1*2");
        let so = || { let mut w = Workbook::vazia(); w.receber_aba(&origem.aba(0).unwrap().sheet); w };

        // O destino já tem uma aba "Dados": a copiada vira "Dados (2)", no fim.
        let existente = pasta.join("destino.fshz");
        let mut destino = Workbook::vazia();
        let d = destino.criar_aba("Dados");
        destino.aba_mut(d).unwrap().sheet.set_cell(0, 0, "x");
        documento_operacoes::gravar_retrato(&feathersheet_io::da_workbook::da_workbook(&destino, 0), &existente, None).unwrap();
        assert_eq!(copiar_para(so(), &existente, false).as_deref(), Ok("Dados (2)"));
        let (volta, _) = App::decodificar_caminho(&existente, None).unwrap();
        assert_eq!(volta.nomes(), vec!["Dados".to_string(), "Dados (2)".to_string()]);
        assert_eq!(volta.valor(0, 0, 0), Value::Text("x".into()), "a aba que já estava fica");
        assert_eq!(volta.valor(1, 0, 1), Value::Number(42.0), "a fórmula veio e calcula");

        let novo = pasta.join("novo.xlsx");
        assert_eq!(copiar_para(so(), &novo, true).as_deref(), Ok("Dados"));
        let (volta, _) = App::decodificar_caminho(&novo, None).unwrap();
        assert_eq!(volta.nomes(), vec!["Dados".to_string()]);
        assert_eq!(volta.valor(0, 0, 1), Value::Number(42.0));

        // Um .xlsx com senha não é regravado.
        let protegido = pasta.join("senha.xlsx");
        std::fs::write(&protegido, include_bytes!("../../../feathersheet-io/tests/arquivos/senha123_libreoffice.xlsx")).unwrap();
        assert!(copiar_para(so(), &protegido, false).is_err());
        let _ = std::fs::remove_dir_all(&pasta);
    }
}
