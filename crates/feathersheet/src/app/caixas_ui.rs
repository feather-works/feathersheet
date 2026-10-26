//! Inserir > Checkbox e o uso das caixas de seleção nas células: inserir,
//! marcar/desmarcar pelo clique ou pela barra de espaço. A regra e o desenho
//! ficam em `crate::caixas`; Delete e Limpar formatação estão em
//! `limpar_celula` e `Msg::LimparFormato`.
use super::*;
use crate::caixas::{self, Regras};

/// Teto de células escritas de uma vez (inserir com FALSO, espaço). Acima
/// disso a regra vale igual: célula vazia aparece desmarcada.
const MAX_CELULAS: u64 = 100_000;

fn tamanho(a: &Rect) -> u64 {
    (u64::from(a.r1) - u64::from(a.r0) + 1) * (u64::from(a.c1) - u64::from(a.c0) + 1)
}

impl App {
    /// A seleção encosta em alguma caixa? (Botão Checkbox aceso: remove.)
    pub(super) fn selecao_tem_caixa(&self) -> bool {
        self.workbook.aba(self.folha)
            .is_some_and(|a| Regras::da(&a.sheet).alguma_caixa_em(&self.area_selecionada().normalized()))
    }

    /// O botão Checkbox: sem caixa na seleção, insere; com caixa, remove.
    pub(super) fn alternar_botao_caixas(&mut self) {
        if self.selecao_tem_caixa() { self.remover_caixas(); } else { self.inserir_caixas(); }
    }

    /// Tira as caixas da seleção e limpa o VERDADEIRO/FALSO delas (o que o
    /// botão escreveu); outro conteúdo fica. Um Ctrl+Z.
    pub(super) fn remover_caixas(&mut self) {
        if !self.confirmar() { return; }
        let area = self.area_selecionada().normalized();
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        let regras = Regras::da(&aba.sheet);
        let Some(json) = regras.sem_caixas_em(&area) else { return };
        let usado = aba.sheet.used_rect();
        let limpar = Rect::new(area.r0, area.c0, area.r1.min(usado.r1), area.c1.min(usado.c1));
        self.workbook.editar(self.folha, |s| {
            s.abrir_transacao();
            if limpar.r0 <= limpar.r1 && limpar.c0 <= limpar.c1 && tamanho(&limpar) <= MAX_CELULAS {
                for r in limpar.r0..=limpar.r1 {
                    for c in limpar.c0..=limpar.c1 {
                        let Some((m, d)) = regras.caixa_em(r, c) else { continue };
                        if !s.get_raw(r, c).is_empty() && !s.get_raw(r, c).starts_with('=')
                            && caixas::estado(s, r, c, m, d).is_some() {
                            s.set_cell(r, c, "");
                        }
                    }
                }
            }
            s.alterar_validacoes(Some(json), &area);
            s.fechar_transacao();
        });
        self.alterado = true;
        self.status = tr!("Caixas de seleção removidas. Ctrl+Z desfaz.").into();
    }

    /// Caixas nas células selecionadas: a regra cobre a seleção e as vazias
    /// recebem FALSO; célula com outro conteúdo não é sobrescrita. Um Ctrl+Z.
    pub(super) fn inserir_caixas(&mut self) {
        if !self.confirmar() { return; }
        let area = self.area_selecionada().normalized();
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        let Some(json) = Regras::da(&aba.sheet).com_caixa(&aba.sheet, &area) else { return };
        let escrever = tamanho(&area) <= MAX_CELULAS;
        let mut com_conteudo = 0usize;
        self.workbook.editar(self.folha, |s| {
            s.abrir_transacao();
            s.alterar_validacoes(Some(json), &area);
            if escrever {
                for r in area.r0..=area.r1 {
                    for c in area.c0..=area.c1 {
                        match caixas::estado(s, r, c, caixas::MARCADO, caixas::DESMARCADO) {
                            None => com_conteudo += 1,
                            Some(_) if s.get_raw(r, c).is_empty() => s.set_cell(r, c, caixas::DESMARCADO),
                            Some(_) => {}
                        }
                    }
                }
            }
            s.fechar_transacao();
        });
        self.alterado = true;
        self.status = if com_conteudo > 0 {
            trf!("Caixas de seleção inseridas; {com_conteudo} célula(s) com outro conteúdo ficaram como estão. Ctrl+Z desfaz.")
        } else {
            tr!("Caixas de seleção inseridas: clique ou barra de espaço marca. Ctrl+Z desfaz.").into()
        };
    }

    /// Clique na caixa: seleciona a célula e troca marcado ↔ desmarcado.
    pub(super) fn alternar_caixa(&mut self, r: u32, c: u32) {
        if !self.confirmar() { return; }
        self.linha = r;
        self.coluna = c;
        self.fim_selecao = (r, c);
        self.selecoes_extras.clear();
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        let regras = Regras::da(&aba.sheet);
        let Some((m, d)) = regras.caixa_em(r, c) else { return };
        if aba.sheet.get_raw(r, c).starts_with('=') {
            self.status = tr!("A caixa mostra o resultado de uma fórmula: edite a fórmula para mudar.").into();
            return;
        }
        let Some(marcada) = caixas::estado(&aba.sheet, r, c, m, d) else { return };
        let novo = if marcada { d } else { m }.to_owned();
        self.workbook.escrever_celula(self.folha, r, c, novo.clone());
        self.edicao = novo;
        self.alterado = true;
        self.status = format!("{} {}", cell_name(r, c), if marcada { "desmarcada" } else { "marcada" });
    }

    /// Barra de espaço com caixas na seleção: se alguma está desmarcada,
    /// marca todas; senão desmarca todas (Excel). `false` = não há caixa
    /// na seleção, e o espaço segue como digitação.
    pub(super) fn alternar_caixas_da_selecao(&mut self) -> bool {
        let Some(aba) = self.workbook.aba(self.folha) else { return false };
        let regras = Regras::da(&aba.sheet);
        if !regras.tem_caixa() { return false; }
        let areas: Vec<Rect> = self.areas_selecionadas().into_iter().map(|a| a.normalized()).collect();
        if areas.iter().map(tamanho).sum::<u64>() > MAX_CELULAS { return false; }
        let mut alvos: Vec<(u32, u32, String, String, bool)> = Vec::new();
        for a in &areas {
            for r in a.r0..=a.r1 {
                for c in a.c0..=a.c1 {
                    let Some((m, d)) = regras.caixa_em(r, c) else { continue };
                    if aba.sheet.get_raw(r, c).starts_with('=') { continue; }
                    if let Some(marcada) = caixas::estado(&aba.sheet, r, c, m, d) {
                        alvos.push((r, c, m.to_owned(), d.to_owned(), marcada));
                    }
                }
            }
        }
        if alvos.is_empty() { return false; }
        let marcar = alvos.iter().any(|a| !a.4);
        self.workbook.editar(self.folha, |s| {
            s.abrir_transacao();
            for (r, c, m, d, marcada) in &alvos {
                if *marcada != marcar { s.set_cell(*r, *c, if marcar { m } else { d }); }
            }
            s.fechar_transacao();
        });
        self.alterado = true;
        self.status = trf!("{} caixa(s) {}", alvos.len(), if marcar { "marcada(s)" } else { "desmarcada(s)" });
        true
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::celulas::values::Value;

    fn selecionar(app: &mut App, r0: u32, c0: u32, r1: u32, c1: u32) {
        app.linha = r0;
        app.coluna = c0;
        app.fim_selecao = (r1, c1);
    }

    #[test]
    fn inserir_marcar_espaco_delete_e_limpar_formatacao() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 1, 0, "texto".into());
        selecionar(&mut app, 0, 0, 2, 0);
        let _ = app.update(Msg::CaixasInserir);
        let regras_de = |app: &App| Regras::da(&app.workbook.aba(0).unwrap().sheet);
        assert_eq!(app.workbook.valor(0, 0, 0), Value::Bool(false), "vazia vira FALSO");
        assert_eq!(app.workbook.valor(0, 1, 0), Value::Text("texto".into()), "conteúdo não é sobrescrito");
        let regras = regras_de(&app);
        assert!(regras.caixa_em(2, 0).is_some() && regras.caixa_em(3, 0).is_none());
        // Clique marca; fórmula lê VERDADEIRO.
        let _ = app.update(Msg::CaixaAlternar(2, 0));
        assert_eq!(app.workbook.valor(0, 2, 0), Value::Bool(true));
        app.workbook.escrever_celula(0, 0, 3, "=CONT.SE(A1:A3;VERDADEIRO)".into());
        assert_eq!(app.workbook.valor(0, 0, 3), Value::Number(1.0));
        // Espaço: há desmarcada (A1) → marca todas as caixas da seleção.
        selecionar(&mut app, 0, 0, 2, 0);
        assert!(app.alternar_caixas_da_selecao());
        assert_eq!(app.workbook.valor(0, 0, 0), Value::Bool(true));
        // De novo: todas marcadas → desmarca.
        assert!(app.alternar_caixas_da_selecao());
        assert_eq!(app.workbook.valor(0, 2, 0), Value::Bool(false));
        // Delete numa caixa só desmarca; a caixa continua.
        let _ = app.update(Msg::CaixaAlternar(0, 0));
        selecionar(&mut app, 0, 0, 0, 0);
        let _ = app.update(Msg::Limpar);
        assert_eq!(app.workbook.valor(0, 0, 0), Value::Bool(false));
        assert!(regras_de(&app).caixa_em(0, 0).is_some());
        // Limpar formatação tira a caixa só da seleção (o valor fica).
        let _ = app.update(Msg::LimparFormato);
        let regras = regras_de(&app);
        assert!(regras.caixa_em(0, 0).is_none() && regras.caixa_em(2, 0).is_some());
        assert_eq!(app.workbook.valor(0, 0, 0), Value::Bool(false));
        // Sem caixa na seleção, o espaço não é consumido.
        assert!(!app.alternar_caixas_da_selecao());
    }

    #[test]
    fn botao_de_novo_remove_e_delete_em_desmarcada_tira_a_caixa() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 1, 0, "texto".into());
        selecionar(&mut app, 0, 0, 2, 0);
        let _ = app.update(Msg::CaixasInserir);
        let _ = app.update(Msg::CaixaAlternar(2, 0));
        selecionar(&mut app, 0, 0, 2, 0);
        assert!(app.selecao_tem_caixa(), "botão aceso");
        let _ = app.update(Msg::CaixasInserir);
        assert!(!app.selecao_tem_caixa());
        assert_eq!(app.workbook.valor(0, 0, 0), Value::Empty, "FALSO some");
        assert_eq!(app.workbook.valor(0, 2, 0), Value::Empty, "VERDADEIRO some");
        assert_eq!(app.workbook.valor(0, 1, 0), Value::Text("texto".into()), "outro conteúdo fica");
        app.historico(false);
        assert!(app.selecao_tem_caixa());
        assert_eq!(app.workbook.valor(0, 2, 0), Value::Bool(true));
        // Delete: marcada → desmarca; já desmarcada → tira a caixa (Excel).
        selecionar(&mut app, 2, 0, 2, 0);
        let _ = app.update(Msg::Limpar);
        assert_eq!(app.workbook.valor(0, 2, 0), Value::Bool(false));
        let _ = app.update(Msg::Limpar);
        assert_eq!(app.workbook.valor(0, 2, 0), Value::Empty);
        assert!(!app.selecao_tem_caixa());
        selecionar(&mut app, 0, 0, 0, 0);
        assert!(app.selecao_tem_caixa(), "as outras caixas continuam");
    }

    #[test]
    fn inserir_e_um_desfazer_so() {
        let mut app = App::default();
        selecionar(&mut app, 0, 0, 3, 1);
        let _ = app.update(Msg::CaixasInserir);
        assert_eq!(app.workbook.valor(0, 3, 1), Value::Bool(false));
        app.historico(false);
        assert_eq!(app.workbook.valor(0, 3, 1), Value::Empty);
        assert!(!Regras::da(&app.workbook.aba(0).unwrap().sheet).tem_caixa());
    }
}
