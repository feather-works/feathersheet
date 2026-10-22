//! Atalhos de entrada do Excel: Ctrl+Enter (o texto digitado vai para a
//! seleção inteira) e Alt+↓ (escolher da lista os valores já digitados
//! na coluna).
use super::*;
use feathersheet_core::{celulas::values::Value, operacoes::fill::shift_formula};

/// Ctrl+Enter acima disto pede confirmação implícita pelo status: cada
/// célula é uma escrita e uma entrada do desfazer da transação.
const TETO_CTRL_ENTER: u64 = 1_000_000;
/// Alt+↓ lê no máximo isto de linhas acima e abaixo da célula ativa.
const TETO_LISTA_COLUNA: u32 = 100_000;

impl App {
    /// Escreve o conteúdo do editor em todas as células da seleção, num
    /// único Ctrl+Z. Fórmula tem as referências relativas deslocadas a
    /// partir da célula ativa, como o Excel faz. Devolve `false` quando a
    /// seleção é uma célula só (o chamador segue com o Enter comum).
    pub(super) fn preencher_selecao_com_edicao(&mut self) -> bool {
        if !self.editor_formula.ativo { return false; }
        let (folha, linha, coluna) = self.editor_formula.origem
            .unwrap_or((self.folha, self.linha, self.coluna));
        let areas = self.areas_selecionadas();
        let uma_celula = areas.len() == 1 && areas[0].r0 == areas[0].r1 && areas[0].c0 == areas[0].c1;
        if folha != self.folha || uma_celula { return false; }
        let celulas: u64 = areas.iter().map(|a|
            u64::from(a.r1 - a.r0 + 1) * u64::from(a.c1 - a.c0 + 1)).sum();
        if celulas > TETO_CTRL_ENTER {
            self.status = trf!(
                "Ctrl+Enter vale para até {} células; selecione uma área menor.",
                TETO_CTRL_ENTER);
            return true;
        }
        let texto = self.formula_para_gravar(&self.edicao, "");
        let formula = texto.starts_with('=');
        self.editor_formula.encerrar();
        self.edicao_inline = None;
        self.multilinha = None;
        self.workbook.editar(folha, |sheet| {
            sheet.abrir_transacao();
            for area in &areas {
                for r in area.r0..=area.r1 {
                    for c in area.c0..=area.c1 {
                        let bruto = if formula {
                            shift_formula(&texto, r as i32 - linha as i32, c as i32 - coluna as i32)
                        } else { texto.clone() };
                        sheet.set_cell(r, c, &bruto);
                    }
                }
            }
            sheet.fechar_transacao();
        });
        self.alterado = true;
        self.edicao = self.workbook.aba(folha)
            .map(|a| feathersheet_core::celulas::locale::apresentar_formula(
                &a.sheet.get_raw(self.linha, self.coluna), &self.regionalizacao()))
            .unwrap_or_default();
        self.status = trf!("{celulas} célula(s) preenchida(s) com Ctrl+Enter.");
        true
    }

    /// Alt+↓: valores distintos do bloco contínuo da coluna (acima e abaixo
    /// da célula ativa, até a primeira célula vazia), em ordem alfabética.
    /// Com validação de lista na célula, a lista dela tem preferência.
    pub(super) fn abrir_lista_da_coluna(&mut self) {
        if !self.confirmar() { return; }
        let (aba, linha, coluna) = (self.folha, self.linha, self.coluna);
        let texto = |r: u32| -> Option<String> {
            let v = self.workbook.valor(aba, r, coluna);
            (!matches!(v, Value::Empty)).then(|| feathersheet_core::celulas::values::to_text(&v))
        };
        let mut vistos = std::collections::BTreeSet::new();
        let mut r = linha;
        while r > 0 && linha - r < TETO_LISTA_COLUNA {
            let Some(t) = texto(r - 1) else { break };
            vistos.insert(t);
            r -= 1;
        }
        let mut r = linha;
        while r + 1 < MAX_ROWS && r - linha < TETO_LISTA_COLUNA {
            let Some(t) = texto(r + 1) else { break };
            vistos.insert(t);
            r += 1;
        }
        vistos.retain(|t| !t.trim().is_empty());
        if vistos.is_empty() {
            self.status = tr!("Não há valores acima ou abaixo nesta coluna para escolher.").into();
            return;
        }
        // Sem diferenciar maiúsculas na ordem, como a lista do Excel.
        let mut itens: Vec<String> = vistos.into_iter().collect();
        itens.sort_by_key(|t| t.to_lowercase());
        // A grade copia a lista a cada quadro; o Excel também corta a lista.
        itens.truncate(10_000);
        self.lista_coluna = Some(itens);
        self.lista_aberta = true;
        self.marcar_valor_atual_na_lista();
    }

    /// Itens da lista suspensa da célula ativa: a validação manda; sem ela,
    /// os valores da coluna (Alt+↓).
    pub(super) fn opcoes_lista_ativa(&self) -> Vec<String> {
        crate::grade::lista_suspensa::opcoes_validacao(&self.workbook, self.folha, self.linha, self.coluna)
            .or_else(|| self.lista_coluna.clone()).unwrap_or_default()
    }

    /// Ao abrir, marca e mostra o valor que já está na célula (como o Excel).
    pub(super) fn marcar_valor_atual_na_lista(&mut self) {
        let opcoes = self.opcoes_lista_ativa();
        let atual = self.workbook.aba(self.folha)
            .map(|a| a.sheet.get_raw(self.linha, self.coluna)).unwrap_or_default();
        self.lista_destaque = opcoes.iter().position(|o| *o == atual).unwrap_or(0);
        self.lista_pagina = crate::grade::lista_suspensa::rolar_ate(0, self.lista_destaque, opcoes.len());
    }

    /// Teclas com a lista aberta: ↑/↓, PageUp/PageDown, Home/End andam;
    /// Enter ou Tab escolhem; Esc fecha. `None` deixa a tecla para a grade.
    pub(super) fn tecla_na_lista(&mut self, key: &keyboard::Key) -> Option<Task<Msg>> {
        use crate::grade::lista_suspensa::{rolar_ate, VISIVEIS};
        use keyboard::key::Named;
        let keyboard::Key::Named(tecla) = key else { return None };
        let opcoes = self.opcoes_lista_ativa();
        if opcoes.is_empty() { return None; }
        let ultimo = opcoes.len() - 1;
        let d = self.lista_destaque.min(ultimo);
        let novo = match tecla {
            Named::ArrowDown => (d + 1).min(ultimo),
            Named::ArrowUp => d.saturating_sub(1),
            Named::PageDown => (d + VISIVEIS - 1).min(ultimo),
            Named::PageUp => d.saturating_sub(VISIVEIS - 1),
            Named::Home => 0,
            Named::End => ultimo,
            Named::Enter | Named::Tab => {
                return Some(self.update(Msg::ValidacaoListaEscolher(opcoes[d].clone())));
            }
            Named::Escape => {
                self.lista_aberta = false;
                self.lista_coluna = None;
                return Some(Task::none());
            }
            _ => return None,
        };
        self.lista_destaque = novo;
        self.lista_pagina = rolar_ate(self.lista_pagina, novo, opcoes.len());
        Some(Task::none())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn lista_suspensa_anda_rola_e_escolhe_pelo_teclado() {
        use feathersheet_core::operacoes::validation::{QuandoValidacao, Regra, Validacao};
        let mut app = App::default();
        let valores: Vec<String> = (1..=12).map(|i| format!("Item {i}")).collect();
        let regra = Validacao { ranges: vec![feathersheet_core::celulas::refs::Rect::cell(0, 0)],
            regra: Regra::Lista { valores }, quando: QuandoValidacao::Recusar, mensagem: None, mensagens: Default::default() };
        let json = serde_json::to_string(&vec![regra]).unwrap();
        app.workbook.editar(0, |s| s.va = Some(json));
        app.workbook.escrever_celula(0, 0, 0, "Item 3".into());
        let _ = app.update(Msg::ValidacaoListaAlternar);
        assert!(app.lista_aberta);
        assert_eq!(app.lista_destaque, 2, "abre marcando o valor atual");
        let tecla = |n| keyboard::Key::Named(n);
        for _ in 0..7 { app.tecla_na_lista(&tecla(keyboard::key::Named::ArrowDown)); }
        assert_eq!((app.lista_destaque, app.lista_pagina), (9, 2), "rola para manter o item à vista");
        app.tecla_na_lista(&tecla(keyboard::key::Named::Enter));
        assert!(!app.lista_aberta);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "Item 10");
        let _ = app.update(Msg::ValidacaoListaAlternar);
        app.tecla_na_lista(&tecla(keyboard::key::Named::Escape));
        assert!(!app.lista_aberta);
    }

    #[test]
    fn ctrl_enter_preenche_a_selecao_e_desloca_a_formula() {
        let mut app = App::default();
        let _ = app.update(Msg::IrPara("B1:C2".into()));
        app.editor_formula.ativo = true;
        app.editor_formula.origem = Some((0, 0, 1));
        app.edicao = "=A1*2".into();
        assert!(app.preencher_selecao_com_edicao());
        let bruto = |r, c| app.workbook.aba(0).unwrap().sheet.get_raw(r, c);
        assert_eq!(bruto(0, 1), "=A1*2");
        assert_eq!(bruto(1, 2), "=B2*2");
        let _ = app.update(Msg::Desfazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(1, 2), "");
    }

    #[test]
    fn ctrl_enter_numa_celula_so_segue_o_enter_comum() {
        let mut app = App::default();
        app.editor_formula.ativo = true;
        app.editor_formula.origem = Some((0, 0, 0));
        app.edicao = "x".into();
        assert!(!app.preencher_selecao_com_edicao());
    }

    #[test]
    fn alt_seta_lista_valores_distintos_do_bloco_da_coluna() {
        let mut app = App::default();
        for (r, t) in ["Rio", "são paulo", "Rio", "", "Fora"].iter().enumerate() {
            if !t.is_empty() { app.workbook.escrever_celula(0, r as u32, 0, (*t).into()); }
        }
        app.trocar_selecao(3, 0);
        app.abrir_lista_da_coluna();
        assert_eq!(app.lista_coluna, Some(vec![
            "Fora".to_string(), "Rio".to_string(), "são paulo".to_string()]));
        assert!(app.lista_aberta);
    }
}
