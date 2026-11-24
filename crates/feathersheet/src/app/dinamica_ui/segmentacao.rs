//! Segmentação de dados (básica, 16/10/2026, meta da v0.1.0): Inserir ▸
//! Segmentação de dados, com a seleção numa tabela dinâmica, escolhe um
//! campo e cria um cartão de botões, um por item. Clicar num item filtra a
//! dinâmica (e o gráfico dinâmico dela) só com os itens escolhidos; nenhum
//! escolhido = sem filtro. A segmentação fica guardada na própria dinâmica
//! (`segmentacoes` no JSON, dentro do arquivo) e os cartões aparecem à
//! direita da grade na aba da dinâmica, com o painel de campos fechado.
//! Conexões de relatório (#145, 24/11/2026): a segmentação também filtra outras dinâmicas da mesma origem
//! (`liga` no JSON, pelo id da dinâmica); o botão de corrente do cartão escolhe quais.
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
    /// Quantas outras dinâmicas ela filtra (Conexões de relatório).
    ligadas: usize,
}

#[derive(Default)]
pub(in crate::app) struct Segmentacoes {
    /// Escolhendo o campo da segmentação nova: (aba, id da dinâmica, campos).
    escolher: Option<(usize, String, Vec<(usize, String)>)>,
    /// Escolhendo o campo de data da linha do tempo nova: (aba, id da dinâmica, campos de data).
    pub(super) escolher_tempo: Option<(usize, String, Vec<(usize, String)>)>,
    /// Conexões de relatório abertas: (aba da dinâmica, id, coluna da segmentação).
    conexoes: Option<(usize, String, usize)>,
    /// Os cartões da aba, pela (aba, versões de todas as abas).
    cache: RefCell<Option<((usize, Vec<u32>), Vec<Cartao>, Vec<linha_tempo::CartaoTempo>)>>,
}

impl Segmentacoes {
    /// Os cartões guardados não valem mais (ex.: a lista de itens de origem grande terminou de ser lida).
    pub(super) fn esquecer(&self) {
        self.cache.borrow_mut().take();
    }
}

/// A dinâmica `id` em qualquer aba: (aba, definição).
// shortcut: o id só é único por aba (`dinamica_nativa_<aba>_<n>`, `din_<aba>_<n>` do .xlsx); com abas reordenadas
// e dinâmicas novas pode repetir, e vale a primeira com a mesma origem. Se incomodar, gerar id único na pasta.
fn localizar_por_id(app: &App, id: &str, fonte: &Json) -> Option<(usize, Json)> {
    (0..app.workbook.len()).find_map(|i| {
        let texto = app.workbook.aba(i)?.sheet.dn.as_deref()?;
        serde_json::from_str::<Vec<Json>>(texto).ok()?.into_iter()
            .find(|d| d["id"].as_str() == Some(id) && d["fonte"] == *fonte).map(|d| (i, d))
    })
}

/// As outras dinâmicas da pasta com a mesma origem: (id, "aba · canto").
fn mesma_origem(app: &App, destino: usize, id: &str) -> Vec<(String, String)> {
    let Some(fonte) = meta_por_id(app, destino, id).map(|m| m["fonte"].clone()) else { return Vec::new() };
    let mut saida = Vec::new();
    for i in 0..app.workbook.len() {
        let Some(aba) = app.workbook.aba(i) else { continue };
        let Some(lista) = aba.sheet.dn.as_deref().and_then(|t| serde_json::from_str::<Vec<Json>>(t).ok()) else { continue };
        for d in lista {
            let outro = d["id"].as_str().unwrap_or_default();
            if d["fonte"] != fonte || (i == destino && outro == id) { continue; }
            let canto = d["areaResultado"]["r0"].as_u64().zip(d["areaResultado"]["c0"].as_u64())
                .map(|(r, c)| feathersheet_core::celulas::refs::cell_name(r as u32, c as u32)).unwrap_or_default();
            saida.push((outro.to_owned(), format!("{} · {canto}", aba.sheet.name)));
        }
    }
    saida
}

/// A definição da dinâmica `id` gravada na aba `destino`.
pub(super) fn meta_por_id(app: &App, destino: usize, id: &str) -> Option<Json> {
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
                    if !e.segmentacoes.iter().any(|s| s.coluna == coluna) {
                        e.segmentacoes.push(Segmentacao { coluna, ..Default::default() });
                    }
                });
            }
            Evento::SegmentacaoItem(destino, id, coluna, chave) => {
                self.mudar_segmentacao(destino, &id, |e| {
                    if let Some(s) = e.segmentacoes.iter_mut().find(|s| s.coluna == coluna) {
                        match s.sel.iter().position(|x| *x == chave) {
                            Some(i) => { s.sel.remove(i); }
                            None => s.sel.push(chave),
                        }
                    }
                });
                self.propagar_segmentacao(destino, &id, coluna, None);
            }
            Evento::SegmentacaoLimpar(destino, id, coluna) => {
                self.mudar_segmentacao(destino, &id, |e| {
                    if let Some(s) = e.segmentacoes.iter_mut().find(|s| s.coluna == coluna) {
                        s.sel.clear();
                    }
                });
                self.propagar_segmentacao(destino, &id, coluna, None);
            }
            Evento::SegmentacaoRemover(destino, id, coluna) => {
                // Como no Excel: a segmentação apagada tira o filtro dela das dinâmicas ligadas também.
                let fonte = meta_por_id(self, destino, &id).map(|m| m["fonte"].clone()).unwrap_or_default();
                let ligadas = self.segmentacao_de(destino, &id, coluna).map(|s| s.liga).unwrap_or_default();
                self.mudar_segmentacao(destino, &id, |e| {
                    e.segmentacoes.retain(|s| s.coluna != coluna);
                    e.filtros.retain(|f| f.coluna != coluna);
                });
                for alvo in ligadas { self.filtrar_ligada(&alvo, &fonte, coluna, &[]); }
            }
            Evento::SegmentacaoConexoes(destino, id, coluna) => self.segmentacao.conexoes = Some((destino, id, coluna)),
            Evento::SegmentacaoFecharConexoes => self.segmentacao.conexoes = None,
            Evento::SegmentacaoLigar(alvo) => {
                let Some((destino, id, coluna)) = self.segmentacao.conexoes.clone() else { return };
                let ligar = !self.segmentacao_de(destino, &id, coluna).is_some_and(|s| s.liga.contains(&alvo));
                self.mudar_segmentacao(destino, &id, |e| {
                    if let Some(s) = e.segmentacoes.iter_mut().find(|s| s.coluna == coluna) {
                        if ligar { s.liga.push(alvo.clone()); } else { s.liga.retain(|x| *x != alvo); }
                    }
                });
                // Ligada recebe a escolha de agora; desligada volta a mostrar tudo nesse campo.
                self.propagar_segmentacao(destino, &id, coluna, Some((&alvo, ligar)));
            }
            _ => {}
        }
    }

    /// A segmentação da coluna `coluna` da dinâmica `id`.
    fn segmentacao_de(&self, destino: usize, id: &str, coluna: usize) -> Option<Segmentacao> {
        let meta = meta_por_id(self, destino, id)?;
        edicao::ler_estado(self, destino, &meta).ok()?.segmentacoes.into_iter().find(|s| s.coluna == coluna)
    }

    /// Leva a escolha da segmentação às dinâmicas ligadas. `so`: só esta (ligada agora, `true`, ou desligada,
    /// `false`, que perde o filtro do campo).
    fn propagar_segmentacao(&mut self, destino: usize, id: &str, coluna: usize, so: Option<(&str, bool)>) {
        let Some(fonte) = meta_por_id(self, destino, id).map(|m| m["fonte"].clone()) else { return };
        let Some(s) = self.segmentacao_de(destino, id, coluna) else {
            if let Some((alvo, false)) = so { self.filtrar_ligada(alvo, &fonte, coluna, &[]); }
            return;
        };
        match so {
            Some((alvo, true)) => self.filtrar_ligada(alvo, &fonte, coluna, &s.sel),
            Some((alvo, false)) => self.filtrar_ligada(alvo, &fonte, coluna, &[]),
            None => for alvo in &s.liga { self.filtrar_ligada(alvo, &fonte, coluna, &s.sel); },
        }
    }

    /// Filtra a dinâmica ligada `alvo` (mesma `fonte`) no campo `coluna` pelos itens `sel` (vazio = sem filtro).
    /// Se ela tem a própria segmentação nesse campo, o cartão dela acompanha.
    fn filtrar_ligada(&mut self, alvo: &str, fonte: &Json, coluna: usize, sel: &[String]) {
        let Some((destino, meta)) = localizar_por_id(self, alvo, fonte) else { return };
        let Ok(mut e) = edicao::ler_estado(self, destino, &meta) else { return };
        e.filtros.retain(|f| f.coluna != coluna);
        if !sel.is_empty() {
            e.filtros.push(Filtro { coluna, valores: sel.iter().cloned().collect(), excluir: false });
        }
        if let Some(s) = e.segmentacoes.iter_mut().find(|s| s.coluna == coluna) { s.sel = sel.to_vec(); }
        self.aplicar_sem_mover(destino, &e);
    }

    /// Recalcula a dinâmica sem tirar a pessoa do lugar.
    pub(super) fn aplicar_sem_mover(&mut self, destino: usize, e: &Estado) {
        let lugar = (self.folha, self.linha, self.coluna, self.linha_inicio, self.coluna_inicio,
            self.fim_selecao, self.endereco.clone());
        let feito = self.aplicar_estado_na_dinamica(destino, e);
        (self.folha, self.linha, self.coluna, self.linha_inicio, self.coluna_inicio,
            self.fim_selecao, self.endereco) = lugar;
        if let Err(erro) = feito {
            self.status = erro;
        }
        self.segmentacao.cache.borrow_mut().take();
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
        for s in e.segmentacoes.clone() {
            e.filtros.retain(|f| f.coluna != s.coluna);
            if !s.sel.is_empty() {
                e.filtros.push(Filtro { coluna: s.coluna, valores: s.sel.into_iter().collect(), excluir: false });
            }
        }
        self.aplicar_sem_mover(destino, &e);
    }

    /// Os cartões da aba ativa: segmentações e linhas do tempo (guardados até alguma aba mudar).
    #[cfg(test)]
    fn cartoes_da_aba(&self) -> Vec<Cartao> {
        self.cartoes_e_tempos().0
    }

    fn cartoes_e_tempos(&self) -> (Vec<Cartao>, Vec<linha_tempo::CartaoTempo>) {
        let chave = (self.folha, (0..self.workbook.len()).filter_map(|i| self.workbook.aba(i)).map(|a| a.versao).collect::<Vec<_>>());
        if let Some((k, cartoes, tempos)) = &*self.segmentacao.cache.borrow() {
            if *k == chave {
                return (cartoes.clone(), tempos.clone());
            }
        }
        let mut cartoes = Vec::new();
        let lista: Vec<Json> = self.workbook.aba(self.folha)
            .and_then(|a| a.sheet.dn.as_deref())
            .filter(|t| t.contains("\"segmentacoes\"") || t.contains("\"linhasTempo\""))
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or_default();
        for meta in &lista {
            if meta["segmentacoes"].as_array().is_none_or(|a| a.is_empty()) { continue; }
            let Ok(e) = edicao::ler_estado(self, self.folha, meta) else { continue };
            let id = meta["id"].as_str().unwrap_or_default().to_owned();
            for s in &e.segmentacoes {
                let nome = e.nomes.get(s.coluna.saturating_sub(e.origem.c0 as usize)).cloned().unwrap_or_default();
                let (itens, _) = avancado::opcoes_de_filtro(self, &e, s.coluna);
                cartoes.push(Cartao { destino: self.folha, id: id.clone(), coluna: s.coluna, nome, selecionados: s.sel.clone(),
                    itens, ligadas: s.liga.len() });
            }
        }
        let tempos = self.cartoes_tempo(&lista);
        *self.segmentacao.cache.borrow_mut() = Some((chave, cartoes.clone(), tempos.clone()));
        (cartoes, tempos)
    }

    /// Os cartões das segmentações no canto de cima à direita da própria grade, abaixo
    /// das letras das colunas. Antes eram postos por conta (barra de título + faixa) e
    /// caíam por cima da barra de fórmulas e do aviso, e embaixo do painel do Gráfico.
    pub(in crate::app) fn grade_com_segmentacoes<'a>(&'a self, grade: Element<'a, Msg>) -> Element<'a, Msg> {
        let (cartoes, tempos) = self.cartoes_e_tempos();
        if cartoes.is_empty() && tempos.is_empty() {
            return grade;
        }
        let mut pilha = column(Vec::<pelicano::Element<_>>::new()).spacing(8).align(pelicano::Align::End);
        for t in tempos {
            pilha = pilha.push(linha_tempo::cartao(t));
        }
        for c in cartoes {
            pilha = pilha.push(cartao(c));
        }
        // 32 px: a linha das letras; 24 px: a barra de rolagem.
        let canto = container(pilha).width(Length::FILL).align_x(pelicano::Align::End)
            .padding(pelicano::Padding { top: 32.0, right: 24.0, bottom: 0.0, left: 0.0 });
        pelicano::stack![grade, canto].into()
    }

    /// A escolha do campo de uma segmentação nova (ou as Conexões de relatório), no meio da janela.
    pub(super) fn sobrepor_segmentacoes<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if let Some((destino, id, coluna)) = &self.segmentacao.conexoes {
            let ligadas = self.segmentacao_de(*destino, id, *coluna).map(|s| s.liga).unwrap_or_default();
            let candidatas = mesma_origem(self, *destino, id);
            let mut lista = column(Vec::<pelicano::Element<_>>::new()).spacing(2);
            if candidatas.is_empty() {
                lista = lista.push(text(tr!("Nenhuma outra tabela dinâmica usa a mesma origem.")).size(12).color(COR_TEXTO_MUTED()));
            }
            for (outro, rotulo) in candidatas {
                let marcada = ligadas.contains(&outro);
                lista = lista.push(button(row![
                    icone(if marcada { "checkbox" } else { "square" }, 14.0),
                    text(rotulo).size(12),
                ].spacing(8).align(pelicano::Align::Center)).width(Length::FILL).padding([5, 8])
                    .style(estilo_botao_pequeno(marcada))
                    .on_press(Msg::Dinamica(Evento::SegmentacaoLigar(outro))));
            }
            let janela = container(column![
                row![
                    text(tr!("Conexões de relatório")).size(13).width(Length::FILL),
                    button(icone("x", 14.0)).padding([3, 5]).style(estilo_botao_pequeno(false))
                        .on_press(Msg::Dinamica(Evento::SegmentacaoFecharConexoes)),
                ].spacing(6).align(pelicano::Align::Center),
                text(tr!("A segmentação também filtra as tabelas dinâmicas marcadas (só as da mesma origem).")).size(11.5)
                    .color(COR_TEXTO_MUTED()),
                scrollable(lista).height(Length::Shrink),
            ].spacing(8)).padding(12).width(Length::Fixed(320.0)).max_height(360.0).style(estilo_popup_menu);
            return pelicano::stack![base, container(janela).center()].height(Length::FILL).into();
        }
        if self.segmentacao.escolher_tempo.is_some() { return self.sobrepor_escolha_tempo(base); }
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
        button(icone("link", 13.0)).padding([3, 5]).style(estilo_botao_pequeno(c.ligadas > 0))
            .on_press(Msg::Dinamica(Evento::SegmentacaoConexoes(c.destino, c.id.clone(), c.coluna))),
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

    /// #145: Conexões de relatório. A segmentação da dinâmica A, ligada à B (mesma origem), filtra as duas;
    /// desligar ou apagar a segmentação devolve a B inteira. Ida e volta do `liga` pelo JSON da dinâmica.
    #[test]
    fn segmentacao_ligada_filtra_outra_dinamica_da_mesma_origem() {
        let mut app = App::default();
        app.workbook.editar(0, |s| {
            s.set_cell(0, 0, "Região"); s.set_cell(0, 1, "Valor");
            s.set_cell(1, 0, "Norte"); s.set_cell(1, 1, "10");
            s.set_cell(2, 0, "Sul"); s.set_cell(2, 1, "30");
            s.set_cell(3, 0, "Norte"); s.set_cell(3, 1, "20");
        });
        let criar = |app: &mut App| {
            app.folha = 0;
            (app.linha, app.coluna, app.fim_selecao) = (0, 0, (3, 1));
            app.evento_dinamica(Evento::Abrir);
            crate::app::dinamica_ui::campos_de_antes(app);
            app.evento_dinamica(Evento::Criar);
            app.dinamica = None;
            app.folha
        };
        let a = criar(&mut app);
        let b = criar(&mut app);
        assert_ne!(a, b);
        let tem = |app: &App, aba: usize, t: &str| (0..8).any(|r| app.workbook.aba(aba).unwrap().sheet.get_raw(r, 0) == t);
        let id = |app: &App, aba: usize| serde_json::from_str::<Vec<Json>>(app.workbook.aba(aba).unwrap().sheet.dn.as_deref().unwrap())
            .unwrap()[0]["id"].as_str().unwrap().to_owned();
        let (id_a, id_b) = (id(&app, a), id(&app, b));

        app.folha = a;
        (app.linha, app.coluna) = (0, 0);
        app.evento_dinamica(Evento::SegmentacaoInserir);
        app.evento_dinamica(Evento::SegmentacaoCampo(0));
        app.evento_dinamica(Evento::SegmentacaoConexoes(a, id_a.clone(), 0));
        assert_eq!(mesma_origem(&app, a, &id_a).iter().map(|(i, _)| i.clone()).collect::<Vec<_>>(), vec![id_b.clone()]);
        let _: Element<'_, Msg> = app.sobrepor_segmentacoes(Space::new(0.0, 0.0).into());
        app.evento_dinamica(Evento::SegmentacaoLigar(id_b.clone()));
        assert_eq!(app.segmentacao_de(a, &id_a, 0).unwrap().liga, vec![id_b.clone()], "o liga vai e volta pelo JSON");
        assert_eq!(app.cartoes_da_aba()[0].ligadas, 1);

        app.evento_dinamica(Evento::SegmentacaoItem(a, id_a.clone(), 0, "Sul".into()));
        assert!(tem(&app, a, "Sul") && !tem(&app, a, "Norte"));
        assert!(tem(&app, b, "Sul") && !tem(&app, b, "Norte"), "a ligada filtra junto");

        app.evento_dinamica(Evento::SegmentacaoLigar(id_b.clone()));
        assert!(tem(&app, b, "Norte"), "desligada volta a mostrar tudo");
        assert!(!tem(&app, a, "Norte"), "a dona continua filtrada");

        app.evento_dinamica(Evento::SegmentacaoLigar(id_b.clone()));
        assert!(!tem(&app, b, "Norte"), "ligar de novo leva a escolha de agora");
        app.evento_dinamica(Evento::SegmentacaoRemover(a, id_a.clone(), 0));
        assert!(tem(&app, a, "Norte") && tem(&app, b, "Norte"), "apagar a segmentação solta as duas");
        for icone in ["link", "checkbox", "square"] {
            assert!(feather_ui::icones::existe(icone), "ícone {icone}");
        }
    }
}
