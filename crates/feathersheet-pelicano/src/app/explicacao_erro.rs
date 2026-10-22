//! Painel de diagnóstico de erros de fórmula, migrado do error-explainer.ts.
//! A explicação é recalculada sob demanda; não altera o workbook.
use super::*;
use pelicano::column;
use feathersheet_core::formula::explicar::{explicar_erro, Explicacao};

impl App {
    /// Diálogo "Diagnóstico de fórmula": tamanho pelo conteúdo (sem altura
    /// fixa), no componente comum `feather_ui::dialogo`.
    pub(super) fn view_explicacao_erro(&self) -> Option<super::dialogo_interno::Dialogo<'_, Msg>> {
        use super::dialogo_interno::dlg;
        let (aba, linha, coluna) = self.erro_explicando?;
        let referencia = cell_name(linha, coluna);
        let semibold = pelicano::Font { weight: 600, ..Default::default() };
        let explicacao = explicar_erro(&self.workbook, aba, linha, coluna);
        let mut conteudo = pelicano::column(Vec::<Element<_>>::new()).spacing(12);
        if let Some(Explicacao { code, meaning, summary, steps, hint }) = explicacao {
            conteudo = conteudo
                .push(row![
                    text(code).size(18).font(semibold).color(COR_ERRO()),
                    text(meaning).size(13).color(COR_TEXTO()),
                ].spacing(10).align(pelicano::Align::Center))
                .push(text(summary).size(12.5).line_height(1.4))
                .push(dlg::secao(tr!("Caminho do erro")));
            let mut passos = pelicano::column(Vec::<Element<_>>::new()).spacing(6);
            for (indice, passo) in steps.iter().enumerate() {
                let mut detalhe = column![
                    text(format!("{}. {}", indice + 1, passo.cell)).size(13).font(semibold),
                    text(trf!("Fórmula: {}", passo.formula)).size(12).font(pelicano::Font::MONO)
                        .color(COR_TEXTO_MUTED()),
                    text(trf!("Trecho: {}", passo.culprit)).size(12).color(COR_TEXTO_MUTED()),
                ].spacing(3);
                if let Some(onde) = &passo.onde {
                    detalhe = detalhe.push(text(onde.clone()).size(12).color(COR_TEXTO_MUTED()));
                }
                passos = passos.push(container(
                    row![
                        detalhe.width(Length::FILL),
                        button(row![feather_ui::icones::icone("chevron-right", 14.0), text(tr!("Ir para")).size(12.5)]
                            .spacing(4).align(pelicano::Align::Center))
                            .padding([5, 10])
                            .on_press(Msg::ExplicacaoErroIrPara(indice))
                            .style(estilo_botao_secundario),
                    ].align(pelicano::Align::Center).spacing(10)
                ).padding([8, 12]).width(Length::FILL).style(dlg::estilo_bloco));
            }
            conteudo = conteudo
                .push(passos)
                .push(dlg::secao(tr!("Como corrigir")))
                .push(text(hint).size(12.5).line_height(1.4));
        } else {
            conteudo = conteudo.push(dlg::aviso_vazio(
                tr!("Esta célula não contém um erro de fórmula no momento.")));
        }
        Some(self.dialogo(tr!("Diagnóstico de fórmula"), conteudo)
            .icone("bug")
            .subtitulo(trf!("Célula {referencia}"))
            .largura(560.0)
            .ao_fechar(Msg::ExplicacaoErroFechar))
    }

    pub(super) fn sobrepor_explicacao_erro<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        match self.view_explicacao_erro() {
            Some(d) => d.sobre(base),
            None => base,
        }
    }

    pub(super) fn ir_para_passo_erro(&mut self, indice: usize) {
        let Some((aba, linha, coluna)) = self.erro_explicando else { return };
        let Some(explicacao) = explicar_erro(&self.workbook, aba, linha, coluna) else {
            return;
        };
        let Some(passo) = explicacao.steps.get(indice) else { return };
        let Some((destino_aba, linha_alvo, coluna_alvo)) =
            destino_passo(&self.workbook, aba, &passo.cell)
        else { return };
        if !self.confirmar() { return; }
        self.folha = destino_aba;
        self.nome_aba = self.workbook.nomes().get(destino_aba).cloned().unwrap_or_default();
        self.trocar_selecao(linha_alvo, coluna_alvo);

        self.erro_explicando = None;
    }
}

/// Resolve endereços A1 e 'Nome com ! ou aspas'' duplicadas'!B7.
/// O separador ! fora de aspas é o único delimitador de aba válido.
fn destino_passo(
    workbook: &feathersheet_core::planilha::workbook::Workbook,
    origem: usize,
    referencia: &str,
) -> Option<(usize, u32, u32)> {
    let mut em_aspas = false;
    let mut separador = None;
    let mut iter = referencia.char_indices().peekable();
    while let Some((indice, caractere)) = iter.next() {
        if caractere == '\'' {
            if em_aspas && iter.peek().is_some_and(|(_, c)| *c == '\'') {
                iter.next();
            } else {
                em_aspas = !em_aspas;
            }
        } else if caractere == '!' && !em_aspas {
            separador = Some(indice);
        }
    }
    if em_aspas { return None; }
    let (aba, celula) = if let Some(indice) = separador {
        let nome = &referencia[..indice];
        let nome = if nome.starts_with('\'') && nome.ends_with('\'') && nome.len() >= 2 {
            nome[1..nome.len() - 1].replace("''", "'")
        } else {
            nome.to_owned()
        };
        (workbook.indice_por_nome(&nome)?, &referencia[indice + 1..])
    } else {
        (origem, referencia)
    };
    let (linha, coluna) = feathersheet_core::celulas::refs::parse_cell_ref(celula)?;
    Some((aba, linha, coluna))
}

#[cfg(test)]
mod testes_destino {
    use super::destino_passo;
    use feathersheet_core::planilha::workbook::Workbook;

    #[test]
    fn referencia_simples_e_referencia_invalida() {
        let wb = Workbook::new();
        assert_eq!(destino_passo(&wb, 0, "C12"), Some((0, 11, 2)));
        assert_eq!(destino_passo(&wb, 0, "C0"), None);
        assert_eq!(destino_passo(&wb, 0, "A1:B2"), None);
        assert_eq!(destino_passo(&wb, 0, "'Aba incompleta!B2"), None);
    }
}
