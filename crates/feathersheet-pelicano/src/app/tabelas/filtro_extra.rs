//! Submenus do menu da coluna (AutoFiltro/tabela) iguais aos do Excel:
//! "Classificar por cor ▸", "Classificar por lista ▸", "Filtrar por cor ▸",
//! "Filtros de número ▸" (10 primeiros, média) e "Filtros de data ▸"
//! (períodos dinâmicos). As regras moram no motor (`operacoes::filtro`,
//! `filtro_data`, `ordenacao_ext`); aqui só se monta o JSON do critério
//! (`feathersheet_io::filtro_json`) e se desenha.
//!
//! Os submenus usam o mesmo flyout de "Filtrar por condição"
//! (`tabela_filtro_pagina`), ancorado ao lado do item que os abriu.
use super::{area_de, sob_id, Acao};
use crate::app::{faixa_nativa, App, Msg};
use feather_ui::icones::icone;
use feather_ui::tema::{
    estilo_botao_primario, estilo_botao_secundario, estilo_entrada_suspensa, estilo_item_menu,
    estilo_linha, estilo_popup_menu, COR_AZUL_ACENTO, COR_AZUL_FUNDO, COR_BORDA, COR_PAINEL,
    COR_TEXTO, COR_TEXTO_MUTED, LARGURA_SUBMENU,
};
use feathersheet_core::celulas::style::NumberFormat;
use feathersheet_core::celulas::values::Value;
use feathersheet_core::operacoes::filtro::{self, AlvoCor};
use feathersheet_core::operacoes::filtro_data::{self, Dinamico};
use feathersheet_core::operacoes::ordenacao_ext::{self, CriterioOrdem};
use feathersheet_core::planilha::sheet::Sheet;
use pelicano::button; use pelicano::column; use pelicano::container; use pelicano::row; use pelicano::scrollable; use pelicano::text; use pelicano::text_input; use pelicano::Space;
use pelicano::Background; use pelicano::Border; use pelicano::Color; use pelicano::Element; use pelicano::Length;
use serde_json::{json, Value as Json};

/// Páginas do flyout (1 e 2 são "Filtrar por condição").
pub(in crate::app) const PAG_COR: u8 = 3;
pub(in crate::app) const PAG_NUMERO: u8 = 4;
pub(in crate::app) const PAG_TOP10: u8 = 5;
pub(in crate::app) const PAG_DATA: u8 = 6;
pub(in crate::app) const PAG_PERIODO: u8 = 7;
pub(in crate::app) const PAG_ORD_COR: u8 = 8;
pub(in crate::app) const PAG_ORD_LISTA: u8 = 9;

/// Linhas lidas para descobrir cores e tipo da coluna ao abrir o menu. O
/// filtro em si sempre olha a coluna inteira.
const LINHAS_AMOSTRA_CORES: u32 = 100_000;
const LINHAS_AMOSTRA_TIPO: u32 = 8_192;

/// O que a coluna guarda, para mostrar "Filtros de número" ou "de data".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::app) enum TipoColuna {
    #[default]
    Texto,
    Numero,
    Data,
}

/// Estado dos submenus novos (um campo só no `App`).
#[derive(Debug, Clone)]
pub(in crate::app) struct Estado {
    pub tipo: TipoColuna,
    pub fundos: Vec<Option<String>>,
    pub fontes: Vec<Option<String>>,
    /// "10 primeiros": maiores (true) ou menores, quantidade e se é %.
    pub top_topo: bool,
    pub top_n: String,
    pub top_percentual: bool,
    /// Lista própria digitada em "Classificar por lista".
    pub lista_texto: String,
    /// Página para onde "Voltar" leva depois de "Configurar condição".
    pub voltar: u8,
}

impl Default for Estado {
    fn default() -> Self {
        Estado {
            tipo: TipoColuna::Texto,
            fundos: Vec::new(),
            fontes: Vec::new(),
            top_topo: true,
            top_n: "10".into(),
            top_percentual: false,
            lista_texto: String::new(),
            voltar: 1,
        }
    }
}

impl Estado {
    /// Se a coluna tem alguma cor de preenchimento ou de fonte.
    pub fn tem_cor(&self) -> bool {
        self.fundos.iter().chain(&self.fontes).any(Option::is_some)
    }
}

/// O serial do Excel do dia de hoje no fuso do sistema. No Linux usa
/// `localtime_r`; nos outros sistemas, por ora, o dia em UTC.
pub(in crate::app) fn serial_de_hoje() -> i64 {
    let segundos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        let s = segundos as libc::time_t;
        // SAFETY: `localtime_r` só escreve na `tm` que passamos.
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        if !unsafe { libc::localtime_r(&s, &mut tm) }.is_null() {
            return filtro_data::serial_de_data(tm.tm_year + 1900, (tm.tm_mon + 1) as u32, tm.tm_mday as u32);
        }
    }
    // 25569 = serial de 01/01/1970.
    (segundos / 86_400) as i64 + 25_569
}

fn formato_de_data(sheet: &Sheet, r: u32, c: u32) -> bool {
    sheet.estilo_resolvido(r, c).is_some_and(|s| {
        matches!(s.format, NumberFormat::Date | NumberFormat::Datetime)
            || s.code.as_deref().is_some_and(|code| {
                let c = code.to_lowercase();
                c.contains("yy") || c.contains("dd") || c.contains("mmm")
            })
    })
}

/// Tipo e cores da coluna `col` nas linhas de dados `r0..=r1`.
pub(in crate::app) fn analisar(
    sheet: &Sheet, valor: &dyn Fn(u32, u32) -> Value, r0: u32, r1: u32, col: u32,
) -> (TipoColuna, Vec<Option<String>>, Vec<Option<String>>) {
    if r0 > r1 {
        return (TipoColuna::Texto, Vec::new(), Vec::new());
    }
    let (mut numeros, mut textos, mut datas) = (0u32, 0u32, 0u32);
    for r in r0..=r1.min(r0.saturating_add(LINHAS_AMOSTRA_TIPO - 1)) {
        match valor(r, col) {
            Value::Number(_) => {
                numeros += 1;
                if formato_de_data(sheet, r, col) { datas += 1; }
            }
            Value::Text(_) | Value::Bool(_) => textos += 1,
            _ => {}
        }
    }
    let tipo = if numeros == 0 || textos > numeros {
        TipoColuna::Texto
    } else if datas * 2 >= numeros {
        TipoColuna::Data
    } else {
        TipoColuna::Numero
    };
    let fim = r1.min(r0.saturating_add(LINHAS_AMOSTRA_CORES - 1));
    let (fundos, fontes) = filtro::cores_da_coluna(r0, fim, col, &|r, c| filtro::cores_da_celula(sheet, r, c));
    (tipo, fundos, fontes)
}

/// `#rrggbb` em cor do Iced (cor de CONTEÚDO: a da própria célula).
fn cor_de_hex(hex: &str) -> Option<Color> {
    let h = filtro::normalizar_cor(hex);
    if h.len() != 6 { return None; }
    let n = u32::from_str_radix(&h, 16).ok()?;
    Some(Color::rgb8((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

/// Itens de "Filtros de data", na ordem do Excel; `None` = divisor.
const DATAS: &[Option<(&str, Dinamico)>] = &[
    Some(("Hoje", Dinamico::Hoje)),
    Some(("Ontem", Dinamico::Ontem)),
    Some(("Amanhã", Dinamico::Amanha)),
    None,
    Some(("Esta semana", Dinamico::EstaSemana)),
    Some(("Semana passada", Dinamico::SemanaPassada)),
    Some(("Próxima semana", Dinamico::ProximaSemana)),
    None,
    Some(("Este mês", Dinamico::EsteMes)),
    Some(("Mês passado", Dinamico::MesPassado)),
    Some(("Próximo mês", Dinamico::ProximoMes)),
    None,
    Some(("Este trimestre", Dinamico::EsteTrimestre)),
    Some(("Trimestre passado", Dinamico::TrimestrePassado)),
    Some(("Próximo trimestre", Dinamico::ProximoTrimestre)),
    None,
    Some(("Este ano", Dinamico::EsteAno)),
    Some(("Ano passado", Dinamico::AnoPassado)),
    Some(("Próximo ano", Dinamico::ProximoAno)),
    None,
    Some(("Acumulado no ano", Dinamico::AnoAteHoje)),
];

const MESES: [&str; 12] = ["Janeiro", "Fevereiro", "Março", "Abril", "Maio", "Junho",
    "Julho", "Agosto", "Setembro", "Outubro", "Novembro", "Dezembro"];

/// JSON do critério de período dinâmico.
pub(in crate::app) fn criterio_dinamico(d: Dinamico) -> Json {
    json!({"op": "dinamico", "tipo": d.nome_xlsx()})
}

/// JSON do critério por cor.
pub(in crate::app) fn criterio_cor(alvo: AlvoCor, cor: Option<String>) -> Json {
    json!({"op": if alvo == AlvoCor::Fundo { "cor_fundo" } else { "cor_fonte" }, "cor": cor})
}

fn divisor() -> Element<'static, Msg> {
    container(container(Space::new(0.0, 1)).width(Length::FILL).style(estilo_linha))
        .padding([5, 0]).width(Length::FILL).into()
}

fn secao(rotulo: &str) -> Element<'static, Msg> {
    container(text(rotulo.to_owned()).size(11).color(COR_TEXTO_MUTED()))
        .padding([4, 10]).into()
}

/// Linha clicável do flyout; `ativo` marca o critério em uso.
fn item(rotulo: impl Into<String>, acao: Option<Acao>, ativo: bool) -> Element<'static, Msg> {
    let marca: Element<'static, Msg> = if ativo { icone("checks", 14.0).into() }
        else { Space::new(14, 0.0).into() };
    button(row![text(rotulo.into()).size(13), Space::new(Length::FILL, 0.0), marca]
        .align(pelicano::Align::Center))
        .width(Length::FILL)
        .height(Length::Fixed(28.0))
        .padding([4, 10])
        .style(estilo_item_menu(ativo, acao.is_some()))
        .on_press_maybe(acao.map(Msg::TabelaAcao))
        .into()
}

/// Item que abre outro nível (seta à direita).
fn item_sub(rotulo: &str, acao: Acao) -> Element<'static, Msg> {
    button(row![text(rotulo.to_owned()).size(13), Space::new(Length::FILL, 0.0),
        icone("chevron-right", 14.0)].align(pelicano::Align::Center))
        .width(Length::FILL)
        .height(Length::Fixed(28.0))
        .padding([4, 10])
        .style(estilo_item_menu(false, true))
        .on_press(Msg::TabelaAcao(acao))
        .into()
}

/// Amostra da cor: barra preenchida (fundo), "Abc" colorido (fonte) ou
/// quadro vazio para "Sem preenchimento" / "Automático".
fn amostra(alvo: AlvoCor, cor: Option<&str>) -> Element<'static, Msg> {
    let c = cor.and_then(cor_de_hex);
    if alvo == AlvoCor::Fonte {
        return container(text("Abc").size(12.5).color(c.unwrap_or(COR_TEXTO())))
            .width(Length::Fixed(34.0)).into();
    }
    container(Space::new(Length::Fixed(34.0), Length::Fixed(14.0)))
        .style(move |_| pelicano::Style {
            background: Some(Background::Color(c.unwrap_or(COR_PAINEL()))),
            border: Border { color: COR_BORDA(), width: 1.0, radius: [3.0; 4] },
            ..Default::default()
        })
        .into()
}

fn item_cor(alvo: AlvoCor, cor: &Option<String>, acao: Acao, ativo: bool) -> Element<'static, Msg> {
    let rotulo = match (cor, alvo) {
        (Some(h), _) => h.to_uppercase(),
        (None, AlvoCor::Fundo) => tr!("Sem preenchimento").into(),
        (None, AlvoCor::Fonte) => tr!("Automático").into(),
    };
    let marca: Element<'static, Msg> = if ativo { icone("checks", 14.0).into() }
        else { Space::new(14, 0.0).into() };
    button(row![amostra(alvo, cor.as_deref()), text(rotulo).size(12).color(COR_TEXTO_MUTED()),
        Space::new(Length::FILL, 0.0), marca].spacing(9).align(pelicano::Align::Center))
        .width(Length::FILL)
        .height(Length::Fixed(28.0))
        .padding([4, 10])
        .style(estilo_item_menu(ativo, true))
        .on_press(Msg::TabelaAcao(acao))
        .into()
}

/// Botão de alternância do formulário "10 primeiros".
fn chip(rotulo: &str, ligado: bool, acao: Acao) -> Element<'static, Msg> {
    button(text(rotulo.to_owned()).size(12).color(if ligado { COR_AZUL_ACENTO() } else { COR_TEXTO() }))
        .padding([5, 10])
        .style(move |t, s| {
            let mut e = estilo_item_menu(ligado, true)(t, s);
            if ligado { e.background = Some(Background::Color(COR_AZUL_FUNDO())); }
            e.border = Border { color: if ligado { COR_AZUL_ACENTO() } else { COR_BORDA() }, width: 1.0, radius: [5.0; 4] };
            e
        })
        .on_press(Msg::TabelaAcao(acao))
        .into()
}

/// Altura de um item do menu principal + o espaçamento da coluna.
const PASSO_ITEM: f32 = 28.0 + 3.0;
/// Topo do primeiro item (padding 8 + cabeçalho + divisor + espaçamentos).
const TOPO_ITENS: f32 = 45.0;
/// Divisor do meio (13 px + espaçamento).
const PASSO_DIVISOR: f32 = 16.0;

impl App {
    /// Recalcula tipo e cores da coluna (ao abrir o menu da coluna).
    pub(in crate::app) fn analisar_coluna_filtro(&mut self) {
        let mut estado = Estado {
            top_topo: self.tabela_filtro_extra.top_topo,
            top_n: self.tabela_filtro_extra.top_n.clone(),
            top_percentual: self.tabela_filtro_extra.top_percentual,
            lista_texto: self.tabela_filtro_extra.lista_texto.clone(),
            ..Estado::default()
        };
        if let Some(t) = self.tabela_editando.as_deref().and_then(|id| sob_id(&self.workbook, self.folha, id).ok()) {
            if let (Some(area), Some(aba)) = (area_de(&t), self.workbook.aba(self.folha)) {
                let col = self.tabela_col_filtro.clamp(area.c0, area.c1);
                let r0 = area.r0.saturating_add(u32::from(t["temCabecalho"] == true));
                let r1 = area.r1.min(aba.sheet.max_row);
                let w = &self.workbook;
                let folha = self.folha;
                let (tipo, fundos, fontes) = analisar(&aba.sheet, &|r, c| w.valor(folha, r, c), r0, r1, col);
                estado.tipo = tipo;
                estado.fundos = fundos;
                estado.fontes = fontes;
            }
        }
        self.tabela_filtro_extra = estado;
    }

    /// Itens novos do menu principal, logo abaixo de "Classificar de Z a A".
    pub(in crate::app) fn itens_classificar_extra(&self) -> [Element<'_, Msg>; 2] {
        let tem_cor = self.tabela_filtro_extra.tem_cor();
        [
            submenu_principal("bucket", tr!("Classificar por cor"), tem_cor.then_some(Acao::Submenu(PAG_ORD_COR))),
            submenu_principal("list", tr!("Classificar por lista"), Some(Acao::Submenu(PAG_ORD_LISTA))),
        ]
    }

    /// Itens novos do menu principal, antes de "Filtrar por condição".
    pub(in crate::app) fn itens_filtro_extra(&self) -> Vec<Element<'_, Msg>> {
        let e = &self.tabela_filtro_extra;
        let mut v = vec![submenu_principal("bucket", tr!("Filtrar por cor"), e.tem_cor().then_some(Acao::Submenu(PAG_COR)))];
        match e.tipo {
            TipoColuna::Numero => v.push(submenu_principal("list-numbers", tr!("Filtros de número"), Some(Acao::Submenu(PAG_NUMERO)))),
            TipoColuna::Data => v.push(submenu_principal("clock", tr!("Filtros de data"), Some(Acao::Submenu(PAG_DATA)))),
            TipoColuna::Texto => {}
        }
        v
    }

    /// Como um valor da lista de caixinhas aparece: numa coluna de datas,
    /// o serial vira `dd/mm/aaaa` (a chave do filtro continua o serial).
    pub(in crate::app) fn rotulo_valor_lista(&self, valor: &str) -> String {
        if self.tabela_filtro_extra.tipo == TipoColuna::Data {
            if let Ok(n) = valor.parse::<f64>() {
                if n.is_finite() && n >= 1.0 {
                    let (a, m, d) = feathersheet_core::celulas::date_serial::serial_to_ymd(n.floor() as i64);
                    return format!("{d:02}/{m:02}/{a}");
                }
            }
        }
        valor.to_owned()
    }

    /// Se o valor da lista bate com a pesquisa (pelo texto ou pela data mostrada).
    pub(in crate::app) fn valor_bate_busca(&self, valor: &str, busca: &str) -> bool {
        busca.is_empty() || valor.to_lowercase().contains(busca)
            || (self.tabela_filtro_extra.tipo == TipoColuna::Data
                && self.rotulo_valor_lista(valor).contains(busca))
    }

    /// Quantos itens [`Self::itens_filtro_extra`] põe no menu.
    pub(in crate::app) fn n_itens_filtro_extra(&self) -> usize {
        1 + usize::from(self.tabela_filtro_extra.tipo != TipoColuna::Texto)
    }

    /// Distância do topo do menu principal até o item que abriu a página.
    fn deslocamento_item(&self, pagina: u8) -> f32 {
        let extra = u32::from(self.tabela_filtro_extra.tipo != TipoColuna::Texto) as f32;
        // A-Z, Z-A, por cor, por lista | divisor | limpar, limpar todos,
        // por cor, [número/data], condição.
        let depois_divisor = TOPO_ITENS + 4.0 * PASSO_ITEM + PASSO_DIVISOR;
        match pagina {
            PAG_ORD_COR => TOPO_ITENS + 2.0 * PASSO_ITEM,
            PAG_ORD_LISTA => TOPO_ITENS + 3.0 * PASSO_ITEM,
            PAG_COR => depois_divisor + 2.0 * PASSO_ITEM,
            PAG_NUMERO | PAG_TOP10 | PAG_DATA | PAG_PERIODO => depois_divisor + 3.0 * PASSO_ITEM,
            _ => depois_divisor + (3.0 + extra) * PASSO_ITEM,
        }
    }

    /// Topo do menu principal na janela: a mesma conta da camada que o
    /// desenha (`interface_editor.rs`).
    pub(in crate::app) fn topo_menu_principal(&self) -> f32 {
        let topo_grade = 43.0 + 52.0
            + if self.config.faixa_recolhida { 0.0 } else { faixa_nativa::ALTURA_FAIXA_NATIVA }
            + if self.mostrar_fx { 54.0 } else { 0.0 };
        (topo_grade + self.tabela_popup_y + 4.0).min((self.tamanho_principal.height - 490.0 - 8.0).max(8.0))
    }

    /// Espaço acima do flyout para ele abrir na altura do item que o
    /// chamou (a camada o põe em `topo + 115`), sem passar do pé da janela.
    pub(in crate::app) fn recuo_flyout(&self, altura: f32) -> f32 {
        let pagina = self.tabela_filtro_pagina;
        let topo = self.topo_menu_principal();
        let base = self.base_flyout();
        let desejado = topo + self.deslocamento_item(pagina) - 7.0;
        let maximo = (self.tamanho_principal.height - 8.0 - altura - base).max(0.0);
        (desejado - base).clamp(0.0, maximo)
    }

    /// Onde a camada põe o topo do flyout (`interface_editor.rs`).
    fn base_flyout(&self) -> f32 {
        let altura_camada = if self.tabela_filtro_pagina == 1 { 310.0 } else { 180.0 };
        (self.topo_menu_principal() + 115.0)
            .clamp(0.0, (self.tamanho_principal.height - altura_camada - 8.0).max(0.0))
    }

    /// Altura da lista rolável de um flyout longo (datas, meses), para ele
    /// terminar dentro da janela.
    fn altura_lista_flyout(&self, maximo: f32) -> f32 {
        (self.tamanho_principal.height - 8.0 - self.base_flyout() - 64.0).clamp(140.0, maximo)
    }

    /// Critério da coluna em uso (para marcar o item ativo).
    fn criterio_atual(&self) -> Option<&Json> {
        self.tabela_filtro_original.as_ref()
    }

    /// Os flyouts das páginas 3 a 9.
    pub(in crate::app) fn submenu_extra(&self, pagina: u8) -> Element<'_, Msg> {
        let e = &self.tabela_filtro_extra;
        let atual = self.criterio_atual();
        let (titulo, voltar) = match pagina {
            PAG_COR => (tr!("Filtrar por cor"), false),
            PAG_NUMERO => (tr!("Filtros de número"), false),
            PAG_TOP10 => (tr!("10 primeiros"), true),
            PAG_DATA => (tr!("Filtros de data"), false),
            PAG_PERIODO => (tr!("Todas as datas no período"), true),
            PAG_ORD_COR => (tr!("Classificar por cor"), false),
            _ => (tr!("Classificar por lista"), false),
        };
        let mut cabecalho = row(Vec::<pelicano::Element<_>>::new()).spacing(4).align(pelicano::Align::Center);
        if voltar {
            cabecalho = cabecalho.push(button(icone("chevron-left", 14.0))
                .padding([3, 5])
                .style(estilo_item_menu(false, true))
                .on_press(Msg::TabelaAcao(Acao::VoltarCondicoes)));
        }
        cabecalho = cabecalho
            .push(text(titulo).size(13).color(COR_TEXTO()))
            .push(Space::new(Length::FILL, 0.0))
            .push(button(icone("x", 14.0))
                .padding([1, 7])
                .style(estilo_item_menu(false, true))
                .on_press(Msg::TabelaAcao(Acao::FecharCondicoes)));
        let mut corpo = column![cabecalho, divisor()].spacing(2).padding(7);
        let mut altura = 60.0;
        match pagina {
            PAG_COR | PAG_ORD_COR => {
                let filtrar = pagina == PAG_COR;
                for (alvo, lista, rotulo) in [
                    (AlvoCor::Fundo, &e.fundos, if filtrar { tr!("Filtrar por cor da célula") } else { tr!("Classificar por cor da célula") }),
                    (AlvoCor::Fonte, &e.fontes, if filtrar { tr!("Filtrar por cor da fonte") } else { tr!("Classificar por cor da fonte") }),
                ] {
                    if !lista.iter().any(Option::is_some) { continue; }
                    corpo = corpo.push(secao(rotulo));
                    altura += 26.0;
                    for cor in lista {
                        let (acao, ativo) = if filtrar {
                            let crit = criterio_cor(alvo, cor.clone());
                            let ativo = atual.is_some_and(|a| filtro_json_igual(a, &crit));
                            (Acao::AplicarCriterio(crit), ativo)
                        } else {
                            (Acao::OrdenarPor(CriterioOrdem::Cor { alvo, cor: cor.clone(), em_cima: true }, true), false)
                        };
                        corpo = corpo.push(item_cor(alvo, cor, acao, ativo));
                        altura += 30.0;
                    }
                }
            }
            PAG_NUMERO => {
                for (rotulo, op) in [(tr!("É igual a…"), "eq_num"), (tr!("É diferente de…"), "neq_num"),
                    (tr!("É maior do que…"), "gt"), (tr!("É maior ou igual a…"), "gte"),
                    (tr!("É menor do que…"), "lt"), (tr!("É menor ou igual a…"), "lte")] {
                    let ativo = atual.is_some_and(|a| a["op"] == op);
                    corpo = corpo.push(item(rotulo, Some(Acao::Operador(op)), ativo));
                }
                corpo = corpo.push(divisor());
                let top = atual.is_some_and(|a| a["op"] == "top10");
                corpo = corpo.push(button(row![text(tr!("10 primeiros…")).size(13), Space::new(Length::FILL, 0.0),
                        if top { Element::from(icone("checks", 14.0)) } else { icone("chevron-right", 14.0).into() }]
                        .align(pelicano::Align::Center))
                    .width(Length::FILL).height(Length::Fixed(28.0)).padding([4, 10])
                    .style(estilo_item_menu(top, true))
                    .on_press(Msg::TabelaAcao(Acao::Submenu(PAG_TOP10))));
                for (rotulo, d) in [(tr!("Acima da média"), Dinamico::AcimaDaMedia), (tr!("Abaixo da média"), Dinamico::AbaixoDaMedia)] {
                    let crit = criterio_dinamico(d);
                    let ativo = atual.is_some_and(|a| filtro_json_igual(a, &crit));
                    corpo = corpo.push(item(rotulo, Some(Acao::AplicarCriterio(crit)), ativo));
                }
                altura += 9.0 * 30.0 + 11.0;
            }
            PAG_TOP10 => {
                corpo = corpo.push(text(tr!("Mostrar os itens")).size(11.5).color(COR_TEXTO_MUTED()));
                corpo = corpo.push(row![
                    chip(tr!("Primeiros"), e.top_topo, Acao::Top10Topo(true)),
                    chip(tr!("Últimos"), !e.top_topo, Acao::Top10Topo(false)),
                ].spacing(6));
                corpo = corpo.push(Space::new(0.0, 4));
                corpo = corpo.push(row![
                    text_input("10", &e.top_n)
                        .on_input(|v| Msg::TabelaAcao(Acao::Top10N(v)))
                        .on_submit(Msg::TabelaAcao(Acao::AplicarTop10))
                        .style(estilo_entrada_suspensa)
                        .size(12.5).padding([6, 8]).width(Length::Fixed(64.0)),
                    chip(tr!("Itens"), !e.top_percentual, Acao::Top10Percentual(false)),
                    chip(tr!("Porcentagem"), e.top_percentual, Acao::Top10Percentual(true)),
                ].spacing(6).align(pelicano::Align::Center));
                corpo = corpo.push(divisor());
                corpo = corpo.push(row![
                    button(text(tr!("Cancelar")).size(12)).padding([6, 9])
                        .style(estilo_botao_secundario)
                        .on_press(Msg::TabelaAcao(Acao::FecharCondicoes)),
                    Space::new(Length::FILL, 0.0),
                    button(text(tr!("Aplicar")).size(12)).padding([6, 12])
                        .style(estilo_botao_primario)
                        .on_press(Msg::TabelaAcao(Acao::AplicarTop10)),
                ].align(pelicano::Align::Center));
                altura += 150.0;
            }
            PAG_DATA => {
                let mut lista = column(Vec::<pelicano::Element<_>>::new()).spacing(1);
                for linha in DATAS {
                    lista = match linha {
                        None => lista.push(divisor()),
                        Some((rotulo, d)) => {
                            let crit = criterio_dinamico(*d);
                            let ativo = atual.is_some_and(|a| filtro_json_igual(a, &crit));
                            lista.push(item(tr!(*rotulo), Some(Acao::AplicarCriterio(crit)), ativo))
                        }
                    };
                }
                lista = lista.push(divisor());
                lista = lista.push(item_sub(tr!("Todas as datas no período"), Acao::Submenu(PAG_PERIODO)));
                let alto = self.altura_lista_flyout(560.0);
                corpo = corpo.push(scrollable(lista).height(Length::Fixed(alto)));
                altura += alto;
            }
            PAG_PERIODO => {
                let mut lista = column(Vec::<pelicano::Element<_>>::new()).spacing(1);
                for q in 1..=4u8 {
                    let crit = criterio_dinamico(Dinamico::Trimestre(q));
                    let ativo = atual.is_some_and(|a| filtro_json_igual(a, &crit));
                    lista = lista.push(item(trf!("Trimestre {q}"), Some(Acao::AplicarCriterio(crit)), ativo));
                }
                lista = lista.push(divisor());
                for (i, mes) in MESES.iter().enumerate() {
                    let crit = criterio_dinamico(Dinamico::Mes(i as u8 + 1));
                    let ativo = atual.is_some_and(|a| filtro_json_igual(a, &crit));
                    lista = lista.push(item(tr!(*mes), Some(Acao::AplicarCriterio(crit)), ativo));
                }
                let alto = self.altura_lista_flyout(520.0);
                corpo = corpo.push(scrollable(lista).height(Length::Fixed(alto)));
                altura += alto;
            }
            _ => {
                corpo = corpo.push(secao(tr!("Listas prontas")));
                for (nome, itens) in ordenacao_ext::listas_prontas() {
                    // Os primeiros itens que cabem na linha, e reticências.
                    let mut rotulo = String::new();
                    for parte in nome.split(", ") {
                        if !rotulo.is_empty() && rotulo.chars().count() + parte.chars().count() > 26 { break; }
                        if !rotulo.is_empty() { rotulo.push_str(", "); }
                        rotulo.push_str(parte);
                    }
                    rotulo.push('…');
                    corpo = corpo.push(item(rotulo, Some(Acao::OrdenarPor(CriterioOrdem::Lista(itens), true)), false));
                }
                corpo = corpo.push(divisor());
                corpo = corpo.push(secao(tr!("Lista própria (separe por ;)")));
                let itens = ordenacao_ext::lista_de_texto(&e.lista_texto);
                let pronto = (!itens.is_empty()).then(|| Msg::TabelaAcao(Acao::OrdenarPor(CriterioOrdem::Lista(itens), true)));
                let campo = text_input(tr!("baixa; média; alta"), &e.lista_texto)
                    .on_input(|v| Msg::TabelaAcao(Acao::ListaTexto(v)));
                // O pelicano não tem `on_submit_maybe`.
                let campo = match pronto.clone() { Some(m) => campo.on_submit(m), None => campo };
                corpo = corpo.push(row![
                    campo
                        .style(estilo_entrada_suspensa)
                        .size(12).padding([6, 8]).width(Length::FILL),
                    button(text(tr!("Classificar")).size(12)).padding([6, 10])
                        .style(estilo_botao_primario)
                        .on_press_maybe(pronto),
                ].spacing(6).align(pelicano::Align::Center));
                altura += 26.0 * 2.0 + 4.0 * 30.0 + 50.0;
            }
        }
        let recuo = self.recuo_flyout(altura);
        column![
            Space::new(0.0, Length::Fixed(recuo)),
            container(corpo).width(Length::Fixed(LARGURA_SUBMENU + 24.0)).style(estilo_popup_menu),
        ].into()
    }
}

impl App {
    /// `FEATHERSHEET_CAPTURAR_FILTRO=cor|numero|top10|data|periodo|ordcor|lista|condicao`:
    /// tabela de exemplo (Produto colorido, Valor, Data) e o menu da coluna
    /// aberto com esse submenu, para a foto. `menu` deixa só o principal.
    pub(in crate::app) fn preparar_captura_filtro(&mut self, modo: &str) {
        let hoje = serial_de_hoje();
        let data = |s: i64| {
            let (a, m, d) = feathersheet_core::celulas::date_serial::serial_to_ymd(s);
            format!("{d:02}/{m:02}/{a}")
        };
        let linhas = [(tr!("Caneta"), "12", -3, Some("#f4cccc"), None), (tr!("Caderno"), "35", 0, None, Some("#c00000")),
            (tr!("Lápis"), "4", 9, Some("#d9ead3"), None), (tr!("Borracha"), "7", -40, Some("#f4cccc"), None),
            (tr!("Mochila"), "120", 2, None, None), (tr!("Régua"), "9", -1, Some("#cfe2f3"), None)];
        self.workbook.escrever_celula(self.folha, 0, 0, tr!("Produto").into());
        self.workbook.escrever_celula(self.folha, 0, 1, tr!("Valor").into());
        self.workbook.escrever_celula(self.folha, 0, 2, tr!("Data").into());
        for (i, (nome, valor, dias, fundo, fonte)) in linhas.iter().enumerate() {
            let r = i as u32 + 1;
            self.workbook.escrever_celula(self.folha, r, 0, (*nome).into());
            self.workbook.escrever_celula(self.folha, r, 1, (*valor).into());
            self.workbook.escrever_celula(self.folha, r, 2, data(hoje + dias));
            for (campo, cor) in [("bg", fundo), ("color", fonte)] {
                if let Some(cor) = cor {
                    let mut e = feathersheet_core::celulas::estilo_parcial::EstiloParcial::new();
                    e.insert(campo.into(), feathersheet_core::celulas::estilo_parcial::Campo::Texto((*cor).into()));
                    self.workbook.editar(self.folha, |s| s.set_style(&feathersheet_core::celulas::refs::Rect::cell(r, 0), &e));
                }
            }
        }
        let area = feathersheet_core::celulas::refs::Rect::new(0, 0, linhas.len() as u32, 2);
        if super::criar(&mut self.workbook, self.folha, area, "Tabela1", true, false).is_err() { return; }
        let (col, pagina) = match modo {
            "cor" => (0, PAG_COR),
            "ordcor" => (0, PAG_ORD_COR),
            "lista" => (0, PAG_ORD_LISTA),
            "numero" => (1, PAG_NUMERO),
            "top10" => (1, PAG_TOP10),
            "data" => (2, PAG_DATA),
            "periodo" => (2, PAG_PERIODO),
            "condicao" => (0, 1),
            _ => (0, 0),
        };
        let _ = self.acao_tabela(Acao::FiltroCabecalho(0, col, 130.0 + col as f32 * 100.0, 20.0));
        if pagina == PAG_TOP10 { let _ = self.acao_tabela(Acao::Submenu(PAG_NUMERO)); }
        if pagina == PAG_PERIODO { let _ = self.acao_tabela(Acao::Submenu(PAG_DATA)); }
        if pagina != 0 { let _ = self.acao_tabela(Acao::Submenu(pagina)); }
    }
}

/// Item do menu principal que abre um submenu (ícone, rótulo, seta).
fn submenu_principal(nome_icone: &str, rotulo: &str, acao: Option<Acao>) -> Element<'static, Msg> {
    let disponivel = acao.is_some();
    button(row![icone(nome_icone, 15.0), text(rotulo.to_owned()).size(13),
        Space::new(Length::FILL, 0.0), icone("chevron-right", 14.0)]
        .spacing(8).align(pelicano::Align::Center))
        .width(Length::FILL)
        .height(Length::Fixed(28.0))
        .padding([4, 10])
        .style(estilo_item_menu(false, disponivel))
        .on_press_maybe(acao.map(Msg::TabelaAcao))
        .into()
}

/// Mesmo critério, comparando pelo tipo do motor (cor em qualquer grafia).
fn filtro_json_igual(a: &Json, b: &Json) -> bool {
    feathersheet_io::filtro_json::criterio_de_json(a) == feathersheet_io::filtro_json::criterio_de_json(b)
        && a["op"] == b["op"]
        && a["cor"].as_str().map(filtro::normalizar_cor) == b["cor"].as_str().map(filtro::normalizar_cor)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::app::App;
    use feathersheet_core::celulas::estilo_parcial::{Campo, EstiloParcial};
    use feathersheet_core::celulas::refs::Rect;

    /// Tabela "Tabela1" em A1:A(n+1) com cabeçalho "Valor" e os dados dados.
    fn app_com_coluna(dados: &[&str]) -> App {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "Valor".into());
        for (i, d) in dados.iter().enumerate() {
            app.workbook.escrever_celula(0, i as u32 + 1, 0, (*d).into());
        }
        super::super::criar(&mut app.workbook, 0, Rect::new(0, 0, dados.len() as u32, 0),
            "Tabela1", true, false).unwrap();
        app
    }

    fn pintar(app: &mut App, r: u32, campo: &str, cor: &str) {
        let mut e = EstiloParcial::new();
        e.insert(campo.into(), Campo::Texto(cor.into()));
        app.workbook.editar(0, |s| s.set_style(&Rect::cell(r, 0), &e));
    }

    fn escondidas(app: &App) -> Vec<u32> {
        let s = &app.workbook.aba(0).unwrap().sheet;
        (1..=8).filter(|r| s.row_heights.get(r) == Some(&0)).collect()
    }

    fn coluna(app: &App, n: u32) -> Vec<String> {
        (1..=n).map(|r| app.workbook.aba(0).unwrap().sheet.get_raw(r, 0)).collect()
    }

    #[test]
    fn filtrar_por_cor_esconde_as_outras_e_desfaz_num_passo() {
        let mut app = app_com_coluna(&["a", "b", "c", "d"]);
        pintar(&mut app, 1, "bg", "#ff0000");
        pintar(&mut app, 3, "bg", "#FF0000");
        pintar(&mut app, 2, "color", "#0000ff");
        let _ = app.acao_tabela(Acao::FiltroCabecalho(0, 0, 120.0, 20.0));
        let e = &app.tabela_filtro_extra;
        assert!(e.tem_cor());
        assert_eq!(e.fundos, vec![Some("#ff0000".to_string()), None]);
        assert!(e.fontes.contains(&Some("#0000ff".to_string())));
        assert_eq!(e.tipo, TipoColuna::Texto);
        let _ = app.acao_tabela(Acao::Submenu(PAG_COR));
        assert_eq!(app.tabela_filtro_pagina, PAG_COR);
        {
            let _menu: Element<'_, Msg> = app.menu_filtro_tabela();
            let _sub: Element<'_, Msg> = app.menu_condicoes_tabela();
        }
        let _ = app.acao_tabela(Acao::AplicarCriterio(criterio_cor(AlvoCor::Fundo, Some("#ff0000".into()))));
        assert!(!app.tabela_popup_filtro, "{}", app.status);
        assert_eq!(escondidas(&app), vec![2, 4]);
        // "Sem preenchimento" é o inverso.
        let _ = app.acao_tabela(Acao::FiltroCabecalho(0, 0, 120.0, 20.0));
        let _ = app.acao_tabela(Acao::AplicarCriterio(criterio_cor(AlvoCor::Fundo, None)));
        assert_eq!(escondidas(&app), vec![1, 3]);
        // Um Ctrl+Z volta ao filtro anterior (alturas e JSON juntos).
        let _ = app.update(Msg::Desfazer);
        assert_eq!(escondidas(&app), vec![2, 4]);
        let t = super::super::sob_id(&app.workbook, 0, "tab_Tabela1").unwrap();
        assert_eq!(t["filtros"][0]["criterio"]["op"], "cor_fundo");
        assert_eq!(t["filtros"][0]["criterio"]["cor"], "#ff0000");
        let _ = app.update(Msg::Desfazer);
        assert!(escondidas(&app).is_empty());
        let t = super::super::sob_id(&app.workbook, 0, "tab_Tabela1").unwrap();
        assert!(t["filtros"].as_array().is_none_or(|f| f.is_empty()));
        let _ = app.update(Msg::Refazer);
        assert_eq!(escondidas(&app), vec![2, 4]);
        // Filtro pela cor da fonte.
        let _ = app.acao_tabela(Acao::FiltroCabecalho(0, 0, 120.0, 20.0));
        let _ = app.acao_tabela(Acao::AplicarCriterio(criterio_cor(AlvoCor::Fonte, Some("#0000ff".into()))));
        assert_eq!(escondidas(&app), vec![1, 3, 4]);
    }

    #[test]
    fn dez_primeiros_itens_porcentagem_e_media() {
        let mut app = app_com_coluna(&["5", "1", "9", "3", "7"]);
        let _ = app.acao_tabela(Acao::FiltroCabecalho(0, 0, 120.0, 20.0));
        assert_eq!(app.tabela_filtro_extra.tipo, TipoColuna::Numero);
        assert!(!app.tabela_filtro_extra.tem_cor());
        let _ = app.acao_tabela(Acao::Submenu(PAG_NUMERO));
        let _ = app.acao_tabela(Acao::Submenu(PAG_TOP10));
        assert_eq!(app.tabela_filtro_pagina, PAG_TOP10);
        { let _sub: Element<'_, Msg> = app.menu_condicoes_tabela(); }
        let _ = app.acao_tabela(Acao::VoltarCondicoes);
        assert_eq!(app.tabela_filtro_pagina, PAG_NUMERO);
        let _ = app.acao_tabela(Acao::Submenu(PAG_TOP10));
        let _ = app.acao_tabela(Acao::Top10N("2x".into()));
        assert_eq!(app.tabela_filtro_extra.top_n, "2");
        let _ = app.acao_tabela(Acao::AplicarTop10);
        // Ficam 9 e 7 (linhas 3 e 5).
        assert_eq!(escondidas(&app), vec![1, 2, 4], "{}", app.status);
        let t = super::super::sob_id(&app.workbook, 0, "tab_Tabela1").unwrap();
        assert_eq!(t["filtros"][0]["criterio"], json!({"op":"top10","topo":true,"n":2,"percentual":false}));
        // Últimos 40% de 5 itens = 2 menores (1 e 3).
        let _ = app.acao_tabela(Acao::FiltroCabecalho(0, 0, 120.0, 20.0));
        let _ = app.acao_tabela(Acao::Top10Topo(false));
        let _ = app.acao_tabela(Acao::Top10Percentual(true));
        let _ = app.acao_tabela(Acao::Top10N("40".into()));
        let _ = app.acao_tabela(Acao::AplicarTop10);
        assert_eq!(escondidas(&app), vec![1, 3, 5]);
        // Zero é recusado sem mexer no filtro.
        let _ = app.acao_tabela(Acao::FiltroCabecalho(0, 0, 120.0, 20.0));
        let _ = app.acao_tabela(Acao::Top10N("0".into()));
        let _ = app.acao_tabela(Acao::AplicarTop10);
        assert!(app.status.contains("1 a 500"), "{}", app.status);
        assert_eq!(escondidas(&app), vec![1, 3, 5]);
        // Acima da média (5): 9 e 7.
        let _ = app.acao_tabela(Acao::AplicarCriterio(criterio_dinamico(Dinamico::AcimaDaMedia)));
        assert_eq!(escondidas(&app), vec![1, 2, 4]);
        let _ = app.update(Msg::Desfazer);
        assert_eq!(escondidas(&app), vec![1, 3, 5]);
    }

    #[test]
    fn filtros_de_data_usam_o_dia_de_hoje() {
        use feathersheet_core::celulas::date_serial::serial_to_ymd;
        let hoje = serial_de_hoje();
        let data = |s: i64| { let (a, m, d) = serial_to_ymd(s); format!("{d:02}/{m:02}/{a}") };
        let mut app = app_com_coluna(&[&data(hoje - 400), &data(hoje), &data(hoje + 1), &data(hoje - 1)]);
        let _ = app.acao_tabela(Acao::FiltroCabecalho(0, 0, 120.0, 20.0));
        assert_eq!(app.tabela_filtro_extra.tipo, TipoColuna::Data);
        let _ = app.acao_tabela(Acao::Submenu(PAG_DATA));
        { let _sub: Element<'_, Msg> = app.menu_condicoes_tabela(); }
        let _ = app.acao_tabela(Acao::Submenu(PAG_PERIODO));
        { let _sub: Element<'_, Msg> = app.menu_condicoes_tabela(); }
        let _ = app.acao_tabela(Acao::VoltarCondicoes);
        assert_eq!(app.tabela_filtro_pagina, PAG_DATA);
        let _ = app.acao_tabela(Acao::AplicarCriterio(criterio_dinamico(Dinamico::Hoje)));
        assert_eq!(escondidas(&app), vec![1, 3, 4], "{}", app.status);
        let t = super::super::sob_id(&app.workbook, 0, "tab_Tabela1").unwrap();
        assert_eq!(t["filtros"][0]["criterio"], json!({"op":"dinamico","tipo":"today"}));
        let _ = app.acao_tabela(Acao::FiltroCabecalho(0, 0, 120.0, 20.0));
        let _ = app.acao_tabela(Acao::AplicarCriterio(criterio_dinamico(Dinamico::Ontem)));
        assert_eq!(escondidas(&app), vec![1, 2, 3]);
        // Mês de hoje, em qualquer ano: o de 400 dias atrás quase nunca cai nele.
        let (_, mes, _) = serial_to_ymd(hoje);
        let _ = app.acao_tabela(Acao::FiltroCabecalho(0, 0, 120.0, 20.0));
        let _ = app.acao_tabela(Acao::AplicarCriterio(criterio_dinamico(Dinamico::Mes(mes as u8))));
        assert!(!escondidas(&app).contains(&2));
        let _ = app.update(Msg::Desfazer);
        assert_eq!(escondidas(&app), vec![1, 2, 3]);
        // A lista de caixinhas mostra a data, e a pesquisa acha por ela.
        let _ = app.acao_tabela(Acao::FiltroCabecalho(0, 0, 120.0, 20.0));
        assert_eq!(app.rotulo_valor_lista(&app.tabela_filtro_lista[1]), data(hoje));
        let _ = app.acao_tabela(Acao::BuscaLista(data(hoje)[..5].to_string()));
        assert_eq!(app.tabela_filtro_indices, vec![1]);
        // Condição com a data digitada: "maior do que" ontem deixa hoje e amanhã.
        let _ = app.acao_tabela(Acao::Operador("gt"));
        let _ = app.acao_tabela(Acao::Criterio(data(hoje - 1)));
        let _ = app.acao_tabela(Acao::AplicarFiltro);
        assert_eq!(escondidas(&app), vec![1, 4], "{}", app.status);
        // "Igual" com data compara como número.
        let _ = app.acao_tabela(Acao::FiltroCabecalho(0, 0, 120.0, 20.0));
        let _ = app.acao_tabela(Acao::Operador("equals_text"));
        let _ = app.acao_tabela(Acao::Criterio(data(hoje)));
        let _ = app.acao_tabela(Acao::AplicarFiltro);
        assert_eq!(escondidas(&app), vec![1, 3, 4], "{}", app.status);
    }

    #[test]
    fn classificar_por_cor_e_por_lista_desfaz_num_passo() {
        let mut app = app_com_coluna(&["a", "b", "c", "d"]);
        pintar(&mut app, 3, "bg", "#00ff00");
        let _ = app.acao_tabela(Acao::FiltroCabecalho(0, 0, 120.0, 20.0));
        let _ = app.acao_tabela(Acao::Submenu(PAG_ORD_COR));
        { let _sub: Element<'_, Msg> = app.menu_condicoes_tabela(); }
        let _ = app.acao_tabela(Acao::OrdenarPor(CriterioOrdem::Cor { alvo: AlvoCor::Fundo, cor: Some("#00ff00".into()), em_cima: true }, true));
        assert_eq!(coluna(&app, 4), ["c", "a", "b", "d"], "{}", app.status);
        let t = super::super::sob_id(&app.workbook, 0, "tab_Tabela1").unwrap();
        assert_eq!(t["ordenacao"]["niveis"][0]["por"], "cor_fundo");
        let _ = app.update(Msg::Desfazer);
        assert_eq!(coluna(&app, 4), ["a", "b", "c", "d"]);

        let mut app = app_com_coluna(&["qua", "seg", "dom", "sáb"]);
        let _ = app.acao_tabela(Acao::FiltroCabecalho(0, 0, 120.0, 20.0));
        let _ = app.acao_tabela(Acao::Submenu(PAG_ORD_LISTA));
        { let _sub: Element<'_, Msg> = app.menu_condicoes_tabela(); }
        let (_, dias) = ordenacao_ext::listas_prontas().into_iter().next().unwrap();
        let _ = app.acao_tabela(Acao::OrdenarPor(CriterioOrdem::Lista(dias), true));
        assert_eq!(coluna(&app, 4), ["dom", "seg", "qua", "sáb"]);
        // Lista própria digitada.
        let _ = app.acao_tabela(Acao::FiltroCabecalho(0, 0, 120.0, 20.0));
        let _ = app.acao_tabela(Acao::ListaTexto("sáb; qua".into()));
        let itens = ordenacao_ext::lista_de_texto(&app.tabela_filtro_extra.lista_texto);
        let _ = app.acao_tabela(Acao::OrdenarPor(CriterioOrdem::Lista(itens), true));
        assert_eq!(coluna(&app, 4)[..2], ["sáb", "qua"]);
        let _ = app.update(Msg::Desfazer);
        assert_eq!(coluna(&app, 4), ["dom", "seg", "qua", "sáb"]);
    }

    #[test]
    fn captura_abre_cada_submenu() {
        for (modo, pagina) in [("cor", PAG_COR), ("ordcor", PAG_ORD_COR), ("lista", PAG_ORD_LISTA),
            ("numero", PAG_NUMERO), ("top10", PAG_TOP10), ("data", PAG_DATA), ("periodo", PAG_PERIODO)] {
            let mut app = App::default();
            app.preparar_captura_filtro(modo);
            assert!(app.tabela_popup_filtro, "{modo}: {}", app.status);
            assert_eq!(app.tabela_filtro_pagina, pagina, "{modo}");
            let _m: Element<'_, Msg> = app.menu_filtro_tabela();
            let _s: Element<'_, Msg> = app.menu_condicoes_tabela();
        }
    }

    #[test]
    fn hoje_e_um_serial_plausivel() {
        // 01/01/2024 = 45292; o relógio do teste está depois disso.
        assert!(serial_de_hoje() > 45_292);
    }

    #[test]
    fn cor_de_hex_aceita_curta_e_longa() {
        assert_eq!(cor_de_hex("#ff0000"), Some(Color::rgb8(255, 0, 0)));
        assert_eq!(cor_de_hex("0f0"), Some(Color::rgb8(0, 255, 0)));
        assert_eq!(cor_de_hex("xyz"), None);
    }
}
