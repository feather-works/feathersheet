//! Ctrl+Z de inserir/excluir linhas e colunas (#65). Antes da operação, a pasta inteira vira uma
//! foto `.fshz` na memória e o histórico de cada aba é guardado junto (a operação o esqueceria:
//! um passo velho cairia na célula errada). Ctrl+Z primeiro desfaz o que veio depois, na própria
//! aba; quando ela não tem mais nada, volta a foto e os históricos de antes, e o Ctrl+Z segue.
use super::*;
use feathersheet_core::planilha::sheet::edicao::Historico;

/// Acima disto de células na área usada da pasta, a foto pesaria demais (tempo e memória).
// shortcut: pasta maior não desfaz inserir/excluir (avisa no status); guardar só a fatia tirada
// (um passo de estrutura no motor) resolveria também para ela.
const TETO_CELULAS_FOTO: u64 = 5_000_000;
/// No máximo isto de fotos guardadas; as mais velhas saem.
const MAX_FOTOS: usize = 20;

pub(super) struct FotoEstrutura {
    folha: usize,
    bytes: Vec<u8>,
    historicos: Vec<Historico>,
    /// A versão de cada aba logo depois de guardar: outra aba mexida depois não deixa voltar.
    versoes: Vec<u32>,
    /// `edicoes_novas` da aba da operação ao guardar: o refazer exige que nada novo tenha
    /// entrado depois (desfazer e refazer edições antigas, pode).
    novas: u64,
}

/// A foto tirada antes de inserir/excluir; `None` quando a pasta é grande demais.
pub(super) type FotoAntes = Option<(Vec<u8>, Vec<Historico>)>;

impl App {
    fn versoes_das_abas(&self) -> Vec<u32> {
        (0..self.workbook.nomes().len()).filter_map(|i| self.workbook.aba(i).map(|a| a.versao)).collect()
    }

    fn edicoes_novas_da_aba(&self) -> u64 {
        self.workbook.aba(self.folha).map(|a| a.sheet.edicoes_novas()).unwrap_or(0)
    }

    fn tirar_historicos(&mut self) -> Vec<Historico> {
        (0..self.workbook.nomes().len())
            .map(|i| self.workbook.aba_mut(i).map(|a| a.sheet.tirar_historico()).unwrap_or_default()).collect()
    }

    fn por_historicos(&mut self, historicos: Vec<Historico>) {
        for (i, h) in historicos.into_iter().enumerate() {
            if let Some(a) = self.workbook.aba_mut(i) { a.sheet.por_historico(h); }
        }
    }

    /// Chamar logo antes de inserir/excluir linhas ou colunas.
    pub(super) fn foto_antes_da_estrutura(&mut self) -> FotoAntes {
        let celulas: u64 = (0..self.workbook.nomes().len()).filter_map(|i| self.workbook.aba(i)).map(|a| {
            let r = a.sheet.used_rect();
            u64::from(r.r1.saturating_sub(r.r0) + 1) * u64::from(r.c1.saturating_sub(r.c0) + 1)
        }).sum();
        if celulas > TETO_CELULAS_FOTO { return None; }
        let bytes = feathersheet_io::da_workbook::pasta_gravar_bytes(&self.workbook, self.folha as u32).ok()?;
        Some((bytes, self.tirar_historicos()))
    }

    /// Chamar depois, com o que a operação devolveu (`feito`). Sem foto, avisa no status.
    pub(super) fn guardar_foto_da_estrutura(&mut self, foto: FotoAntes, feito: bool) {
        let Some((bytes, historicos)) = foto else {
            if feito { self.status = format!("{} · {}", self.status, tr!("pasta grande: isto não se desfaz com Ctrl+Z")); }
            return;
        };
        if !feito { self.por_historicos(historicos); return; }
        self.refazer_estrutura.clear();
        let novas = self.edicoes_novas_da_aba();
        self.desfazer_estrutura.push(FotoEstrutura { folha: self.folha, bytes, historicos, versoes: self.versoes_das_abas(), novas });
        if self.desfazer_estrutura.len() > MAX_FOTOS { self.desfazer_estrutura.remove(0); }
    }

    /// Ctrl+Z/Ctrl+Y de uma inserção/exclusão, quando a aba ativa já não tem nada no próprio
    /// histórico. `true` se trocou a pasta.
    pub(super) fn historico_estrutura(&mut self, refazer: bool) -> bool {
        let pilha = if refazer { &self.refazer_estrutura } else { &self.desfazer_estrutura };
        let Some(op) = pilha.last() else { return false };
        if op.folha != self.folha { return false; }
        let agora = self.versoes_das_abas();
        // A aba da operação pode ter mudado (edições desfeitas, ou desfeitas e refeitas); as
        // outras, não. No refazer, nenhuma edição nova nela: a foto passaria por cima.
        let valida = agora.len() == op.versoes.len()
            && agora.iter().zip(&op.versoes).enumerate().all(|(i, (a, b))| a == b || i == op.folha)
            && (!refazer || self.edicoes_novas_da_aba() == op.novas);
        let livre = self.workbook.aba(self.folha)
            .is_some_and(|a| if refazer { !a.sheet.can_redo() } else { !a.sheet.can_undo() });
        if !valida || !livre { return false; }
        let Ok(atual) = feathersheet_io::da_workbook::pasta_gravar_bytes(&self.workbook, self.folha as u32) else { return false };
        let foto = match feathersheet_io::fshz_read::ler_fshz(&op.bytes) {
            Ok(lida) => feathersheet_io::para_workbook::para_workbook_soltando(lida),
            Err(e) => { self.status = trf!("Não deu para desfazer: {e}"); return true; }
        };
        let op = if refazer { self.refazer_estrutura.pop() } else { self.desfazer_estrutura.pop() }.expect("visto acima");
        let historicos_atuais = self.tirar_historicos();
        if !self.workbook.restaurar_foto(foto) {
            self.por_historicos(historicos_atuais);
            return false;
        }
        self.por_historicos(op.historicos);
        let oposta = FotoEstrutura { folha: op.folha, bytes: atual, historicos: historicos_atuais,
            versoes: self.versoes_das_abas(), novas: self.edicoes_novas_da_aba() };
        if refazer { self.desfazer_estrutura.push(oposta) } else { self.refazer_estrutura.push(oposta) }
        self.alterado = true;
        self.invalidar_cache_objetos();
        self.recarregar_grafico_aberto();
        self.trocar_selecao(self.linha, self.coluna);
        self.status = if refazer { tr!("Refazer") } else { tr!("Desfazer") }.into();
        true
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn bruto(app: &App, aba: usize, r: u32, c: u32) -> String {
        app.workbook.aba(aba).unwrap().sheet.get_raw(r, c)
    }

    /// A1:A3 = 10, 20, 30 e, na aba Resumo, =Planilha1!A2*2.
    fn app_com_dados() -> App {
        let mut app = App::default();
        for (r, v) in ["10", "20", "30"].iter().enumerate() { app.workbook.escrever_celula(0, r as u32, 0, (*v).into()); }
        let resumo = app.workbook.inserir_aba("Resumo", 1);
        app.workbook.escrever_celula(resumo, 0, 0, "=Planilha1!A2*2".into());
        app
    }

    #[test]
    fn excluir_linha_desfaz_com_a_formula_da_outra_aba_e_o_historico_de_antes() {
        let mut app = app_com_dados();
        // Uma edição antes: o Ctrl+Z dela tem de continuar valendo depois.
        let _ = app.update(Msg::IrPara("C1".into()));
        let _ = app.update(Msg::Digitar("antes".into()));
        let _ = app.update(Msg::EnviarEditor);
        app.trocar_selecao(1, 0);
        app.estrutura(Estrutura::ExcluirLinha);
        assert_eq!(bruto(&app, 0, 1, 0), "30");
        assert!(bruto(&app, 1, 0, 0).contains("#REF!"), "{}", bruto(&app, 1, 0, 0));
        // Uma edição depois: o 1º Ctrl+Z é dela.
        let _ = app.update(Msg::IrPara("D5".into()));
        let _ = app.update(Msg::Digitar("depois".into()));
        let _ = app.update(Msg::EnviarEditor);
        assert_eq!(bruto(&app, 0, 4, 3), "depois");
        let _ = app.update(Msg::Desfazer);
        assert_eq!(bruto(&app, 0, 4, 3), "");
        assert_eq!(bruto(&app, 0, 1, 0), "30", "ainda sem a linha");
        // O 2º volta a linha e a fórmula da outra aba.
        let _ = app.update(Msg::Desfazer);
        assert_eq!((bruto(&app, 0, 0, 0), bruto(&app, 0, 1, 0), bruto(&app, 0, 2, 0)), ("10".into(), "20".into(), "30".into()));
        assert_eq!(bruto(&app, 1, 0, 0), "=Planilha1!A2*2");
        // O 3º é a edição de antes.
        assert_eq!(bruto(&app, 0, 0, 2), "antes");
        let _ = app.update(Msg::Desfazer);
        assert_eq!(bruto(&app, 0, 0, 2), "");
        // Ctrl+Y refaz na ordem: a edição de antes, a exclusão, a edição de depois.
        let _ = app.update(Msg::Refazer);
        assert_eq!(bruto(&app, 0, 0, 2), "antes");
        let _ = app.update(Msg::Refazer);
        assert_eq!(bruto(&app, 0, 1, 0), "30");
        let _ = app.update(Msg::Refazer);
        assert_eq!(bruto(&app, 0, 4, 3), "depois");
    }

    #[test]
    fn inserir_linha_desfaz_e_a_aba_continua_a_mesma() {
        let mut app = app_com_dados();
        let id = app.workbook.aba(0).unwrap().id;
        app.trocar_selecao(0, 0);
        app.inserir_menu(Estrutura::InserirLinha, false);
        assert_eq!(bruto(&app, 0, 1, 0), "10");
        assert_eq!(bruto(&app, 1, 0, 0), "=Planilha1!A3*2");
        let _ = app.update(Msg::Desfazer);
        assert_eq!(bruto(&app, 0, 0, 0), "10");
        assert_eq!(bruto(&app, 1, 0, 0), "=Planilha1!A2*2");
        assert_eq!(app.workbook.aba(0).unwrap().id, id, "o id da aba não muda (congelamento é por id)");
    }

    #[test]
    fn edicao_nova_depois_do_desfazer_corta_o_refazer() {
        let mut app = app_com_dados();
        app.trocar_selecao(1, 0);
        app.estrutura(Estrutura::ExcluirLinha);
        let _ = app.update(Msg::Desfazer);
        assert_eq!(bruto(&app, 0, 1, 0), "20");
        let _ = app.update(Msg::IrPara("E1".into()));
        let _ = app.update(Msg::Digitar("nova".into()));
        let _ = app.update(Msg::EnviarEditor);
        let _ = app.update(Msg::Refazer);
        assert_eq!(bruto(&app, 0, 0, 4), "nova", "o Ctrl+Y não passa por cima da edição nova");
        assert_eq!(bruto(&app, 0, 1, 0), "20");
    }

    #[test]
    fn outra_aba_mexida_depois_nao_deixa_desfazer_por_cima() {
        let mut app = app_com_dados();
        app.trocar_selecao(0, 0);
        app.estrutura(Estrutura::InserirLinha);
        app.workbook.escrever_celula(1, 5, 5, "novo".into());
        assert!(!app.historico_estrutura(false));
        assert_eq!(bruto(&app, 0, 1, 0), "10", "nada voltou");
        assert_eq!(bruto(&app, 1, 5, 5), "novo", "a edição da outra aba ficou");
    }
}

#[cfg(test)]
mod medida {
    use super::*;

    /// `cargo test medir_foto_da_estrutura -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn medir_foto_da_estrutura() {
        let mut app = App::default();
        let linhas = 200_000u32;
        app.workbook.editar(0, |s| {
            for r in 0..linhas {
                s.set_cell(r, 0, &r.to_string());
                s.set_cell(r, 1, &format!("texto {}", r % 500));
                s.set_cell(r, 2, &format!("=A{}*2", r + 1));
            }
        });
        let t = std::time::Instant::now();
        app.trocar_selecao(10, 0);
        app.estrutura(Estrutura::InserirLinha);
        let inserir = t.elapsed();
        let t = std::time::Instant::now();
        let _ = app.update(Msg::Desfazer);
        println!("{linhas} linhas × 3: inserir com foto {inserir:?}, desfazer {:?}, foto {} KB",
            t.elapsed(), app.refazer_estrutura.last().map(|f| f.bytes.len() / 1024).unwrap_or(0));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(10, 0), "10");
    }
}
