//! Minigráficos na interface: o diálogo "Criar Minigráficos" (Inserir ▸
//! Minigráficos ▸ Linha, Coluna ou Ganhos/Perdas), como no Excel, e a aba
//! contextual "Minigráfico" quando a célula ativa tem um. O modelo e as contas
//! ficam em `feathersheet_core::operacoes::minigraficos`; o desenho, em
//! `crate::minigraficos_visual`. Cada mudança é um Ctrl+Z (`alterar_minigraficos`).
use super::*;
use feathersheet_core::celulas::refs::{format_ref, parse_range_ref, quote_sheet};
use feathersheet_core::operacoes::minigraficos::{criar, na_celula, partir_dados, ErroCriar, Grupo, Tipo};
use feathersheet_io::minigraficos_aba;
use pelicano::column;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Campo { Dados, Local }

/// O que liga e desliga na aba contextual (grupos Mostrar e Agrupar).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcao { Alto, Baixo, Negativos, Primeiro, Ultimo, Marcadores, EixoIgual }

#[derive(Debug, Clone)]
pub enum Evento {
    Abrir(Tipo),
    Dados(String),
    Local(String),
    Confirmar,
    Fechar,
    /// Aba contextual: valem para o grupo da célula ativa.
    Tipo(Tipo),
    Alternar(Opcao),
    Estilo(usize),
    Limpar,
    LimparGrupo,
}

#[derive(Debug, Clone)]
pub(super) struct Estado {
    tipo: Tipo,
    aba: usize,
    dados: String,
    local: String,
    campo: Campo,
    erro: Option<String>,
}

/// Galeria de estilos: (cor da série, cor dos destaques). O primeiro é o padrão do Excel.
const ESTILOS: [(&str, &str); 6] = [
    ("#376092", "#D00000"), ("#0B57D0", "#E37400"), ("#1B873F", "#C5221F"),
    ("#5F6368", "#0B57D0"), ("#E37400", "#5F6368"), ("#7B1FA2", "#1B873F"),
];

impl App {
    fn minigraficos_da_aba(&self, aba: usize) -> Vec<Grupo> {
        minigraficos_aba::ler(self.workbook.aba(aba).and_then(|a| a.sheet.mg.as_deref()))
    }

    fn gravar_minigraficos(&mut self, aba: usize, grupos: &[Grupo]) {
        let novo = minigraficos_aba::gravar(grupos);
        self.workbook.editar(aba, |s| { s.alterar_minigraficos(novo); });
        self.alterado = true;
    }

    /// (grupo, item) do minigráfico na célula ativa: decide a aba contextual.
    pub(super) fn minigrafico_na_celula(&self) -> Option<(usize, usize)> {
        na_celula(&self.minigraficos_da_aba(self.folha), self.linha, self.coluna)
    }

    pub(super) fn evento_minigrafico(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Abrir(tipo) => {
                if !self.confirmar() { return Task::none(); }
                // Com uma faixa selecionada, ela já vira os dados (como no Excel).
                let area = self.area_selecionada();
                let faixa = area.r0 != area.r1 || area.c0 != area.c1;
                self.minigrafico = Some(Estado {
                    tipo, aba: self.folha,
                    dados: if faixa { format_ref(&area) } else { String::new() },
                    local: String::new(),
                    campo: if faixa { Campo::Local } else { Campo::Dados },
                    erro: None,
                });
            }
            Evento::Dados(t) => if let Some(e) = &mut self.minigrafico { e.dados = t; e.campo = Campo::Dados; e.erro = None; },
            Evento::Local(t) => if let Some(e) = &mut self.minigrafico { e.local = t; e.campo = Campo::Local; e.erro = None; },
            Evento::Confirmar => {
                let Some(e) = self.minigrafico.clone() else { return Task::none() };
                match self.criar_minigraficos(&e) {
                    Ok(()) => {
                        self.minigrafico = None;
                        if self.folha != e.aba { self.folha = e.aba; }
                        self.status = tr!("Minigráficos criados").into();
                    }
                    Err(m) => if let Some(e) = &mut self.minigrafico { e.erro = Some(m); },
                }
            }
            Evento::Fechar => self.minigrafico = None,
            Evento::Tipo(_) | Evento::Alternar(_) | Evento::Estilo(_) | Evento::Limpar | Evento::LimparGrupo => {
                let Some((g, i)) = self.minigrafico_na_celula() else { return Task::none() };
                let mut grupos = self.minigraficos_da_aba(self.folha);
                let grupo = &mut grupos[g];
                match evento {
                    Evento::Tipo(t) => grupo.tipo = t,
                    Evento::Alternar(o) => {
                        let campo = match o {
                            Opcao::Alto => &mut grupo.alto, Opcao::Baixo => &mut grupo.baixo,
                            Opcao::Negativos => &mut grupo.negativos, Opcao::Primeiro => &mut grupo.primeiro,
                            Opcao::Ultimo => &mut grupo.ultimo, Opcao::Marcadores => &mut grupo.marcadores,
                            Opcao::EixoIgual => &mut grupo.eixo_igual,
                        };
                        *campo = !*campo;
                    }
                    Evento::Estilo(n) => {
                        let (serie, destaque) = ESTILOS[n.min(ESTILOS.len() - 1)];
                        grupo.cor = serie.into();
                        for c in [&mut grupo.cor_negativo, &mut grupo.cor_marcadores, &mut grupo.cor_alto,
                            &mut grupo.cor_baixo, &mut grupo.cor_primeiro, &mut grupo.cor_ultimo] {
                            *c = destaque.into();
                        }
                    }
                    Evento::Limpar => {
                        grupo.itens.remove(i);
                        if grupo.itens.is_empty() { grupos.remove(g); }
                    }
                    _ => { grupos.remove(g); }
                }
                let aba = self.folha;
                self.gravar_minigraficos(aba, &grupos);
            }
        }
        Task::none()
    }

    fn criar_minigraficos(&mut self, e: &Estado) -> Result<(), String> {
        let nome_da_aba = self.workbook.nomes().get(e.aba).cloned().unwrap_or_default();
        // A letra da coluna pode vir minúscula; o nome da aba, não se mexe.
        let maiusculas = |t: &str| match t.rsplit_once('!') {
            Some((aba, faixa)) => format!("{aba}!{}", faixa.to_uppercase()),
            None => t.to_uppercase(),
        };
        let dados = maiusculas(&e.dados.trim().replace('$', ""));
        let (aba_dados, faixa_dados) = partir_dados(&dados)
            .ok_or_else(|| trf!("\"{}\" não é um intervalo de dados válido.", e.dados.trim()))?;
        let aba_dados = aba_dados.filter(|n| *n != nome_da_aba);
        if let Some(n) = &aba_dados {
            if self.workbook.indice_por_nome(n).is_none() { return Err(trf!("A aba \"{n}\" não existe.")); }
        }
        let local_txt = maiusculas(&e.local.trim().replace('$', ""));
        let local = parse_range_ref(local_txt.rsplit_once('!').map_or(local_txt.as_str(), |(_, f)| f))
            .ok_or_else(|| trf!("\"{}\" não é um intervalo de local válido.", e.local.trim()))?;
        let novo = criar(e.tipo, faixa_dados, aba_dados.as_deref(), local).map_err(|erro| match erro {
            ErroCriar::LocalNaoEUmaLinhaOuColuna => tr!("O local precisa ser uma linha ou uma coluna de células.").to_owned(),
            ErroCriar::TamanhosNaoBatem => tr!("Os dados não se dividem em uma série por célula do local: confira os tamanhos.").to_owned(),
        })?;
        // Uma célula tem um minigráfico só: o novo substitui o que estava lá.
        let mut grupos = self.minigraficos_da_aba(e.aba);
        for g in &mut grupos {
            g.itens.retain(|m| !novo.itens.iter().any(|n| n.linha == m.linha && n.coluna == m.coluna));
        }
        grupos.retain(|g| !g.itens.is_empty());
        grupos.push(novo);
        self.gravar_minigraficos(e.aba, &grupos);
        Ok(())
    }

    /// Seleção na grade com a janela aberta: a faixa vai para o campo ativo.
    pub(super) fn minigrafico_selecao(&mut self, area: Rect) {
        let folha = self.folha;
        let nome = self.workbook.nomes().get(folha).cloned().unwrap_or_default();
        let Some(e) = self.minigrafico.as_mut() else { return };
        match e.campo {
            Campo::Dados if folha == e.aba => e.dados = format_ref(&area),
            Campo::Dados => e.dados = format!("{}!{}", quote_sheet(&nome), format_ref(&area)),
            Campo::Local if folha == e.aba => e.local = format_ref(&area),
            Campo::Local => return,
        }
        e.erro = None;
    }

    /// Inserir/excluir linhas ou colunas: os minigráficos da aba andam junto
    /// (sem desfazer, como as mesclas e tabelas: o histórico da aba é esquecido).
    pub(super) fn refluir_minigraficos(&mut self, aba: usize, coluna: bool, at: u32, quantas: u32, removendo: bool) {
        let mut grupos = self.minigraficos_da_aba(aba);
        if grupos.is_empty() { return; }
        feathersheet_core::operacoes::minigraficos::refluir(&mut grupos, coluna, at, quantas, removendo);
        let novo = minigraficos_aba::gravar(&grupos);
        self.workbook.editar(aba, |s| s.mg = novo);
    }

    /// O diálogo sozinho (testes).
    #[cfg(test)]
    fn view_minigrafico(&self) -> Element<'_, Msg> {
        self.dialogo_minigrafico().painel()
    }

    fn dialogo_minigrafico(&self) -> super::dialogo_interno::Dialogo<'_, Msg> {
        let e = self.minigrafico.as_ref().expect("janela aberta");
        let ev = Msg::Minigrafico;
        let campo = |rotulo: &'static str, dica: &'static str, valor: &str, ativo: bool, f: fn(String) -> Evento| row![
            text(rotulo).size(12.5).width(Length::Fixed(150.0)),
            text_input(dica, valor.to_owned()).on_input(move |t| Msg::Minigrafico(f(t)))
                .on_submit(Msg::Minigrafico(Evento::Confirmar))
                .size(13).padding([5, 8]).width(Length::FILL)
                .style(move |tema, status| {
                    let mut s = pelicano::input_default(tema, status);
                    if ativo { s.border.color = COR_AZUL_ACENTO(); }
                    s
                }),
        ].spacing(8).align(pelicano::Align::Center);
        let mut corpo = column![
            text(tr!("Escolha os dados que você deseja")).size(12.5).color(COR_TEXTO_MUTED()),
            campo(tr!("Intervalo de dados:"), tr!("ex.: B2:M5"), &e.dados, e.campo == Campo::Dados, Evento::Dados),
            text(tr!("Escolha onde você deseja que os minigráficos sejam colocados")).size(12.5).color(COR_TEXTO_MUTED()),
            campo(tr!("Intervalo de local:"), tr!("ex.: N2:N5"), &e.local, e.campo == Campo::Local, Evento::Local),
            row![
                feather_ui::icones::icone("info-circle", 14.0),
                text(tr!("Selecione na planilha para preencher o campo destacado. O local é uma linha \
                      ou coluna de células; cada uma recebe uma linha (ou coluna) dos dados."))
                    .size(11.5).color(COR_TEXTO_MUTED()),
            ].spacing(6),
        ].spacing(10);
        if let Some(erro) = &e.erro {
            corpo = corpo.push(row![
                feather_ui::icones::icone("info-circle", 15.0), text(erro.clone()).size(12).color(COR_ERRO()),
            ].spacing(6).align(pelicano::Align::Center));
        }
        let (titulo_tipo, icone) = match e.tipo {
            Tipo::Linha => (tr!("Linha"), "sparkline-line"),
            Tipo::Coluna => (tr!("Coluna"), "sparkline-columns"),
            Tipo::GanhosPerdas => (tr!("Ganhos/Perdas"), "sparkline-winloss"),
        };
        // Não modal, à direita: a seleção na planilha preenche os campos.
        self.dialogo(tr!("Criar Minigráficos"), corpo)
            .icone(icone)
            .subtitulo(titulo_tipo)
            .topo(14.0)
            .altura_maxima((self.tamanho_principal.height - self.topo_dialogo() - 28.0).max(300.0))
            .largura(470.0)
            .nao_modal()
            .a_direita()
            .ao_fechar(ev(Evento::Fechar))
            .secundario(tr!("Cancelar"), Some(ev(Evento::Fechar)))
            .primario("OK", Some(ev(Evento::Confirmar)))
    }

    pub(super) fn sobrepor_minigrafico<'a>(&'a self, centro: Element<'a, Msg>) -> Element<'a, Msg> {
        if self.minigrafico.is_none() { return centro; }
        self.dialogo_minigrafico().sobre(centro)
    }

    /// A aba contextual "Minigráfico", como a do Excel.
    pub(super) fn grupos_faixa_minigrafico(&self) -> Vec<Element<'_, Msg>> {
        let grupos = self.minigraficos_da_aba(self.folha);
        let Some((g, _)) = na_celula(&grupos, self.linha, self.coluna) else {
            return vec![self.faixa_grupo(tr!("Minigráfico"),
                text(tr!("Selecione uma célula com minigráfico.")).size(12).into())];
        };
        let grupo = &grupos[g];
        let m = |e: Evento| Some(Msg::Minigrafico(e));
        let opcao = |rotulo: &str, icone: &str, o: Opcao, ligada: bool| self.faixa_pequeno(rotulo, icone, m(Evento::Alternar(o)), ligada);
        let amostra = |cor: &'static str| {
            let [r, g, b, _] = feather_ui::seletor_cor::rgba_de(cor);
            container(Space::new(10.0, 18.0)).style(move |_| pelicano::Style {
                background: Some(pelicano::Background::Color(pelicano::Color::rgb8(r, g, b))),
                ..Default::default()
            })
        };
        let mut estilos: Vec<Element<'_, Msg>> = ESTILOS.iter().enumerate().map(|(n, &(serie, destaque))| {
            let atual = grupo.cor.eq_ignore_ascii_case(serie) && grupo.cor_alto.eq_ignore_ascii_case(destaque);
            feather_ui::formatacao::dica(
                button(row![amostra(serie), amostra(destaque)].spacing(2))
                    .padding(4).on_press(Msg::Minigrafico(Evento::Estilo(n)))
                    .style(estilo_botao_pequeno(atual)),
                tr!("Estilo do minigráfico"),
            ).into()
        }).collect();
        vec![
            self.faixa_grupo(tr!("Tipo"), self.faixa_linha(vec![
                self.faixa_grande(tr!("Linha"), "sparkline-line", m(Evento::Tipo(Tipo::Linha)), grupo.tipo == Tipo::Linha),
                self.faixa_grande(tr!("Coluna"), "sparkline-columns", m(Evento::Tipo(Tipo::Coluna)), grupo.tipo == Tipo::Coluna),
                self.faixa_grande(tr!("Ganhos/Perdas"), "sparkline-winloss", m(Evento::Tipo(Tipo::GanhosPerdas)), grupo.tipo == Tipo::GanhosPerdas),
            ])),
            self.faixa_grupo(tr!("Mostrar"), self.faixa_linha(vec![
                self.faixa_pilha(vec![
                    opcao(tr!("Ponto Alto"), "arrow-up", Opcao::Alto, grupo.alto),
                    opcao(tr!("Ponto Baixo"), "arrow-down", Opcao::Baixo, grupo.baixo),
                    opcao(tr!("Pontos Negativos"), "minus", Opcao::Negativos, grupo.negativos),
                ]),
                self.faixa_pilha(vec![
                    opcao(tr!("Primeiro Ponto"), "chevron-left", Opcao::Primeiro, grupo.primeiro),
                    opcao(tr!("Último Ponto"), "chevron-right", Opcao::Ultimo, grupo.ultimo),
                    opcao(tr!("Marcadores"), "circle-dot", Opcao::Marcadores, grupo.marcadores),
                ]),
            ])),
            self.faixa_grupo(tr!("Estilo"), self.faixa_linha(vec![
                self.faixa_pilha(estilos.drain(..3).collect()),
                self.faixa_pilha(estilos),
            ])),
            self.faixa_grupo(tr!("Agrupar"), self.faixa_pilha(vec![
                opcao(tr!("Mesmo eixo para todos"), "arrows-vertical", Opcao::EixoIgual, grupo.eixo_igual),
                self.faixa_pequeno(tr!("Limpar minigráfico"), "eraser", m(Evento::Limpar), false),
                self.faixa_pequeno(tr!("Limpar grupo"), "trash", m(Evento::LimparGrupo), false),
            ])),
        ]
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cria_pela_selecao_muda_pela_aba_contextual_e_desfaz() {
        let mut app = App::default();
        for (c, v) in ["1", "3", "2", "5"].iter().enumerate() {
            app.workbook.escrever_celula(0, 0, c as u32, v.to_string());
            app.workbook.escrever_celula(0, 1, c as u32, format!("-{v}"));
        }
        // Seleciona os dados, abre Linha, marca o local arrastando na grade.
        let _ = app.update(Msg::IrPara("A1:D2".into()));
        let _ = app.update(Msg::Minigrafico(Evento::Abrir(Tipo::Linha)));
        assert_eq!(app.minigrafico.as_ref().unwrap().dados, "A1:D2");
        let _ = app.update(Msg::Selecionar(0, 4));
        let _ = app.update(Msg::Estender(1, 4));
        assert_eq!(app.minigrafico.as_ref().unwrap().local, "E1:E2");
        let _: Element<'_, Msg> = app.view_minigrafico();
        let _ = app.update(Msg::Minigrafico(Evento::Confirmar));
        assert!(app.minigrafico.is_none());
        let grupos = app.minigraficos_da_aba(0);
        assert_eq!(grupos[0].itens.iter().map(|m| m.dados.as_str()).collect::<Vec<_>>(), ["A1:D1", "A2:D2"]);

        // Na célula com minigráfico, a aba contextual muda o grupo.
        let _ = app.update(Msg::Selecionar(1, 4));
        assert_eq!(app.minigrafico_na_celula(), Some((0, 1)));
        assert!(!app.grupos_faixa_minigrafico().is_empty());
        let _ = app.update(Msg::Minigrafico(Evento::Tipo(Tipo::Coluna)));
        let _ = app.update(Msg::Minigrafico(Evento::Alternar(Opcao::Alto)));
        let _ = app.update(Msg::Minigrafico(Evento::Estilo(2)));
        let g = &app.minigraficos_da_aba(0)[0];
        assert_eq!((g.tipo, g.alto, g.cor.as_str()), (Tipo::Coluna, true, "#1B873F"));
        let _ = app.update(Msg::Desfazer);
        assert_eq!(app.minigraficos_da_aba(0)[0].cor, "#376092", "cada mudança é um Ctrl+Z");
        let _ = app.update(Msg::Minigrafico(Evento::Limpar));
        assert_eq!(app.minigraficos_da_aba(0)[0].itens.len(), 1);
        assert_eq!(app.minigrafico_na_celula(), None, "a célula limpa sai da aba contextual");
        let _ = app.update(Msg::Selecionar(0, 4));
        let _ = app.update(Msg::Minigrafico(Evento::LimparGrupo));
        assert!(app.minigraficos_da_aba(0).is_empty());
    }

    #[test]
    fn erros_ficam_na_janela_e_dados_de_outra_aba_levam_o_nome() {
        let mut app = App::default();
        let _ = app.update(Msg::Minigrafico(Evento::Abrir(Tipo::Coluna)));
        let _ = app.update(Msg::Minigrafico(Evento::Dados("A1:C3".into())));
        let _ = app.update(Msg::Minigrafico(Evento::Local("E1:F2".into())));
        let _ = app.update(Msg::Minigrafico(Evento::Confirmar));
        assert!(app.minigrafico.as_ref().unwrap().erro.is_some());
        let _ = app.update(Msg::Minigrafico(Evento::Dados("b1:b5".into())));
        let _ = app.update(Msg::Minigrafico(Evento::Local("e1".into())));
        let _ = app.update(Msg::Minigrafico(Evento::Confirmar));
        assert!(app.minigrafico.is_none());
        assert_eq!(app.minigraficos_da_aba(0)[0].itens[0].dados, "B1:B5");
        for escuro in [false, true] {
            feather_ui::tema::definir_tema(escuro, 0);
            let _ = app.update(Msg::Minigrafico(Evento::Abrir(Tipo::GanhosPerdas)));
            let _: Element<'_, Msg> = app.view_minigrafico();
        }
    }
}
