//! Estrutura de tópicos e Subtotal (Dados → Estrutura de tópicos), como no
//! Excel: Agrupar/Desagrupar (Alt+Shift+→ / Alt+Shift+←), Mostrar e Ocultar
//! detalhe, Limpar estrutura e a janela Subtotal. As regras moram no motor
//! (`planilha::sheet::topicos` e `planilha::subtotais`); aqui ficam a escolha
//! de linhas ou colunas pela seleção e a janela. Os [+]/[−] e os botões de
//! nível ficam na grade (`grade/topicos.rs`).
use super::*;
use feathersheet_core::celulas::refs::{col_name, Rect};
use feathersheet_core::operacoes::ops;
use feathersheet_core::planilha::subtotais::{OpcoesSubtotal, FUNCOES};

#[derive(Debug, Clone)]
pub enum Evento {
    Agrupar,
    Desagrupar,
    MostrarDetalhe,
    OcultarDetalhe,
    Limpar,
    /// Clique no [+]/[−] da grade: eixo (`true` = colunas), nível e resumo.
    Alternar(bool, u8, u32),
    /// Botões 1, 2, 3… da grade.
    Nivel(bool, u8),
    SubtotalAbrir,
    SubtotalGrupo(Rotulo),
    SubtotalFuncao(Rotulo),
    SubtotalColuna(u32, bool),
    SubtotalAplicar,
    SubtotalRemover,
    SubtotalFechar,
}

/// Item de lista suspensa: o valor e o texto mostrado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rotulo(pub u32, pub String);

impl std::fmt::Display for Rotulo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.1) }
}

#[derive(Debug, Clone)]
pub(super) struct EstadoSubtotal {
    pub area: Rect,
    /// Cabeçalho de cada coluna da lista ("Coluna B" quando vazio).
    pub colunas: Vec<Rotulo>,
    pub grupo: u32,
    pub funcao: u8,
    pub marcadas: Vec<u32>,
    pub erro: Option<String>,
}

impl App {
    /// Linhas, a não ser que a seleção seja de colunas inteiras.
    fn eixo_da_selecao(&self) -> (bool, u32, u32) {
        let a = self.area_selecionada();
        if self.selecao_e_colunas_inteiras() && !self.selecao_e_linhas_inteiras() { (true, a.c0, a.c1) }
        else { (false, a.r0, a.r1) }
    }

    fn mexer_topicos(&mut self, acao: impl FnOnce(&mut feathersheet_core::planilha::sheet::Sheet) -> Result<String, String>) {
        if !self.confirmar() { return; }
        let mut resultado = Err(tr!("A planilha não está disponível.").to_string());
        self.workbook.editar(self.folha, |s| resultado = acao(s));
        match resultado {
            Ok(msg) => { self.alterado = true; self.status = msg; }
            Err(e) => self.status = e,
        }
    }

    pub(super) fn evento_topicos(&mut self, evento: Evento) -> Task<Msg> {
        let (coluna, a, b) = self.eixo_da_selecao();
        let eixo = if coluna { tr!("Colunas") } else { tr!("Linhas") };
        let ativo = if coluna { self.coluna } else { self.linha };
        match evento {
            Evento::Agrupar => self.mexer_topicos(|s| s.agrupar(coluna, a, b)
                .map(|()| trf!("{eixo} agrupadas. Ctrl+Z desfaz."))),
            Evento::Desagrupar => self.mexer_topicos(|s| match s.desagrupar(coluna, a, b)? {
                0 => Err(trf!("Não há {} agrupadas na seleção.", eixo.to_lowercase())),
                _ => Ok(trf!("{eixo} desagrupadas.")),
            }),
            Evento::MostrarDetalhe => self.mexer_topicos(|s| s.mostrar_detalhe(coluna, ativo).map(|()| tr!("Detalhe mostrado.").into())),
            Evento::OcultarDetalhe => self.mexer_topicos(|s| s.ocultar_detalhe(coluna, ativo).map(|()| tr!("Detalhe ocultado.").into())),
            Evento::Limpar => self.mexer_topicos(|s| {
                let l = s.limpar_estrutura(false);
                let c = s.limpar_estrutura(true);
                if l || c { Ok(tr!("Estrutura de tópicos removida.").into()) } else { Err(tr!("A aba não tem estrutura de tópicos.").into()) }
            }),
            Evento::Alternar(coluna, nivel, resumo) => self.mexer_topicos(|s| s.alternar_grupo(coluna, nivel, resumo).map(|()| String::new())),
            Evento::Nivel(coluna, nivel) => self.mexer_topicos(|s| s.mostrar_nivel(coluna, nivel)
                .map(|()| trf!("Mostrando até o nível {nivel}."))),
            Evento::SubtotalAbrir => self.abrir_subtotal(),
            Evento::SubtotalGrupo(r) => if let Some(e) = &mut self.subtotal { e.grupo = r.0; },
            Evento::SubtotalFuncao(r) => if let Some(e) = &mut self.subtotal { e.funcao = r.0 as u8; },
            Evento::SubtotalColuna(c, marcada) => if let Some(e) = &mut self.subtotal {
                e.marcadas.retain(|&x| x != c);
                if marcada { e.marcadas.push(c); e.marcadas.sort_unstable(); }
            },
            Evento::SubtotalAplicar => self.aplicar_subtotal(false),
            Evento::SubtotalRemover => self.aplicar_subtotal(true),
            Evento::SubtotalFechar => self.subtotal = None,
        }
        Task::none()
    }

    fn abrir_subtotal(&mut self) {
        if !self.confirmar() { return; }
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        let area = ops::regiao_atual(&aba.sheet, self.area_selecionada());
        if area.r1 <= area.r0 {
            self.status = tr!("Selecione uma célula da lista (com cabeçalho) para o Subtotal.").into();
            return;
        }
        let colunas: Vec<Rotulo> = (area.c0..=area.c1).map(|c| {
            let t = aba.sheet.get_raw(area.r0, c);
            Rotulo(c, if t.trim().is_empty() { trf!("Coluna {}", col_name(c)) } else { t })
        }).collect();
        // Como o Excel: agrupa pela primeira coluna e soma a última numérica.
        let numerica = (area.c0..=area.c1).rev()
            .find(|&c| matches!(self.workbook.valor(self.folha, area.r0 + 1, c), feathersheet_core::celulas::values::Value::Number(_)))
            .unwrap_or(area.c1);
        self.edicao_menu = None;
        self.contexto = None;
        self.subtotal = Some(EstadoSubtotal { grupo: area.c0, funcao: 9, marcadas: vec![numerica], area, colunas, erro: None });
    }

    fn aplicar_subtotal(&mut self, remover: bool) {
        let Some(e) = self.subtotal.clone() else { return };
        let t = std::time::Instant::now();
        let resultado = if remover {
            self.workbook.remover_subtotais(self.folha, e.area.clone())
        } else {
            let op = OpcoesSubtotal { coluna_grupo: e.grupo, funcao: e.funcao, colunas: e.marcadas.clone() };
            self.workbook.subtotais(self.folha, e.area.clone(), &op)
        };
        match resultado {
            Ok(_) => {
                self.subtotal = None;
                self.alterado = true;
                self.trocar_selecao(self.linha, self.coluna);
                self.status = if remover { tr!("Subtotais removidos. Ctrl+Z desfaz.").into() } else { tr!("Subtotais inseridos. Ctrl+Z desfaz.").into() };
                feathersheet_core::diario::anotar(trf!("[subtotal] {} · {:.2} s",
                    if remover { tr!("remover") } else { tr!("inserir") }, t.elapsed().as_secs_f64()));
            }
            Err(erro) => if let Some(s) = &mut self.subtotal { s.erro = Some(erro); },
        }
    }

    fn view_subtotal(&self) -> super::dialogo_interno::Dialogo<'_, Msg> {
        let e = self.subtotal.as_ref().expect("janela Subtotal aberta");
        let grupo = e.colunas.iter().find(|r| r.0 == e.grupo).cloned();
        // O `pick_list` empresta as opções: a lista fixa das funções vive para sempre.
        static FUNCOES_ROTULOS: std::sync::LazyLock<Vec<Rotulo>> =
            std::sync::LazyLock::new(|| FUNCOES.iter().map(|&(n, t)| Rotulo(n as u32, t.into())).collect());
        let funcoes: &'static [Rotulo] = &FUNCOES_ROTULOS;
        let funcao = funcoes.iter().find(|r| r.0 == e.funcao as u32).cloned();
        let rotulo = |t: &'static str| text(t).size(12).color(COR_TEXTO_MUTED());
        let mut marcas = pelicano::column(Vec::<Element<_>>::new()).spacing(7);
        for r in &e.colunas {
            let c = r.0;
            marcas = marcas.push(pelicano::checkbox(r.1.clone(), e.marcadas.contains(&c))
                .on_toggle(move |v| Msg::Topicos(Evento::SubtotalColuna(c, v))).size(12.5));
        }
        let sub = trf!("Lista {}{}:{}{}. Ordene pela coluna de grupo antes; os subtotais de agora são substituídos.",
            col_name(e.area.c0), e.area.r0 + 1, col_name(e.area.c1), e.area.r1 + 1);
        let mut painel = pelicano::column![
            rotulo(tr!("A cada alteração em")),
            pelicano::pick_list(&e.colunas, grupo, |r| Msg::Topicos(Evento::SubtotalGrupo(r))).width(Length::FILL),
            rotulo(tr!("Usar função")),
            pelicano::pick_list(funcoes, funcao, |r| Msg::Topicos(Evento::SubtotalFuncao(r))).width(Length::FILL),
            rotulo(tr!("Adicionar subtotal a")),
            container(container(super::dialogo_interno::dlg::rolagem(marcas.width(Length::FILL))).max_height(150.0))
                .width(Length::FILL).padding([8, 10]).style(super::dialogo_interno::dlg::estilo_bloco),
        ].spacing(8);
        if let Some(erro) = &e.erro {
            painel = painel.push(row![feather_ui::icones::icone("info-circle", 15.0),
                text(erro.clone()).size(12).color(COR_ERRO())]
                .spacing(6).align(pelicano::Align::Center));
        }
        self.dialogo(tr!("Subtotal"), painel)
            .icone("sum")
            .subtitulo(sub)
            .largura(460.0)
            .ao_fechar(Msg::Topicos(Evento::SubtotalFechar))
            .rodape_esquerda(super::dialogo_interno::dlg::botao_secundario(tr!("Remover todos"),
                Some(Msg::Topicos(Evento::SubtotalRemover))))
            .secundario(tr!("Cancelar"), Some(Msg::Topicos(Evento::SubtotalFechar)))
            .primario("OK", (!e.marcadas.is_empty()).then_some(Msg::Topicos(Evento::SubtotalAplicar)))
    }

    pub(super) fn sobrepor_subtotal<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if self.subtotal.is_none() { return base; }
        self.view_subtotal().sobre(base)
    }

    /// O grupo "Estrutura de tópicos" da aba Dados.
    pub(super) fn grupo_estrutura_topicos(&self) -> Element<'_, Msg> {
        let tem = self.workbook.aba(self.folha).is_some_and(|a| !a.sheet.topicos.vazio());
        self.faixa_grupo(tr!("Estrutura de tópicos"), self.faixa_linha(vec![
            self.faixa_grande(tr!("Agrupar"), "sitemap", Some(Msg::Topicos(Evento::Agrupar)), false),
            self.faixa_grande(tr!("Desagrupar"), "list", Some(Msg::Topicos(Evento::Desagrupar)), false),
            self.faixa_grande(tr!("Subtotal"), "sum", Some(Msg::Topicos(Evento::SubtotalAbrir)), self.subtotal.is_some()),
            self.faixa_pilha(vec![
                self.faixa_pequeno(tr!("Mostrar detalhe"), "plus", tem.then_some(Msg::Topicos(Evento::MostrarDetalhe)), false),
                self.faixa_pequeno(tr!("Ocultar detalhe"), "menos", tem.then_some(Msg::Topicos(Evento::OcultarDetalhe)), false),
                self.faixa_pequeno(tr!("Limpar estrutura"), "eraser", tem.then_some(Msg::Topicos(Evento::Limpar)), false),
            ]),
        ]))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn raw(app: &App, r: u32, c: u32) -> String { app.workbook.aba(0).unwrap().sheet.get_raw(r, c) }

    #[test]
    fn agrupar_recolher_pela_grade_e_desfazer() {
        let mut app = App::default();
        let _ = app.update(Msg::Selecionar(1, 0));
        let _ = app.update(Msg::Estender(3, 0));
        let _ = app.update(Msg::Topicos(Evento::Agrupar));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.topicos.linhas.nivel(2), 1, "{}", app.status);
        let _ = app.update(Msg::Topicos(Evento::Alternar(false, 1, 4)));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.row_heights.get(&2), Some(&0));
        let _ = app.update(Msg::Topicos(Evento::Nivel(false, 2)));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.row_heights.get(&2), None);
        let _ = app.update(Msg::Desfazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.row_heights.get(&2), Some(&0));
    }

    #[test]
    fn subtotal_pela_janela_e_remover_todos() {
        let mut app = App::default();
        for (r, (a, b)) in [("Região", "Venda"), ("Norte", "3"), ("Norte", "4"), ("Sul", "5")].iter().enumerate() {
            app.workbook.escrever_celula(0, r as u32, 0, (*a).into());
            app.workbook.escrever_celula(0, r as u32, 1, (*b).into());
        }
        let _ = app.update(Msg::Selecionar(1, 0));
        let _ = app.update(Msg::Topicos(Evento::SubtotalAbrir));
        let e = app.subtotal.as_ref().expect("janela aberta");
        assert_eq!((e.grupo, e.funcao, e.marcadas.clone()), (0, 9, vec![1]));
        assert_eq!(e.colunas[1].1, "Venda");
        let _ = app.update(Msg::Topicos(Evento::SubtotalAplicar));
        assert!(app.subtotal.is_none());
        assert_eq!(raw(&app, 3, 0), "Norte Total");
        assert_eq!(raw(&app, 6, 0), "Total Geral");
        let _ = app.update(Msg::Topicos(Evento::SubtotalAbrir));
        let _ = app.update(Msg::Topicos(Evento::SubtotalRemover));
        assert_eq!((raw(&app, 3, 0), raw(&app, 4, 0)), ("Sul".into(), "".into()));
    }

    #[test]
    fn icones_do_grupo_existem() {
        for nome in ["sitemap", "list", "sum", "plus", "minus", "eraser", "info-circle"] {
            assert!(feather_ui::icones::existe(nome), "{nome}");
        }
    }
}
