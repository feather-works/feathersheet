//! Aba contextual "Tabela Dinâmica" da faixa (17/10/2026, pedido do usuário):
//! aparece com a célula ativa numa dinâmica (ou com o painel de campos
//! aberto) e leva as opções que ficavam em "Opções" no painel — layout,
//! subtotais, totais gerais, agrupar datas, cores, atualizar, origem,
//! recolher, gráfico e segmentação. Os campos calculados ficam no painel.
//!
//! Sem o painel aberto, cada botão carrega a dinâmica sob o cursor, aplica a
//! mudança e grava (o mesmo caminho do painel, um Ctrl+Z).
use super::*;
use super::edicao::{ler_estado, localizar_selecionada};
use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    /// As dinâmicas da aba (sem o mapa de linhas), por (id da aba, versão):
    /// a barra de abas pergunta a cada quadro, e o JSON com o mapa de uma
    /// dinâmica de 1 milhão de linhas tem megabytes.
    static CACHE: RefCell<Option<(u64, u32, Rc<Vec<serde_json::Value>>)>> = const { RefCell::new(None) };
}

/// As definições das dinâmicas da aba `folha`, sem o mapa.
fn dinamicas_da_aba(app: &App, folha: usize) -> Rc<Vec<serde_json::Value>> {
    let Some(aba) = app.workbook.aba(folha) else { return Rc::default() };
    let chave = (aba.id as u64, aba.versao as u32);
    if let Some(v) = CACHE.with(|c| c.borrow().as_ref().filter(|(i, v, _)| (*i, *v) == chave).map(|(_, _, l)| l.clone())) {
        return v;
    }
    let mut lista: Vec<serde_json::Value> = aba.sheet.dn.as_deref()
        .and_then(|t| serde_json::from_str(t).ok()).unwrap_or_default();
    for d in &mut lista {
        if let Some(o) = d.as_object_mut() { o.remove("mapa"); }
    }
    let lista = Rc::new(lista);
    CACHE.with(|c| *c.borrow_mut() = Some((chave.0, chave.1, lista.clone())));
    lista
}

/// A dinâmica (sem o mapa) cuja área de resultado contém a célula ativa.
fn meta_sob_cursor(app: &App) -> Option<serde_json::Value> {
    dinamicas_da_aba(app, app.folha).iter().find(|d| retangulo_json(&d["areaResultado"])
        .is_some_and(|r| r.contains(app.linha, app.coluna))).cloned()
}

/// O que os botões da aba mostram ligado/desligado.
pub(in crate::app) struct OpcoesFaixa {
    tabular: bool,
    repetir: bool,
    subtotais: bool,
    totais_linhas: bool,
    totais_colunas: bool,
    sozinha: bool,
    agrupar: Option<AgruparData>,
    tema: String,
    painel_aberto: bool,
}

fn rotulo_tema(tema: &str) -> &'static str {
    match tema {
        "azul" => tr!("Azul"),
        "verde" => tr!("Verde"),
        "laranja" => tr!("Laranja"),
        "vermelho" => tr!("Vermelho"),
        "roxo" => tr!("Roxo"),
        "cinza" => tr!("Cinza"),
        _ => tr!("Padrão"),
    }
}

impl App {
    /// A aba aparece: célula ativa numa dinâmica, ou o painel de campos aberto.
    pub(in crate::app) fn dinamica_na_faixa(&self) -> bool {
        self.dinamica.as_ref().is_some_and(|e| e.fase == Fase::Campos) || meta_sob_cursor(self).is_some()
    }

    /// O estado dos botões: do painel aberto ou, sem ele, do que está
    /// gravado na dinâmica sob o cursor (sem recalcular nada).
    fn opcoes_faixa_dinamica(&self) -> Option<OpcoesFaixa> {
        if let Some(e) = self.dinamica.as_ref().filter(|e| e.fase == Fase::Campos) {
            let x = avancado::extras_vigentes(e);
            return Some(OpcoesFaixa {
                tabular: e.layout == Layout::Tabular, repetir: e.repetir_rotulos, subtotais: e.subtotais,
                totais_linhas: x.totais_gerais_linhas, totais_colunas: x.totais_gerais_colunas,
                sozinha: e.atualizar_sozinha, agrupar: e.agrupar_data, tema: e.tema.clone(), painel_aberto: true,
            });
        }
        let meta = meta_sob_cursor(self)?;
        let extras: Extras = serde_json::from_value(meta["extras"].clone()).unwrap_or_default();
        let agrupar = meta["datas"].as_array().and_then(|d| d.first()).and_then(|d| d["parte"].as_str())
            .and_then(|p| match p { "mes" => Some(AgruparData::Mes), "trimestre" => Some(AgruparData::Trimestre),
                "ano" => Some(AgruparData::Ano), _ => None });
        Some(OpcoesFaixa {
            tabular: meta["layout"].as_str() == Some("tabular"),
            repetir: meta["repetirRotulos"].as_bool().unwrap_or(true),
            subtotais: meta["subtotais"].as_bool().unwrap_or(false),
            totais_linhas: extras.totais_gerais_linhas, totais_colunas: extras.totais_gerais_colunas,
            sozinha: meta["atualizarSozinha"].as_bool().unwrap_or(false),
            agrupar, tema: meta["tema"].as_str().unwrap_or("padrao").to_string(), painel_aberto: false,
        })
    }

    /// Um botão da aba: com o painel aberto vai direto a ele; sem o painel,
    /// carrega a dinâmica sob o cursor, aplica e grava.
    pub(super) fn evento_na_faixa(&mut self, evento: Evento) -> Task<Msg> {
        if self.dinamica.as_ref().is_some_and(|e| e.fase == Fase::Campos) {
            return self.evento_dinamica(evento);
        }
        let estado = localizar_selecionada(self).and_then(|(destino, _, meta)| ler_estado(self, destino, &meta));
        match estado {
            Ok(e) => {
                self.dinamica = Some(e);
                let tarefa = self.evento_dinamica(evento);
                self.dinamica = None;
                tarefa
            }
            Err(erro) => { self.status = erro; Task::none() }
        }
    }

    /// Os grupos da aba "Tabela Dinâmica".
    pub(in crate::app) fn grupos_faixa_dinamica(&self) -> Vec<Element<'_, Msg>> {
        let Some(o) = self.opcoes_faixa_dinamica() else { return Vec::new() };
        let na = |e: Evento| Some(Msg::Dinamica(Evento::NaFaixa(Box::new(e))));
        let direto = |e: Evento| Some(Msg::Dinamica(e));
        let mut grupos: Vec<Element<'_, Msg>> = Vec::new();
        grupos.push(self.faixa_grupo(tr!("Tabela Dinâmica"), self.faixa_linha(vec![
            self.faixa_grande(tr!("Campos"), "pivot-table",
                direto(if o.painel_aberto { Evento::Fechar } else { Evento::Editar }), o.painel_aberto),
            self.faixa_grande(tr!("Atualizar"), "refresh", direto(Evento::Atualizar), false),
            self.faixa_pilha(vec![
                self.faixa_pequeno(tr!("Atualizar sozinha"), if o.sozinha { "checkbox" } else { "square" },
                    na(Evento::AtualizarSozinha(!o.sozinha)), o.sozinha),
            ]),
        ])));
        grupos.push(self.faixa_grupo(tr!("Layout"), self.faixa_pilha(vec![
            self.faixa_linha(vec![
                self.faixa_pequeno(tr!("Compacto"), "layout-rows", na(Evento::LayoutEscolher(false)), !o.tabular),
                self.faixa_pequeno(tr!("Tabular"), "layout-columns", na(Evento::LayoutEscolher(true)), o.tabular),
            ]),
            self.faixa_pequeno(tr!("Repetir rótulos"), if o.repetir { "checkbox" } else { "square" },
                na(Evento::RepetirRotulos), o.repetir),
            self.faixa_pequeno(tr!("Subtotais"), if o.subtotais { "checkbox" } else { "square" },
                na(Evento::Subtotais), o.subtotais),
        ])));
        grupos.push(self.faixa_grupo(tr!("Totais gerais"), self.faixa_pilha(vec![
            self.faixa_pequeno(tr!("Das linhas"), if o.totais_linhas { "checkbox" } else { "square" },
                na(Evento::TotaisGerais { linhas: true, ligado: !o.totais_linhas }), o.totais_linhas),
            self.faixa_pequeno(tr!("Das colunas"), if o.totais_colunas { "checkbox" } else { "square" },
                na(Evento::TotaisGerais { linhas: false, ligado: !o.totais_colunas }), o.totais_colunas),
        ])));
        grupos.push(self.faixa_grupo(tr!("Agrupar e recolher"), self.faixa_pilha(vec![
            self.faixa_pequeno(&trf!("Datas: {}", rotulo_agrupamento(o.agrupar)), "layout-grid",
                na(Evento::AgruparEscolher(proximo_agrupamento(o.agrupar))), o.agrupar.is_some()),
            self.faixa_linha(vec![
                self.faixa_pequeno(tr!("Recolher tudo"), "minus", na(Evento::RecolherTodos(true)), false),
                self.faixa_pequeno(tr!("Expandir tudo"), "plus", na(Evento::RecolherTodos(false)), false),
            ]),
        ])));
        grupos.push(self.faixa_grupo(tr!("Estilo"), self.faixa_grande(
            &trf!("Cores: {}", rotulo_tema(&o.tema)), "palette", na(Evento::Tema), false)));
        grupos.push(self.faixa_grupo(tr!("Ferramentas"), self.faixa_linha(vec![
            self.faixa_grande(tr!("Gráfico dinâmico"), "chart-bar", direto(Evento::GraficoDinamico), false),
            self.faixa_grande(tr!("Segmentação"), "filter", direto(Evento::SegmentacaoInserir), false),
        ])));
        grupos
    }

    /// Funil do filtro de relatório na grade: abre o painel da dinâmica e a
    /// lista de itens daquele campo (o Excel abre a lista ali mesmo).
    pub(super) fn abrir_filtro_na_grade(&mut self, r: u32, c: u32) {
        let achada = dinamicas_da_aba(self, self.folha).iter().find_map(|d| {
            let af = retangulo_json(&d["areaFiltros"])?;
            let area = retangulo_json(&d["areaResultado"])?;
            af.contains(r, c).then_some(((r - af.r0) as usize, area))
        });
        let Some((indice, area)) = achada else { return };
        self.trocar_selecao(area.r0, area.c0);
        self.abrir_edicao_dinamica();
        let Some(e) = self.dinamica.as_ref() else { return };
        let Some(coluna) = e.filtros_de_relatorio().get(indice).map(|f| f.coluna) else { return };
        let i = coluna.saturating_sub(e.origem.c0 as usize);
        let _ = self.evento_dinamica(Evento::FiltroCampo(i));
    }
}
