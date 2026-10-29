//! Segmentação de dados (básica, 16/10/2026, meta da v0.1.0): Inserir ▸
//! Segmentação de dados, com a seleção numa tabela dinâmica, escolhe um
//! campo e cria um cartão de botões, um por item. Clicar num item filtra a
//! dinâmica (e o gráfico dinâmico dela) só com os itens escolhidos; nenhum
//! escolhido = sem filtro. A segmentação fica guardada na própria dinâmica
//! (`segmentacoes` no JSON, dentro do arquivo) e os cartões aparecem à
//! direita da grade na aba da dinâmica, com o painel de campos fechado.
//! Ligada a uma dinâmica só; várias dinâmicas e estilos ficam para a v0.2.
use super::*;
use std::cell::RefCell;
use serde_json::Value as Json;
use feather_ui::icones::icone;
use pelicano::column;

const LARGURA_CARTAO: f32 = 210.0;

/// Um cartão na tela: a dinâmica, o campo, os itens e os escolhidos.
#[derive(Debug, Clone)]
struct Cartao {
    destino: usize,
    id: String,
    coluna: usize,
    nome: String,
    selecionados: Vec<String>,
    /// (chave que o filtro compara, rótulo como na célula)
    itens: Vec<(String, String)>,
}

#[derive(Default)]
pub(in crate::app) struct Segmentacoes {
    /// Escolhendo o campo da segmentação nova: (aba, id da dinâmica, campos).
    escolher: Option<(usize, String, Vec<(usize, String)>)>,
    /// Os cartões da aba, pela (aba, versões de todas as abas).
    cache: RefCell<Option<((usize, Vec<u32>), Vec<Cartao>)>>,
}

/// A definição da dinâmica `id` gravada na aba `destino`.
fn meta_por_id(app: &App, destino: usize, id: &str) -> Option<Json> {
    let texto = app.workbook.aba(destino)?.sheet.dn.as_deref()?;
    serde_json::from_str::<Vec<Json>>(texto).ok()?.into_iter().find(|d| d["id"].as_str() == Some(id))
}

impl App {
    pub(super) fn evento_segmentacao(&mut self, evento: Evento) {
        match evento {
            Evento::SegmentacaoInserir => {
                let Some((destino, meta)) = edicao::localizar_na_celula(self, self.folha, self.linha, self.coluna) else {
                    self.status = tr!("Selecione uma célula de uma tabela dinâmica para inserir a segmentação.").into();
                    return;
                };
                let e = match edicao::ler_estado(self, destino, &meta) {
                    Ok(e) => e,
                    Err(erro) => { self.status = erro; return; }
                };
                let reais = e.nomes.len().saturating_sub(e.calculados.len());
                let campos = e.nomes[..reais].iter().enumerate()
                    .map(|(i, n)| (e.origem.c0 as usize + i, n.clone())).collect();
                let id = meta["id"].as_str().unwrap_or_default().to_owned();
                self.segmentacao.escolher = Some((destino, id, campos));
            }
            Evento::SegmentacaoFecharEscolha => self.segmentacao.escolher = None,
            Evento::SegmentacaoCampo(coluna) => {
                let Some((destino, id, _)) = self.segmentacao.escolher.take() else { return };
                self.mudar_segmentacao(destino, &id, |e| {
                    if !e.segmentacoes.iter().any(|(c, _)| *c == coluna) {
                        e.segmentacoes.push((coluna, Vec::new()));
                    }
                });
            }
            Evento::SegmentacaoItem(destino, id, coluna, chave) => {
                self.mudar_segmentacao(destino, &id, |e| {
                    if let Some((_, sel)) = e.segmentacoes.iter_mut().find(|(c, _)| *c == coluna) {
                        match sel.iter().position(|s| *s == chave) {
                            Some(i) => { sel.remove(i); }
                            None => sel.push(chave),
                        }
                    }
                });
            }
            Evento::SegmentacaoLimpar(destino, id, coluna) => {
                self.mudar_segmentacao(destino, &id, |e| {
                    if let Some((_, sel)) = e.segmentacoes.iter_mut().find(|(c, _)| *c == coluna) {
                        sel.clear();
                    }
                });
            }
            Evento::SegmentacaoRemover(destino, id, coluna) => {
                self.mudar_segmentacao(destino, &id, |e| {
                    e.segmentacoes.retain(|(c, _)| *c != coluna);
                    e.filtros.retain(|f| f.coluna != coluna);
                });
            }
            _ => {}
        }
    }

    /// Lê a dinâmica, muda as segmentações, refaz os filtros delas e
    /// recalcula (o gráfico dinâmico segue). Não tira a pessoa do lugar.
    fn mudar_segmentacao(&mut self, destino: usize, id: &str, mudar: impl FnOnce(&mut Estado)) {
        let Some(meta) = meta_por_id(self, destino, id) else { return };
        let mut e = match edicao::ler_estado(self, destino, &meta) {
            Ok(e) => e,
            Err(erro) => { self.status = erro; return; }
        };
        mudar(&mut e);
        for (coluna, sel) in e.segmentacoes.clone() {
            e.filtros.retain(|f| f.coluna != coluna);
            if !sel.is_empty() {
                e.filtros.push(Filtro { coluna, valores: sel.into_iter().collect(), excluir: false });
            }
        }
        let lugar = (self.folha, self.linha, self.coluna, self.linha_inicio, self.coluna_inicio,
            self.fim_selecao, self.endereco.clone());
        let feito = self.aplicar_estado_na_dinamica(destino, &e);
        (self.folha, self.linha, self.coluna, self.linha_inicio, self.coluna_inicio,
            self.fim_selecao, self.endereco) = lugar;
        if let Err(erro) = feito {
            self.status = erro;
        }
        self.segmentacao.cache.borrow_mut().take();
    }

    /// Os cartões da aba ativa (guardados até alguma aba mudar).
    fn cartoes_da_aba(&self) -> Vec<Cartao> {
        let chave = (self.folha, (0..self.workbook.len()).filter_map(|i| self.workbook.aba(i)).map(|a| a.versao).collect::<Vec<_>>());
        if let Some((k, cartoes)) = &*self.segmentacao.cache.borrow() {
            if *k == chave {
                return cartoes.clone();
            }
        }
        let mut cartoes = Vec::new();
        let lista: Vec<Json> = self.workbook.aba(self.folha)
            .and_then(|a| a.sheet.dn.as_deref())
            .filter(|t| t.contains("\"segmentacoes\""))
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or_default();
        for meta in lista {
            if meta["segmentacoes"].as_array().is_none_or(|a| a.is_empty()) { continue; }
            let Ok(e) = edicao::ler_estado(self, self.folha, &meta) else { continue };
            let id = meta["id"].as_str().unwrap_or_default().to_owned();
            for (coluna, sel) in &e.segmentacoes {
                let nome = e.nomes.get(coluna.saturating_sub(e.origem.c0 as usize)).cloned().unwrap_or_default();
                let (itens, _) = avancado::opcoes_de_filtro(self, &e, *coluna);
                cartoes.push(Cartao { destino: self.folha, id: id.clone(), coluna: *coluna, nome, selecionados: sel.clone(), itens });
            }
        }
        *self.segmentacao.cache.borrow_mut() = Some((chave, cartoes.clone()));
        cartoes
    }

    /// Os cartões das segmentações no canto de cima à direita da própria grade, abaixo
    /// das letras das colunas. Antes eram postos por conta (barra de título + faixa) e
    /// caíam por cima da barra de fórmulas e do aviso, e embaixo do painel do Gráfico.
    pub(in crate::app) fn grade_com_segmentacoes<'a>(&'a self, grade: Element<'a, Msg>) -> Element<'a, Msg> {
        let cartoes = self.cartoes_da_aba();
        if cartoes.is_empty() {
            return grade;
        }
        let mut pilha = column(Vec::<pelicano::Element<_>>::new()).spacing(8);
        for c in cartoes {
            pilha = pilha.push(cartao(c));
        }
        // 32 px: a linha das letras; 24 px: a barra de rolagem.
        let canto = container(pilha).width(Length::FILL).align_x(pelicano::Align::End)
            .padding(pelicano::Padding { top: 32.0, right: 24.0, bottom: 0.0, left: 0.0 });
        pelicano::stack![grade, canto].into()
    }

    /// A escolha do campo de uma segmentação nova, no meio da janela.
    pub(super) fn sobrepor_segmentacoes<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        let Some((_, _, campos)) = &self.segmentacao.escolher else { return base };
        let mut lista = column(Vec::<pelicano::Element<_>>::new()).spacing(2);
        for (coluna, nome) in campos {
            lista = lista.push(button(text(nome.clone()).size(12)).width(Length::FILL).padding([5, 8])
                .style(estilo_botao_pequeno(false))
                .on_press(Msg::Dinamica(Evento::SegmentacaoCampo(*coluna))));
        }
        let janela = container(column![
            row![
                text(tr!("Segmentação de dados: escolha o campo")).size(13).width(Length::FILL),
                button(icone("x", 14.0)).padding([3, 5]).style(estilo_botao_pequeno(false))
                    .on_press(Msg::Dinamica(Evento::SegmentacaoFecharEscolha)),
            ].spacing(6).align(pelicano::Align::Center),
            scrollable(lista).height(Length::Fixed(240.0)),
        ].spacing(8)).padding(12).width(Length::Fixed(280.0)).style(estilo_popup_menu);
        pelicano::stack![base, container(janela).center()].height(Length::FILL).into()
    }
}

fn cartao<'a>(c: Cartao) -> Element<'a, Msg> {
    let mut lista = column(Vec::<pelicano::Element<_>>::new()).spacing(3);
    for (chave, rotulo) in &c.itens {
        let escolhido = c.selecionados.contains(chave);
        lista = lista.push(button(text(rotulo.clone()).size(12)).width(Length::FILL).padding([5, 8])
            .style(estilo_botao_pequeno(escolhido))
            .on_press(Msg::Dinamica(Evento::SegmentacaoItem(c.destino, c.id.clone(), c.coluna, chave.clone()))));
    }
    let cabecalho = row![
        icone("slicer", 14.0),
        text(c.nome.clone()).size(12.5).width(Length::FILL),
        button(icone("eraser", 13.0)).padding([3, 5]).style(estilo_botao_pequeno(false))
            .on_press_maybe((!c.selecionados.is_empty())
                .then(|| Msg::Dinamica(Evento::SegmentacaoLimpar(c.destino, c.id.clone(), c.coluna)))),
        button(icone("x", 13.0)).padding([3, 5]).style(estilo_botao_pequeno(false))
            .on_press(Msg::Dinamica(Evento::SegmentacaoRemover(c.destino, c.id.clone(), c.coluna))),
    ].spacing(5).align(pelicano::Align::Center);
    container(column![cabecalho, scrollable(lista).height(Length::Shrink)].spacing(6))
        .padding(8).width(Length::Fixed(LARGURA_CARTAO)).max_height(260.0)
        .style(|_| pelicano::Style {
            background: Some(pelicano::Background::Color(COR_PAINEL())),
            border: pelicano::Border { color: COR_BORDA_SUAVE(), width: 1.0, radius: [8.0; 4] },
            ..Default::default()
        })
        .into()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn segmentacao_filtra_a_dinamica_pelos_itens_escolhidos() {
        let mut app = App::default();
        app.workbook.editar(0, |s| {
            s.set_cell(0, 0, "Região"); s.set_cell(0, 1, "Valor");
            s.set_cell(1, 0, "Norte"); s.set_cell(1, 1, "10");
            s.set_cell(2, 0, "Sul"); s.set_cell(2, 1, "30");
            s.set_cell(3, 0, "Norte"); s.set_cell(3, 1, "20");
        });
        app.fim_selecao = (3, 1);
        app.evento_dinamica(Evento::Abrir);
        crate::app::dinamica_ui::campos_de_antes(&mut app);
        app.evento_dinamica(Evento::Criar);
        app.dinamica = None;
        let destino = app.folha;
        app.evento_dinamica(Evento::SegmentacaoInserir);
        assert!(app.segmentacao.escolher.is_some(), "abre a escolha do campo");
        app.evento_dinamica(Evento::SegmentacaoCampo(0));
        let cartoes = app.cartoes_da_aba();
        assert_eq!(cartoes.len(), 1);
        assert_eq!(cartoes[0].itens.len(), 2);
        let id = cartoes[0].id.clone();
        app.evento_dinamica(Evento::SegmentacaoItem(destino, id.clone(), 0, "Sul".into()));
        let s = &app.workbook.aba(destino).unwrap().sheet;
        let tem = |t: &str| (0..8).any(|r| s.get_raw(r, 0) == t);
        assert!(tem("Sul") && !tem("Norte"), "só Sul fica na dinâmica");
        app.evento_dinamica(Evento::SegmentacaoLimpar(destino, id, 0));
        let s = &app.workbook.aba(destino).unwrap().sheet;
        assert!((0..8).any(|r| s.get_raw(r, 0) == "Norte"), "sem escolha, sem filtro");
    }
}
