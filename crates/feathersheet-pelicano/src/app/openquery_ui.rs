//! Editor nativo OpenQuery. O motor de transformação fica no feathersheet-core.
//! Não copia a grade inteira: só materializa o intervalo escolhido, com limite
//! explícito antes de ler qualquer célula. Nunca carrega automaticamente fontes
//! adicionais nem reinterpreta valores como fórmulas ao escrever o resultado.
use super::*;
use feathersheet_core::{
    consulta::query::{executar_query_limitado, Agregado, Agregacao, ChaveOrdenacao, IndiceColuna, Passo, Tabela},
    Value,
};
use feather_ui::formatacao::dica;
use pelicano::opaque; use pelicano::{text_editor, Content};
use pelicano::Background; use pelicano::Border; use pelicano::Shadow;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
static IMPORTACAO_SEQ: AtomicU64 = AtomicU64::new(1);
mod json_passos;
mod registro;
mod execucao;
mod assistentes;
mod importador;
mod vinculo;
mod manual;
mod janela;
mod script;
mod qualidade;
#[cfg(test)]
mod testes;

const MAX_CELULAS_FONTE: u64 = 160_000;
const MAX_CELULAS_SAIDA: u64 = 240_000;
const PREVIA_LINHAS: usize = 24;
const PREVIA_COLUNAS: usize = 7;

// Escala visual do OpenQuery: janela e assistentes usam só estes valores.
const TAM_TITULO: f32 = 17.0;
const TAM_SECAO: f32 = 13.0;
const TAM_TEXTO: f32 = 12.0;
const TAM_LEGENDA: f32 = 11.0;
const ESPACO_PEQUENO: f32 = 4.0;
const ESPACO_MEDIO: f32 = 8.0;
const ESPACO_GRANDE: f32 = 12.0;
const PADDING_PAINEL: f32 = 12.0;

/// Abas da faixa de opções do OpenQuery (mesma divisão do Power Query).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbaFaixa { Inicio, Transformar, Adicionar, Exibir }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestinoModo { NovaAba, AbaAtual, ApenasConexao }

#[derive(Debug, Clone)]
pub enum Evento {
    Abrir,
    Fechar,
    Nome(String),
    Destino(DestinoModo),
    Coluna(usize),
    ColunaNumero(String),
    Filtro(String),
    Renomear(String),
    Quantidade(String),
    Separador(String),
    Adicionar(&'static str),
    Remover(usize),
    Mover(usize, i8),
    Prever(Option<usize>),
    Codigo,
    ExibirCodigo,
    AbrirEditorAvancado,
    EditarEditorAvancado,
    MostrarScriptAvancado,
    VoltarEditorAvancado,
    AplicarEditorAvancado,
    FecharEditorAvancado,
    Atualizar,
    AtualizarFonte,
    Assistente(assistentes::Evento),
    ImportarArquivo,
    MostrarWeb,
    UrlWeb(String),
    ImportarWeb,
    /// Uma resposta HTML pode conter diversas tabelas.
    TabelasWebLidas(u64, Result<(Vec<Tabela>, String), String>),
    SelecionarTabelaWeb(usize),
    ConfirmarTabelaWeb,
    CancelarSelecaoWeb,
    MostrarTabelas,
    CarregarTabela(usize, String),
    FonteTabela(usize, String),
    ReimportarArquivo,
    MostrarManual,
    EditarManual,
    AplicarManual,
    ArquivoEscolhido(u64, Option<std::path::PathBuf>),
    ArquivoLido(u64, Result<(Tabela, String), String>),
    EditarSalva(String),
    AtualizarSalva(String),
    PedirExclusao(String),
    ConfirmarExclusao,
    CancelarExclusao,
    Carregar,
    AlternarParametros,
    AbrirDestino,
    AplicarParametros,
    AlternarPainelInferior,
    Aba(AbaFaixa),
    AlternarConsultas,
    AlternarEtapas,
    /// Aba central: 0 prévia, 1 JavaScript declarativo, 2 tabela manual.
    AbaCentro(u8),
    /// Aba do painel inferior: 0 erros, 1 colunas, 2 log.
    PainelInferior(u8),
}

pub(super) struct Estado {
    fonte: Tabela,
    fontes: HashMap<String, Tabela>,
    origem_aba: String,
    origem: Rect,
    nome: String,
    destino_modo: DestinoModo,
    coluna: usize,
    coluna_numero: String,
    filtro: String,
    renomear: String,
    quantidade: String,
    separador: String,
    url_web: String,
    /// Fontes extraídas de uma única requisição, aguardando seleção explícita.
    tabelas_web: Vec<Tabela>,
    origem_web_pendente: String,
    indice_tabela_web: usize,
    mostrar_web: bool,
    mostrar_tabelas: bool,
    passos: Vec<Passo>,
    resultado: Tabela,
    codigo: Content,
    /// Rascunho isolado: cancelar não altera as etapas nem a prévia.
    editor_avancado: Option<Content>,
    /// Script expandido para consulta, somente leitura (não executado).
    script_avancado: Option<Content>,
    erro_avancado: String,
    exibir_codigo: bool,
    exibir_script: bool,
    erro: String,
    edicao: Option<String>,
    salvas: Vec<(String, String)>,
    exclusao_pendente: Option<String>,
    assistente: Option<assistentes::Estado>,
    passo_previa: Option<usize>,
    tabela_previa: Option<Tabela>,
    importacao_pendente: Option<u64>,
    importacao_preservar: bool,
    entrada_manual: Content,
    mostrar_manual: bool,
    aba_faixa: AbaFaixa,
    consultas_recolhidas: bool,
    etapas_recolhidas: bool,
    painel_inferior: u8,
    painel_inferior_aberto: bool,
    parametros_abertos: bool,
    acao_parametros: Option<&'static str>,
}

/// Apenas tabelas estruturadas já criadas (Ctrl+T) na pasta aberta.
/// Não confundir tabelas com a área usada de uma aba nem com o intervalo selecionado.
fn tabelas_existentes(workbook: &Workbook) -> Vec<(usize, String, Rect)> {
    let mut tabelas = Vec::new();
    for indice in 0..workbook.len() {
        let Some(aba) = workbook.aba(indice) else { continue };
        let Some(metadados) = aba.sheet.tb.as_deref() else { continue };
        let Ok(lista) = serde_json::from_str::<Vec<serde_json::Value>>(metadados) else { continue };
        for item in lista {
            if item["soFiltro"] == true { continue; }
            let Some(nome) = item["nome"].as_str() else { continue };
            let campo = &item["area"];
            let ponto = |chave: &str| campo[chave].as_u64().and_then(|n| u32::try_from(n).ok());
            let (Some(r0), Some(c0), Some(r1), Some(c1)) =
                (ponto("r0"), ponto("c0"), ponto("r1"), ponto("c1")) else { continue };
            if r0 > r1 || c0 > c1 || r1 >= MAX_ROWS || c1 >= MAX_COLS { continue; }
            tabelas.push((indice, nome.to_owned(), Rect::new(r0,c0,r1,c1)));
        }
    }
    tabelas
}

/// O recorte pela área usada, a leitura de valores calculados e o descarte
/// de linhas vazias pertencem ao motor Rust (reutilizável via WASM).
fn ler_tabela(wb: &Workbook, aba: usize, area: Rect) -> Result<Tabela, String> {
    feathersheet_core::consulta::sheet_query::ler_intervalo(
        wb, aba, area, true, MAX_CELULAS_FONTE,
    )
}

/// Fixture de compatibilidade: testes anteriores abriam a seleção implicitamente.
/// A UI real não faz isso; os testes devem escolher a origem explicitamente.
#[cfg(test)]
fn abrir_aba_para_teste(app: &mut App) {
    app.evento_openquery(Evento::Abrir);
    let aba = app.workbook.aba(app.folha).expect("aba de teste").sheet.name.clone();
    let area = app.workbook.aba(app.folha).unwrap().sheet.used_rect();
    let fonte = ler_tabela(&app.workbook, app.folha, area.clone()).expect("fonte de teste");
    let editor = app.openquery.as_mut().expect("editor aberto");
    editor.fonte = fonte;
    editor.origem_aba = aba;
    editor.origem = area;
    editor.registrar_passos();
}

impl Estado {
    fn novo(fonte: Tabela, origem_aba: String, origem: Rect) -> Self {
        let resultado = fonte.clone();
        Self {
            fonte, fontes: HashMap::new(), origem_aba, origem,
            nome: tr!("Consulta").into(), destino_modo: DestinoModo::NovaAba,
            coluna: 0, coluna_numero: "1".into(),
            filtro: String::new(),
            renomear: String::new(), quantidade: "100".into(),
            separador: ";".into(),
            url_web: String::new(), tabelas_web: Vec::new(),
            origem_web_pendente: String::new(), indice_tabela_web: 0,
            mostrar_web: false, mostrar_tabelas: false,
            passos: Vec::new(), resultado,
            codigo: Content::with_text("const etapas = [];"),
            editor_avancado: None, script_avancado: None, erro_avancado: String::new(),
            exibir_codigo: true, exibir_script: false, erro: String::new(),
            edicao: None, salvas: Vec::new(), exclusao_pendente: None,
            assistente: None,
            passo_previa: None, tabela_previa: None,
            importacao_pendente: None, importacao_preservar: false,
            entrada_manual: Content::with_text("Coluna1;Coluna2\nValor;10"),
            mostrar_manual: false,
            aba_faixa: AbaFaixa::Inicio,
            consultas_recolhidas: false,
            etapas_recolhidas: false,
            painel_inferior: 0,
            painel_inferior_aberto: false,
            parametros_abertos: false,
            acao_parametros: None,
        }
    }

    /// Descarta a prévia HTML sem tocar na fonte ou nas etapas já aplicadas.
    /// Concentra a liberação de memória quando a origem é alterada.
    fn limpar_previa_web(&mut self) {
        self.tabelas_web.clear();
        self.origem_web_pendente.clear();
        self.indice_tabela_web = 0;
    }

    fn sincronizar(&mut self) -> Result<(), String> {
        let passos = json_passos::ler_codigo_js(&self.codigo.text())?;
        self.passos = passos;
        Ok(())
    }

    /// Só publica a edição avançada quando parser E motor aceitam tudo.
    /// Assim, Cancelar/erro não destroem a consulta anterior.
    fn aplicar_editor_avancado(&mut self) -> Result<(), String> {
        let texto = self.editor_avancado.as_ref()
            .ok_or(tr!("Editor avançado não está aberto"))?.text();
        let passos = json_passos::ler_codigo_js(&texto)?;
        let tabela = executar_query_limitado(
            self.fonte.clone(), &passos, &self.fontes, MAX_CELULAS_SAIDA,
        )?;
        self.codigo = Content::with_text(&texto);
        self.passos = passos;
        self.coluna = self.coluna.min(tabela.cabecalho.len().saturating_sub(1));
        self.coluna_numero = (self.coluna + 1).to_string();
        self.resultado = tabela;
        self.passo_previa = None;
        self.tabela_previa = None;
        self.exibir_codigo = true;
        self.exibir_script = false;
        self.mostrar_manual = false;
        self.editor_avancado = None;
        self.script_avancado = None;
        self.erro_avancado.clear();
        self.erro.clear();
        Ok(())
    }

    fn recalcular(&mut self) {
        // Recusa explosões de cardinalidade ANTES de alocar o resultado.
        let tabela = match executar_query_limitado(
            self.fonte.clone(), &self.passos, &self.fontes, MAX_CELULAS_SAIDA
        ) {
            Ok(tabela) => tabela,
            Err(erro) => { self.erro = erro; return; }
        };
        self.coluna = self.coluna.min(tabela.cabecalho.len().saturating_sub(1));
        self.coluna_numero = (self.coluna + 1).to_string();
        self.resultado = tabela;
        self.passo_previa = None;
        self.tabela_previa = None;
        self.erro.clear();
    }

    fn tabela_visual(&self) -> &Tabela {
        self.tabela_previa.as_ref().unwrap_or(&self.resultado)
    }

    fn prever(&mut self, etapa: Option<usize>) {
        if let Err(e) = self.sincronizar() {
            self.erro = e;
            return;
        }
        if let Some(i) = etapa {
            if i >= self.passos.len() { return; }
            match executar_query_limitado(
                self.fonte.clone(), &self.passos[..=i], &self.fontes, MAX_CELULAS_SAIDA,
            ) {
                Ok(t) => {
                    self.tabela_previa = Some(t);
                    self.passo_previa = Some(i);
                    self.erro.clear();
                }
                Err(e) => self.erro = e,
            }
        } else {
            self.tabela_previa = None;
            self.passo_previa = None;
            self.erro.clear();
        }
    }

    fn registrar_passos(&mut self) {
        match json_passos::escrever_codigo_js(&self.passos) {
            Ok(codigo) => {
                self.codigo = Content::with_text(&codigo);
                self.recalcular();
            }
            Err(e) => self.erro = e.to_string(),
        }
    }

    fn adicionar(&mut self, tipo: &str) {
        if let Err(e) = self.sincronizar() {
            self.erro = e;
            self.exibir_codigo = true;
            return;
        }
        let col = self.coluna;
        let total = self.resultado.cabecalho.len();
        if col >= total && !matches!(tipo, "indice" | "limite" | "distintas" |
            "promover" | "rebaixar" | "acrescentar") {
            self.erro = tr!("Selecione uma coluna na prévia").into();
            return;
        }
        let passo = match tipo {
            "filtrar" => Passo::Filtrar { col, criterio: Value::Text(self.filtro.clone()) },
            "ordenar" => Passo::Ordenar { por: vec![ChaveOrdenacao { col, desc: Some(false) }] },
            "decrescente" => Passo::Ordenar { por: vec![ChaveOrdenacao { col, desc: Some(true) }] },
            "texto" => Passo::Tipar { col, para: "texto".into() },
            "numero" => Passo::Tipar { col, para: "numero".into() },
            "renomear" => {
                if self.renomear.trim().is_empty() {
                    self.erro = tr!("Informe o novo nome da coluna").into();
                    return;
                }
                Passo::Renomear { col, nome: self.renomear.trim().into() }
            }
            "remover_coluna" => Passo::Colunas {
                manter: (0..total).filter(|i| *i != col).collect(),
            },
            "distintas" => Passo::Distintas { cols: None },
            "limite" => {
                let Ok(n) = self.quantidade.trim().parse::<f64>() else {
                    self.erro = tr!("Quantidade de linhas inválida").into();
                    return;
                };
                if !n.is_finite() || n < 0.0 {
                    self.erro = tr!("Informe um limite não negativo e finito").into();
                    return;
                }
                Passo::Limite { quantas: n }
            }
            "indice" => Passo::Indice { inicio: Some(1.0), incremento: Some(1.0), nome: None },
            "promover" => Passo::PromoverCabecalho,
            "rebaixar" => Passo::RebaixarCabecalho,
            "remover_erros" => Passo::RemoverErros { col },
            "dividir" => {
                if self.separador.is_empty() {
                    self.erro = tr!("Informe um separador antes de dividir").into();
                    return;
                }
                Passo::Dividir { col, separador: self.separador.clone() }
            }
            "mesclar" => {
                if col + 1 >= total {
                    self.erro = tr!("Mesclar exige uma coluna à direita da selecionada").into();
                    return;
                }
                Passo::Mesclar {
                    cols: vec![col, col + 1],
                    separador: self.separador.clone(),
                    nome: Some("Mesclado".into()),
                }
            }
            "despivotar" => Passo::Despivotar {
                manter: vec![col], nome_atributo: None, nome_valor: None,
            },
            "agrupar" => Passo::Agrupar {
                por: vec![col],
                agregar: vec![Agregado {
                    col, como: Agregacao::ContarValores, nome: Some("Quantidade".into()),
                }],
            },
            "juntar" | "acrescentar" => {
                let Some((nome, outra)) = self.fontes.iter().next() else {
                    self.erro = tr!("Selecione uma fonte adicional na lateral antes de combinar").into();
                    return;
                };
                if tipo == "juntar" {
                    if col >= outra.cabecalho.len() {
                        self.erro = tr!("A segunda fonte não tem a coluna-chave escolhida").into();
                        return;
                    }
                    Passo::Juntar {
                        fonte: nome.clone(),
                        esquerda: IndiceColuna::Single(col),
                        direita: IndiceColuna::Single(col),
                        modo: Some("esquerda".into()),
                    }
                } else {
                    Passo::Acrescentar { fonte: nome.clone() }
                }
            }
            _ => return,
        };
        self.passos.push(passo);
        self.registrar_passos();
    }
}

impl App {
    /// Roteia o evento e mantém a janela própria do OpenQuery em sincronia
    /// com o estado: abre quando a consulta nasce, fecha quando termina.
    pub(super) fn evento_openquery_janela(&mut self, evento: Evento) -> Task<Msg> {
        let reabrir = matches!(evento, Evento::Abrir);
        let mut tarefa = self.evento_openquery(evento);
        let aberta = self.janelas.aberta(Auxiliar::OpenQuery);
        if self.openquery.is_some() && !aberta {
            tarefa = Task::batch([tarefa, self.alternar(Auxiliar::OpenQuery)]);
        } else if self.openquery.is_some() && reabrir {
            if let Some(id) = self.janelas.id(Auxiliar::OpenQuery) {
                tarefa = Task::batch([tarefa, Task::window_of(id, pelicano::WindowCmd::Focus)]);
            }
        } else if self.openquery.is_none() && aberta {
            tarefa = Task::batch([tarefa, self.janelas.fechar_tipo(Auxiliar::OpenQuery)]);
        }
        tarefa
    }

    pub(super) fn evento_openquery(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Abrir => {
                self.confirmar();
                let consultas = match registro::listar(&self.workbook) {
                    Ok(consultas) => consultas,
                    Err(erro) => { self.status = erro; return Task::none(); }
                };
                // O OpenQuery não importa a seleção/aba corrente por padrão.
                // A pessoa escolhe explicitamente Arquivo, Web ou Tabela existente.
                let vazio = Tabela { cabecalho: Vec::new(), linhas: Vec::new() };
                let mut editor = Estado::novo(vazio, String::new(), Rect::new(0, 0, 0, 0));
                editor.exibir_codigo = false;
                editor.salvas = consultas.into_iter().filter_map(|item| Some((
                    item.dados["id"].as_str()?.to_string(),
                    item.dados["nome"].as_str()?.to_string(),
                ))).collect();
                self.openquery = Some(editor);
                self.menu_aberto = false;
                self.contexto = None;
                self.status = tr!("OpenQuery: escolha um arquivo, uma URL ou uma tabela existente").into();
            }
            Evento::Fechar => self.openquery = None,
            Evento::Nome(v) => if let Some(e) = &mut self.openquery { e.nome = v; },
            Evento::MostrarWeb => if let Some(e) = &mut self.openquery {
                e.mostrar_web = true;
                e.mostrar_tabelas = false;
                e.mostrar_manual = false;
            },
            Evento::MostrarTabelas => if let Some(e) = &mut self.openquery {
                e.mostrar_tabelas = true;
                e.mostrar_web = false;
                e.mostrar_manual = false;
                // Escolher uma tabela interna invalida qualquer download Web.
                e.limpar_previa_web();
                e.importacao_pendente = None;
                e.importacao_preservar = false;
            },
            Evento::UrlWeb(v) => if let Some(e) = &mut self.openquery {
                if e.url_web != v {
                    // Uma URL editada torna qualquer resposta anterior obsoleta.
                    e.limpar_previa_web();
                    e.importacao_pendente = None;
                }
                e.url_web = v;
            },
            Evento::CarregarTabela(indice, nome) => {
                let encontrada = tabelas_existentes(&self.workbook).into_iter()
                    .find(|(i, n, _)| *i == indice && n == &nome);
                if let Some((i, _, area)) = encontrada {
                    if let Some(aba) = self.workbook.aba(i) {
                        let origem_aba = aba.sheet.name.clone();
                        match ler_tabela(&self.workbook, i, area.clone()) {
                            Ok(fonte) => if let Some(e) = &mut self.openquery {
                                e.fonte = fonte;
                                e.origem_aba = origem_aba;
                                e.origem = area;
                                e.passos.clear();
                                e.fontes.clear();
                                e.edicao = None;
                                e.nome = format!("Consulta_{nome}");
                                e.exibir_codigo = true;
                                e.exibir_script = false;
                                e.mostrar_tabelas = false;
                                e.mostrar_web = false;
                                e.mostrar_manual = false;
                                e.assistente = None;
                                e.registrar_passos();
                                self.status = trf!("OpenQuery: tabela '{nome}' carregada");
                            },
                            Err(erro) => if let Some(e) = &mut self.openquery { e.erro = erro; }
                        }
                    }
                } else if let Some(e) = &mut self.openquery {
                    e.erro = trf!("A tabela '{nome}' não existe mais nesta pasta");
                }
            }
            Evento::FonteTabela(indice, nome) => {
                let encontrada = tabelas_existentes(&self.workbook).into_iter()
                    .find(|(i, n, _)| *i == indice && n == &nome);
                if let Some((i, _, area)) = encontrada {
                    match ler_tabela(&self.workbook, i, area) {
                        Ok(fonte) => if let Some(e) = &mut self.openquery {
                            let chave = trf!("Tabela: {nome}");
                            if !e.fontes.contains_key(&chave) && e.fontes.len() >= 4 {
                                e.erro = tr!("Limite de quatro fontes adicionais").into();
                            } else {
                                e.fontes.insert(chave, fonte);
                                e.mostrar_tabelas = false;
                                e.recalcular();
                            }
                        },
                        Err(erro) => if let Some(e) = &mut self.openquery { e.erro = erro; }
                    }
                }
            },
            Evento::ImportarWeb => {
                let Some(e) = &mut self.openquery else { return Task::none() };
                if !matches!(reqwest::Url::parse(e.url_web.trim()), Ok(ref url)
                    if matches!(url.scheme(), "http" | "https"))
                {
                    e.erro = tr!("Informe uma URL http:// ou https:// válida").into();
                    return Task::none();
                }
                let url = e.url_web.clone();
                let ticket = IMPORTACAO_SEQ.fetch_add(1, Ordering::Relaxed);
                e.limpar_previa_web();
                e.importacao_pendente = Some(ticket);
                // reqwest normaliza URLs (ex.: https://site => https://site/).
                // Comparar strings destruiria as etapas numa atualização normal.
                let mesma_url = e.origem_aba.strip_prefix("Web: ")
                    .and_then(|antiga| reqwest::Url::parse(antiga).ok())
                    .zip(reqwest::Url::parse(url.trim()).ok())
                    .is_some_and(|(antiga, nova)| antiga == nova);
                e.importacao_preservar = mesma_url && !e.fonte.cabecalho.is_empty();
                self.status = tr!("OpenQuery: baixando dados Web em tarefa separada").into();
                return Task::perform(async move {
                    tokio::task::spawn_blocking(move || importador::ler_web(&url))
                        .await.unwrap_or_else(|erro| Err(trf!("Falha no download Web: {erro}")))
                }, move |resultado| Msg::OpenQuery(Evento::TabelasWebLidas(ticket, resultado)));
            },
            Evento::TabelasWebLidas(ticket, resultado) => {
                if self.openquery.as_ref().and_then(|e| e.importacao_pendente) != Some(ticket) {
                    return Task::none();
                }
                match resultado {
                    Err(erro) => return self.evento_openquery(Evento::ArquivoLido(ticket, Err(erro))),
                    Ok((mut tabelas, origem)) if tabelas.len() == 1 => {
                        return self.evento_openquery(Evento::ArquivoLido(
                            ticket, Ok((tabelas.remove(0), origem)),
                        ));
                    }
                    Ok((tabelas, origem)) if !tabelas.is_empty() => {
                        if let Some(e) = &mut self.openquery {
                            e.tabelas_web = tabelas;
                            e.origem_web_pendente = origem;
                            e.indice_tabela_web = 0;
                            e.mostrar_web = true;
                            e.erro.clear();
                            self.status = trf!(
                                "OpenQuery: {} tabelas encontradas; selecione qual importar",
                                e.tabelas_web.len(),
                            );
                        }
                    }
                    Ok(_) => return self.evento_openquery(Evento::ArquivoLido(
                        ticket, Err(tr!("A página não contém tabelas para importar").into()),
                    )),
                }
            },
            Evento::SelecionarTabelaWeb(indice) => if let Some(e) = &mut self.openquery {
                if indice < e.tabelas_web.len() { e.indice_tabela_web = indice; }
            },
            Evento::ConfirmarTabelaWeb => {
                let Some(e) = &mut self.openquery else { return Task::none() };
                let Some(ticket) = e.importacao_pendente else { return Task::none() };
                let Some(selecionada) = e.tabelas_web.get(e.indice_tabela_web) else {
                    return Task::none();
                };
                if selecionada.linhas.is_empty() {
                    e.erro = tr!("A tabela HTML selecionada não contém registros").into();
                    return Task::none();
                }
                let indice = e.indice_tabela_web;
                // Move sem clonar a tabela (até 160 mil células).
                let fonte = e.tabelas_web.swap_remove(indice);
                let origem = std::mem::take(&mut e.origem_web_pendente);
                e.limpar_previa_web();
                return self.evento_openquery(Evento::ArquivoLido(ticket, Ok((fonte, origem))));
            },
            Evento::CancelarSelecaoWeb => if let Some(e) = &mut self.openquery {
                e.limpar_previa_web();
                e.importacao_pendente = None;
                e.importacao_preservar = false;
            },
            Evento::Destino(modo) => if let Some(e) = &mut self.openquery {
                e.destino_modo = modo;
                e.parametros_abertos = false;
            },
            Evento::Coluna(col) => if let Some(e) = &mut self.openquery {
                e.coluna = col;
                e.coluna_numero = (col + 1).to_string();
            },
            Evento::ColunaNumero(numero) => if let Some(e) = &mut self.openquery {
                if let Ok(indice) = numero.parse::<usize>() {
                    if indice > 0 && indice <= e.resultado.cabecalho.len() {
                        e.coluna = indice - 1;
                    }
                }
                e.coluna_numero = numero;
            },
            Evento::Filtro(v) => if let Some(e) = &mut self.openquery { e.filtro = v; },
            Evento::Renomear(v) => if let Some(e) = &mut self.openquery { e.renomear = v; },
            Evento::Quantidade(v) => if let Some(e) = &mut self.openquery { e.quantidade = v; },
            Evento::Separador(v) => if let Some(e) = &mut self.openquery { e.separador = v; },
            Evento::Adicionar(tipo) => if let Some(e) = &mut self.openquery {
                if matches!(tipo, "filtrar" | "renomear" | "limite" | "dividir") {
                    e.parametros_abertos = true;
                    e.acao_parametros = Some(tipo);
                } else {
                    e.adicionar(tipo);
                }
            },
            Evento::AbrirDestino => if let Some(e) = &mut self.openquery {
                e.acao_parametros = None;
                e.parametros_abertos = true;
            },
            Evento::AlternarParametros => if let Some(e) = &mut self.openquery {
                e.parametros_abertos = !e.parametros_abertos;
                if !e.parametros_abertos { e.acao_parametros = None; }
            },
            Evento::AplicarParametros => if let Some(e) = &mut self.openquery {
                if let Some(tipo) = e.acao_parametros.take() { e.adicionar(tipo); }
                e.parametros_abertos = false;
            },
            Evento::Remover(idx) => if let Some(e) = &mut self.openquery {
                match e.sincronizar() {
                    Ok(()) => if idx < e.passos.len() {
                        e.passos.remove(idx);
                        e.registrar_passos();
                    },
                    Err(erro) => { e.erro = erro; e.exibir_codigo = true; }
                }
            },
            Evento::Mover(idx, delta) => if let Some(e) = &mut self.openquery {
                match e.sincronizar() {
                    Ok(()) => {
                        let novo = (idx as isize).saturating_add(delta as isize);
                        if idx < e.passos.len() && novo >= 0 &&
                            (novo as usize) < e.passos.len() {
                            e.passos.swap(idx, novo as usize);
                            e.registrar_passos();
                        }
                    }
                    Err(erro) => { e.erro = erro; e.exibir_codigo = true; }
                }
            },
            Evento::Prever(etapa) => if let Some(e) = &mut self.openquery {
                e.prever(etapa);
            },
            // O próprio editor já mudou o conteúdo.
            Evento::Codigo => {},
            Evento::ExibirCodigo => if let Some(e) = &mut self.openquery {
                e.exibir_codigo = !e.exibir_codigo;
            },
            Evento::AbrirEditorAvancado => if let Some(e) = &mut self.openquery {
                e.editor_avancado = Some(Content::with_text(&e.codigo.text()));
                e.script_avancado = None;
                e.erro_avancado.clear();
            },
            Evento::EditarEditorAvancado => if let Some(e) = &mut self.openquery {
                if e.editor_avancado.is_some() {
                    e.erro_avancado.clear();
                }
            },
            Evento::MostrarScriptAvancado => if let Some(e) = &mut self.openquery {
                if let Some(editor) = e.editor_avancado.as_ref() {
                    let gerado = json_passos::ler_codigo_js(&editor.text()).and_then(|passos| {
                        script::gerar(&e.nome, &e.fonte.cabecalho, &passos)
                    });
                    match gerado {
                        Ok(codigo) => {
                            e.script_avancado = Some(Content::with_text(&codigo));
                            e.erro_avancado.clear();
                        },
                        Err(erro) => e.erro_avancado = erro,
                    }
                }
            },
            Evento::VoltarEditorAvancado => if let Some(e) = &mut self.openquery {
                e.script_avancado = None;
                e.erro_avancado.clear();
            },
            Evento::AplicarEditorAvancado => if let Some(e) = &mut self.openquery {
                if let Err(erro) = e.aplicar_editor_avancado() {
                    e.erro_avancado = erro;
                    e.script_avancado = None;
                }
            },
            Evento::FecharEditorAvancado => if let Some(e) = &mut self.openquery {
                e.editor_avancado = None;
                e.script_avancado = None;
                e.erro_avancado.clear();
            },
            Evento::Atualizar => if let Some(e) = &mut self.openquery {
                match e.sincronizar() {
                    Ok(()) => e.recalcular(),
                    Err(erro) => e.erro = erro,
                }
            },
            Evento::AtualizarFonte => {
                let origem = self.openquery.as_ref()
                    .map(|e| e.origem_aba.clone()).unwrap_or_default();
                if origem.starts_with("Web: ") {
                    return self.evento_openquery(Evento::ImportarWeb);
                }
                if origem.starts_with("Arquivo: ") {
                    return self.evento_openquery(Evento::ReimportarArquivo);
                }
                if origem == "Entrada manual" || origem == tr!("Consulta salva") || origem == "Snapshot TypeScript" {
                    return self.evento_openquery(Evento::Atualizar);
                }
                if origem.is_empty() {
                    if let Some(e) = &mut self.openquery {
                        e.erro = tr!("Selecione primeiro uma fonte de dados").into();
                    }
                    return Task::none();
                }
                // Tabela estruturada interna: relê apenas o intervalo associado.
                let Some(indice) = self.workbook.indice_por_nome(&origem) else {
                    if let Some(e) = &mut self.openquery {
                        e.erro = tr!("A aba da tabela de origem não existe mais").into();
                    }
                    return Task::none();
                };
                let area = self.openquery.as_ref().map(|e| e.origem.clone())
                    .unwrap_or_else(|| Rect::new(0, 0, 0, 0));
                let resultado = ler_tabela(&self.workbook, indice, area);
                if let Some(e) = &mut self.openquery {
                    match resultado {
                        Ok(tabela) => {
                            e.fonte = tabela;
                            match e.sincronizar() {
                                Ok(()) => e.recalcular(),
                                Err(erro) => e.erro = erro,
                            }
                        },
                        Err(erro) => e.erro = erro,
                    }
                }
            },
            Evento::Assistente(evento) => self.evento_assistente_openquery(evento),
            Evento::MostrarManual => if let Some(e) = &mut self.openquery {
                e.mostrar_manual = !e.mostrar_manual;
            },
            Evento::EditarManual => {},
            Evento::AplicarManual => if let Some(e) = &mut self.openquery {
                match manual::ler(&e.entrada_manual.text()) {
                    Ok(fonte) => {
                        let linhas = fonte.linhas.len();
                        let colunas = fonte.cabecalho.len();
                        e.fonte = fonte;
                        e.origem_aba = tr!("Entrada manual").into();
                        e.origem = Rect::new(0, 0, linhas as u32, colunas.saturating_sub(1) as u32);
                        e.edicao = None;
                        e.assistente = None;
                        e.passos.clear();
                        e.fontes.clear();
                        e.registrar_passos();
                        e.mostrar_manual = false;
                        self.status = trf!("OpenQuery: tabela manual importada ({linhas} linhas)");
                    }
                    Err(erro) => e.erro = erro,
                }
            },
            Evento::ImportarArquivo | Evento::ReimportarArquivo => {
                let Some(editor) = &mut self.openquery else { return Task::none() };
                editor.limpar_previa_web();
                editor.importacao_preservar = matches!(evento, Evento::ReimportarArquivo);
                let ticket = IMPORTACAO_SEQ.fetch_add(1, Ordering::Relaxed);
                editor.importacao_pendente = Some(ticket);
                return Task::perform(async {
                    rfd::AsyncFileDialog::new()
                        .add_filter(tr!("Fontes OpenQuery"), &["csv","tsv","txt","json","xlsx"])
                        .pick_file().await.map(|arquivo| arquivo.path().to_path_buf())
                }, move |arquivo| Msg::OpenQuery(Evento::ArquivoEscolhido(ticket, arquivo)));
            }
            Evento::ArquivoEscolhido(ticket, arquivo) => {
                if self.openquery.as_ref().and_then(|e| e.importacao_pendente) != Some(ticket) {
                    return Task::none();
                }
                let Some(caminho) = arquivo else {
                    if let Some(e) = &mut self.openquery { e.importacao_pendente = None; }
                    return Task::none();
                };
                self.status = tr!("OpenQuery: importando fonte em tarefa separada").into();
                return Task::perform(async move {
                    tokio::task::spawn_blocking(move || importador::ler(&caminho))
                        .await.unwrap_or_else(|e| Err(trf!("Falha no importador: {e}")))
                }, move |resultado| Msg::OpenQuery(Evento::ArquivoLido(ticket, resultado)));
            }
            Evento::ArquivoLido(ticket, resultado) => {
                let Some(editor) = &mut self.openquery else { return Task::none() };
                if editor.importacao_pendente != Some(ticket) { return Task::none(); }
                editor.importacao_pendente = None;
                match resultado {
                    Err(erro) => {
                        editor.erro = erro.clone();
                        self.status = trf!("OpenQuery: {erro}");
                    }
                    Ok((fonte, origem)) => {
                        let preservar = editor.importacao_preservar;
                        editor.importacao_preservar = false;
                        if preservar && !(editor.origem_aba.starts_with("Arquivo: ")
                            || editor.origem_aba.starts_with("Web: ")) {
                            editor.erro = tr!("Atualização da fonte exige um arquivo ou uma URL previamente importada").into();
                            return Task::none();
                        }
                        if preservar && fonte.cabecalho != editor.fonte.cabecalho {
                            editor.erro = tr!("Os cabeçalhos do arquivo mudaram. Para não invalidar as etapas, faça uma nova importação.").into();
                            return Task::none();
                        }
                        let linhas = fonte.linhas.len();
                        let colunas = fonte.cabecalho.len();
                        editor.limpar_previa_web();
                        editor.fonte = fonte;
                        editor.origem_aba = origem.clone();
                        if let Some(url) = origem.strip_prefix("Web: ") {
                            editor.url_web = url.to_owned();
                        }
                        editor.mostrar_web = false;
                        editor.mostrar_tabelas = false;
                        editor.mostrar_manual = false;
                        editor.origem = Rect::new(0, 0, linhas as u32, colunas.saturating_sub(1) as u32);
                        editor.assistente = None;
                        if !preservar { editor.exibir_codigo = true; editor.exibir_script = false;
                            editor.edicao = None;
                            editor.passos.clear();
                            editor.fontes.clear();
                        }
                        editor.registrar_passos();
                        self.status = if preservar {
                            trf!("OpenQuery: reimportadas {linhas} linhas; salve para atualizar o resultado")
                        } else {
                            trf!("OpenQuery: {origem} carregado ({linhas} linhas)")
                        };
                    }
                }
            },
            Evento::EditarSalva(id) => { self.abrir_consulta_salva(&id); },
            Evento::AtualizarSalva(id) => self.atualizar_consulta_salva(&id),
            Evento::PedirExclusao(id) => if let Some(e) = &mut self.openquery {
                e.exclusao_pendente = Some(id);
            },
            Evento::CancelarExclusao => if let Some(e) = &mut self.openquery {
                e.exclusao_pendente = None;
            },
            Evento::ConfirmarExclusao => {
                let id = self.openquery.as_mut().and_then(|e| e.exclusao_pendente.take());
                if let Some(id) = id {
                    // Validar vínculos antes de remover o registro.
                    let resultado = vinculo::preparar_desvinculacao(&self.workbook, &id)
                        .and_then(|plano| {
                            registro::excluir(&mut self.workbook, &id)?;
                            vinculo::aplicar_desvinculacao(&mut self.workbook, plano);
                            Ok(())
                        });
                    match resultado {
                        Ok(()) => {
                            if let Some(e) = &mut self.openquery {
                                e.salvas.retain(|(chave, _)| chave != &id);
                                if e.edicao.as_deref() == Some(id.as_str()) {
                                    e.edicao = None;
                                }
                            }
                            self.alterado = true;
                            self.status = tr!("Consulta desvinculada; dados da aba preservados").into();
                        }
                        Err(erro) => if let Some(e) = &mut self.openquery {
                            e.erro = erro;
                        },
                    }
                }
            },
            Evento::Aba(aba) => if let Some(e) = &mut self.openquery { e.aba_faixa = aba; },
            Evento::AlternarConsultas => if let Some(e) = &mut self.openquery {
                e.consultas_recolhidas = !e.consultas_recolhidas;
            },
            Evento::AlternarEtapas => if let Some(e) = &mut self.openquery {
                e.etapas_recolhidas = !e.etapas_recolhidas;
            },
            Evento::AbaCentro(aba) => if let Some(e) = &mut self.openquery {
                e.mostrar_manual = aba == 2;
                e.exibir_codigo = aba == 1;
                e.exibir_script = aba == 3;
                e.mostrar_web = false;
                e.mostrar_tabelas = false;
                e.limpar_previa_web();
                e.importacao_pendente = None;
                e.importacao_preservar = false;
            },
            Evento::PainelInferior(aba) => if let Some(e) = &mut self.openquery {
                let nova = aba.min(2);
                e.painel_inferior_aberto = e.painel_inferior != nova || !e.painel_inferior_aberto;
                e.painel_inferior = nova;
            },
            Evento::AlternarPainelInferior => if let Some(e) = &mut self.openquery {
                e.painel_inferior_aberto = !e.painel_inferior_aberto;
            },
            Evento::Carregar if self.openquery.as_ref()
                .is_some_and(|e| e.fonte.cabecalho.is_empty()) => {
                if let Some(e) = &mut self.openquery {
                    e.erro = tr!("Escolha uma fonte de dados antes de salvar a consulta").into();
                }
            },
            Evento::Carregar => if self.openquery.as_ref()
                .is_some_and(|e| e.destino_modo == DestinoModo::ApenasConexao) {
                self.salvar_conexao_openquery();
            } else { self.concluir_openquery(); },
        }
        Task::none()
    }
}

/// Mensagem do OpenQuery: abrevia a construção repetida em cada botão.
fn om(evento: Evento) -> Msg {
    Msg::OpenQuery(evento)
}

fn negrito() -> pelicano::Font {
    pelicano::Font { weight: 600, ..pelicano::Font::DEFAULT }
}

/// Vermelho de erro do OpenQuery (uma só definição).
fn cor_erro() -> pelicano::Color {
    pelicano::Color::rgb8(200, 68, 68)
}

/// Seta para cima: o catálogo não tem `arrow-up`, então o traço vem aqui,
/// no mesmo estilo 16 × 16 das setas do FeatherSheet.
fn icone_seta_cima(tamanho: f32) -> pelicano::SvgView {
    const DESENHO: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\" \
        viewBox=\"0 0 16 16\" fill=\"none\" stroke=\"#808080\" stroke-width=\"1.4\" \
        stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M4 10l4-4 4 4\"/></svg>";
    pelicano::svg(pelicano::Svg::from_bytes(DESENHO.as_bytes()).expect("svg fixo"))
        .width(tamanho)
        .height(tamanho)
        .color(COR_TEXTO())
}

/// Título de janela ou de painel grande (escala: 17 semibold).
fn titulo_oq(texto_titulo: impl Into<String>) -> Element<'static, Msg> {
    text(texto_titulo.into()).size(TAM_TITULO).font(negrito()).color(COR_TEXTO()).into()
}

/// Título de seção dentro de um painel (escala: 13 semibold, sem caixa alta).
fn secao_oq(texto_secao: impl Into<String>) -> Element<'static, Msg> {
    text(texto_secao.into()).size(TAM_SECAO).font(negrito()).color(COR_TEXTO()).into()
}

/// Texto corrido (escala: 12).
fn texto_oq(conteudo: impl Into<String>) -> Element<'static, Msg> {
    text(conteudo.into()).size(TAM_TEXTO).color(COR_TEXTO()).into()
}

/// Legenda e apoio (escala: 11, cor de apoio).
fn legenda_oq(conteudo: impl Into<String>) -> Element<'static, Msg> {
    text(conteudo.into()).size(TAM_LEGENDA).color(COR_TEXTO_MUTED()).into()
}

fn estilo_janela_oq(_tema: &pelicano::Theme) -> pelicano::Style {
    pelicano::Style {
        background: Some(Background::Color(COR_PAINEL())),
        border: Border { color: COR_BORDA(), width: 1.0, radius: [8.0; 4] },
        shadow: Some(Shadow {
            color: pelicano::Color::rgba(0.0, 0.0, 0.0, if escuro() { 0.5 } else { 0.25 }),
            offset: pelicano::Point::new(0.0, 8.0),
            blur: 24.0,
        }),
        ..pelicano::Style::default()
    }
}

fn passo_rotulo(p: &Passo) -> &'static str {
    match p {
        Passo::Filtrar { .. } => tr!("Filtrar"),
        Passo::Ordenar { .. } => tr!("Ordenar"),
        Passo::Colunas { .. } => tr!("Selecionar colunas"),
        Passo::Renomear { .. } => tr!("Renomear"),
        Passo::Tipar { .. } => tr!("Alterar tipo"),
        Passo::Substituir { .. } => tr!("Substituir"),
        Passo::SubstituirErros { .. } => tr!("Substituir erros"),
        Passo::RemoverErros { .. } => tr!("Remover erros"),
        Passo::Mesclar { .. } => tr!("Mesclar colunas"),
        Passo::Derivar { .. } => tr!("Data"),
        Passo::Dividir { .. } => tr!("Dividir em linhas"),
        Passo::Distintas { .. } => tr!("Valores distintos"),
        Passo::Agrupar { .. } => tr!("Agrupar"),
        Passo::Pivotar { .. } => tr!("Pivotar"),
        Passo::Juntar { .. } => tr!("Juntar"),
        Passo::Acrescentar { .. } => tr!("Acrescentar"),
        Passo::Despivotar { .. } => tr!("Despivotar"),
        Passo::PromoverCabecalho => tr!("Promover cabeçalho"),
        Passo::RebaixarCabecalho => tr!("Rebaixar cabeçalho"),
        Passo::Indice { .. } => tr!("Adicionar índice"),
        Passo::Condicional { .. } => tr!("Coluna condicional"),
        Passo::Limite { .. } => tr!("Manter linhas"),
    }
}
