//! Editor nativo FeatherQuery. O motor de transformação fica no feathersheet-core.
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

/// Formato de número de cada coluna da fonte (data, hora, %, moeda), pelo nome:
/// a prévia mostra "27/10/2026", não 46322.
pub(super) type Formatos = HashMap<String, feathersheet_core::celulas::style::Style>;

/// O formato da primeira linha de dados de cada coluna de `area` (cabeçalho na 1ª).
// shortcut: só o nome liga coluna e formato; renomear a coluna perde o formato da prévia.
pub(super) fn formatos_da_fonte(wb: &Workbook, aba: usize, area: &Rect, cabecalho: &[String]) -> Formatos {
    use feathersheet_core::celulas::style::NumberFormat;
    let Some(a) = wb.aba(aba) else { return Formatos::new() };
    let area = area.normalized();
    cabecalho.iter().enumerate().filter_map(|(i, nome)| {
        let estilo = a.sheet.get_style(area.r0 + 1, area.c0 + i as u32)?;
        (estilo.format != NumberFormat::Auto || estilo.code.is_some()).then(|| (nome.clone(), estilo.clone()))
    }).collect()
}
mod json_passos;
mod registro;
mod execucao;
mod assistentes;
mod importador;
mod vinculo;
mod manual;
mod janela;
mod script;
mod power_query;
mod qualidade;
pub(in crate::app) mod etapa;
#[cfg(test)]
mod testes;

/// Como o Power Query: as primeiras 1000 linhas (só a parte à vista é desenhada).
const PREVIA_LINHAS: usize = 1000;
/// Largura de cada coluna da prévia (rola para o lado, como no Power Query).
const LARGURA_COLUNA_PREVIA: f32 = 150.0;

// Escala visual do FeatherQuery: janela e assistentes usam só estes valores.
const TAM_TITULO: f32 = 17.0;
const TAM_SECAO: f32 = 13.0;
const TAM_TEXTO: f32 = 12.0;
const TAM_LEGENDA: f32 = 11.0;
const ESPACO_PEQUENO: f32 = 4.0;
const ESPACO_MEDIO: f32 = 8.0;
const ESPACO_GRANDE: f32 = 12.0;
const PADDING_PAINEL: f32 = 12.0;

/// Abas da faixa de opções do FeatherQuery (mesma divisão do Power Query).
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
    AbrirEditorAvancado,
    EditarEditorAvancado,
    AplicarEditorAvancado,
    FecharEditorAvancado,
    /// O M das etapas na área de transferência, para o Editor Avançado do Excel.
    CopiarPowerQuery,
    /// Obter dados ▸ Consultas do Power Query deste arquivo: lê e lista as do `.xlsx`.
    ConsultasPowerQuery,
    /// Abre a consulta `i` da lista no Editor avançado, já convertida.
    AbrirConsultaPowerQuery(usize),
    FecharConsultasPowerQuery,
    /// Lista de métodos do Editor avançado: escreve a linha no cursor.
    InserirNoEditor(String),
    Atualizar,
    AtualizarFonte,
    Assistente(assistentes::Evento),
    ImportarArquivo,
    MostrarWeb,
    UrlWeb(String),
    ImportarWeb,
    /// Uma resposta HTML pode conter diversas tabelas.
    TabelasWebLidas(u64, Result<(Vec<Tabela>, String), String>),
    /// Navegador: as planilhas e tabelas do arquivo escolhido.
    ItensLidos(u64, Vec<importador::Item>, String),
    /// A opção escolhida, lida inteira em segundo plano (origem já com " ▸ nome").
    ItemLido(u64, Result<(Tabela, Formatos), String>, String),
    SelecionarTabelaWeb(usize),
    ConfirmarTabelaWeb,
    CancelarSelecaoWeb,
    MostrarTabelas,
    /// Abre o FeatherQuery (se fechado) e segue com o evento: os botões da aba Dados.
    AbrirCom(Box<Evento>),
    /// De Tabela/Intervalo: a seleção (ou a região em volta da célula) vira a fonte.
    CarregarSelecao,
    /// Fontes recentes: reabre um arquivo ou uma URL já usados.
    AbrirRecente(String),
    CarregarTabela(usize, String),
    /// De tabela ▸ Planilhas desta pasta: a aba inteira (sem o título acima da tabela).
    CarregarAba(usize),
    /// Obter dados ▾: abre/fecha o menu das conexões.
    MenuObter(bool),
    /// Item do menu Obter dados: fecha o menu e faz o pedido.
    DoMenuObter(Box<Evento>),
    /// De arquivo só de um tipo (índice em `importador::TIPOS`).
    ImportarTipo(usize),
    /// Obter dados ▸ Outra consulta: referencia a consulta gravada `id`.
    ReferenciarSalva(String),
    FonteTabela(usize, String),
    ReimportarArquivo,
    MostrarManual,
    EditarManual,
    AplicarManual,
    ArquivoEscolhido(u64, Option<std::path::PathBuf>),
    /// Da pasta: junta os arquivos da pasta escolhida.
    ImportarPasta,
    PastaEscolhida(u64, Option<std::path::PathBuf>),
    ArquivoLido(u64, Result<(Tabela, String), String>),
    EditarSalva(String),
    AtualizarSalva(String),
    /// Liga/desliga "Atualizar ao abrir o arquivo" da consulta gravada.
    AoAbrir(String),
    PedirExclusao(String),
    ConfirmarExclusao,
    CancelarExclusao,
    /// Renomear consulta gravada (#93): abre o campo, digita, confirma ou cancela.
    PedirRenomear(String),
    TextoRenomear(String),
    ConfirmarRenomear,
    CancelarRenomear,
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
    /// Modal de parâmetros de uma etapa (nova ou pela engrenagem).
    Etapa(etapa::Evento),
    /// Ctrl+Z (true) / Ctrl+Y (false) nas etapas.
    Desfazer(bool),
    /// Aviso de tipo: 0 remover as linhas, 1 trocar por 0, 2 fechar o aviso.
    FalhasDeTipo(u8),
    /// Exibir ▸ Dependências (abre/fecha o mapa das consultas).
    Dependencias(bool),
    /// A consulta gravada aberta: copiar como conexão ou usar o resultado como fonte.
    Duplicar,
    Referenciar,
    /// O "senão" da coluna condicional no modal.
    EtapaSenao(String),
    /// Engrenagem: reabre a etapa no modal.
    EditarPasso(usize),
    /// ▾ do cabeçalho: menu da coluna (`None` fecha).
    MenuColuna(Option<usize>),
    /// Posição do mouse sobre o cabeçalho, para o menu abrir onde foi o clique.
    Ponteiro(f32, f32),
    Modificadores(pelicano::Modifiers),
    /// Recolhe/expande um grupo da lateral (0 salvas, 1 origem, 2 tabelas).
    RecolherGrupo(u8),
    JanelaRedimensionada(pelicano::Size),
    /// Menu de tipos (clique no 123/ABC ou "Tipo de dados ▾"): coluna e onde abre.
    MenuTipo(Option<usize>, Option<(f32, f32)>),
    /// Submenu aberto no menu ▾ da coluna (passar o mouse num item).
    Submenu(Option<usize>),
    /// Muda o tipo da coluna (troca a etapa de tipo anterior da mesma coluna, se for a última).
    Tipar(usize, &'static str),
    /// Formatos da fonte lida em segundo plano (vale só para a importação `ticket`).
    Formatos(u64, Formatos),
    /// Rolagem da prévia: só as linhas e colunas à vista são montadas.
    RolarPrevia(pelicano::Viewport),
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
    /// Navegador de arquivo: as opções (a prévia de cada uma vai em `tabelas_web`);
    /// vazio na Web, que mostra "Tabela 1, 2…".
    itens_arquivo: Vec<importador::Item>,
    /// Planilha que o script pediu (`fonte.planilha("x")`), usada na próxima releitura.
    item_pedido: Option<String>,
    /// O arquivo lido nesta sessão: trocar de planilha pelo script relê sem perguntar.
    caminho_arquivo: Option<std::path::PathBuf>,
    origem_web_pendente: String,
    indice_tabela_web: usize,
    mostrar_web: bool,
    mostrar_tabelas: bool,
    passos: Vec<Passo>,
    resultado: Tabela,
    /// O script gerado das etapas (o Editor avançado copia ao abrir).
    codigo: String,
    /// Rascunho isolado: cancelar não altera as etapas nem a prévia.
    editor_avancado: Option<Content>,
    erro_avancado: String,
    /// Recado neutro do Editor avançado (M convertido, copiado como Power Query).
    aviso_avancado: String,
    /// Consultas do Power Query do `.xlsx` aberto (nome, M); `Some` = a lista está na tela.
    consultas_pq: Option<Vec<(String, String)>>,
    erro: String,
    edicao: Option<String>,
    salvas: Vec<(String, String)>,
    exclusao_pendente: Option<String>,
    /// (id, nome digitado) da consulta gravada sendo renomeada.
    renomeando: Option<(String, String)>,
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
    modal: Option<etapa::Modal>,
    erro_modal: String,
    /// Colunas selecionadas (Ctrl/Shift+clique); `coluna` é a última clicada.
    selecionadas: Vec<usize>,
    modificadores: pelicano::Modifiers,
    menu_coluna: Option<usize>,
    menu_tipo: Option<usize>,
    submenu: Option<usize>,
    menu_obter: bool,
    /// O próximo "De arquivo" abre a caixa com esse tipo primeiro (`importador::TIPOS`).
    filtro_arquivo: Option<usize>,
    ponteiro: (f32, f32),
    /// Onde o menu ▾ abriu: fica parado enquanto o mouse anda até ele.
    posicao_menu: (f32, f32),
    janela: pelicano::Size,
    recolhidos: [bool; 3],
    /// Deslocamento e tamanho visível da prévia (x, y, largura, altura).
    rolagem: (f32, f32, f32, f32),
    formatos: Formatos,
    /// Ctrl+Z / Ctrl+Y das etapas: as listas de etapas anteriores (JSON) e a atual.
    historico: Vec<String>,
    refazer: Vec<String>,
    etapas_atuais: String,
    /// A última etapa mudou o tipo e alguns valores não converteram.
    aviso_tipo: Option<AvisoTipo>,
    /// Exibir ▸ Dependências aberto: (nome, fontes, destino) de cada consulta gravada.
    dependencias: Option<Vec<(String, Vec<String>, String)>>,
}

/// "N linhas de Coluna não converteram para número (ex.: …)".
#[derive(Debug, Clone)]
pub(super) struct AvisoTipo {
    pub col: usize,
    pub nome: String,
    pub para: String,
    pub falhas: usize,
    pub exemplos: Vec<String>,
}

/// Quantas mudanças de etapas o Ctrl+Z da consulta guarda.
const MAX_DESFAZER: usize = 100;

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

/// Acrescenta a etapa. Renomear seguido de renomear vira uma etapa só ("Colunas renomeadas"),
/// e mover seguido de mover também, como no Power Query; `total` = colunas antes dela.
fn empilhar(passos: &mut Vec<Passo>, passo: Passo, total: usize) {
    match (passos.last_mut(), passo) {
        (Some(ultimo @ (Passo::Renomear { .. } | Passo::RenomearColunas { .. })), Passo::Renomear { col, nome }) => {
            let mut nomes = match std::mem::replace(ultimo, Passo::Inverter) {
                Passo::Renomear { col, nome } => vec![(col, nome)],
                Passo::RenomearColunas { nomes } => nomes,
                _ => unreachable!(),
            };
            match nomes.iter_mut().find(|(c, _)| *c == col) {
                Some(par) => par.1 = nome,
                None => nomes.push((col, nome)),
            }
            *ultimo = Passo::RenomearColunas { nomes };
        }
        (Some(Passo::Reordenar { ordem: antes }), Passo::Reordenar { ordem }) => {
            let mut cheia: Vec<usize> = antes.clone();
            cheia.extend((0..total).filter(|c| !antes.contains(c)));
            let mut nova: Vec<usize> = ordem.iter().filter_map(|&i| cheia.get(i).copied()).collect();
            let faltam: Vec<usize> = cheia.iter().copied().filter(|c| !nova.contains(c)).collect();
            nova.extend(faltam);
            *antes = nova;
        }
        (_, passo) => passos.push(passo),
    }
}

/// Monta a tabela inteira da opção escolhida no Navegador fora da thread da janela.
fn ler_item(ticket: u64, item: importador::Item, origem: String) -> Task<Msg> {
    Task::perform(async move {
        tokio::task::spawn_blocking(move || item.ler())
            .await.unwrap_or_else(|e| Err(trf!("Falha no importador: {e}")))
    }, move |resultado| Msg::OpenQuery(Evento::ItemLido(ticket, resultado, origem)))
}

/// O recorte pela área usada, a leitura de valores calculados e o descarte
/// de linhas vazias pertencem ao motor Rust (reutilizável via WASM).
fn ler_tabela(wb: &Workbook, aba: usize, area: Rect) -> Result<Tabela, String> {
    feathersheet_core::consulta::sheet_query::ler_intervalo(
        wb, aba, area, true, u64::MAX,
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
            url_web: String::new(), tabelas_web: Vec::new(), itens_arquivo: Vec::new(), item_pedido: None, caminho_arquivo: None,
            origem_web_pendente: String::new(), indice_tabela_web: 0,
            mostrar_web: false, mostrar_tabelas: false,
            passos: Vec::new(), resultado,
            codigo: String::new(),
            editor_avancado: None, erro_avancado: String::new(), aviso_avancado: String::new(), consultas_pq: None,
            erro: String::new(),
            edicao: None, salvas: Vec::new(), exclusao_pendente: None, renomeando: None,
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
            modal: None,
            erro_modal: String::new(),
            selecionadas: vec![0],
            modificadores: pelicano::Modifiers::default(),
            menu_coluna: None,
            menu_tipo: None,
            submenu: None,
            menu_obter: false,
            filtro_arquivo: None,
            ponteiro: (0.0, 0.0),
            posicao_menu: (0.0, 0.0),
            janela: pelicano::Size::new(1280.0, 800.0),
            recolhidos: [false; 3],
            rolagem: (0.0, 0.0, 1600.0, 1000.0),
            formatos: Formatos::new(),
            historico: Vec::new(), refazer: Vec::new(), etapas_atuais: "[]".into(),
            aviso_tipo: None,
            dependencias: None,
        }
    }

    /// Descarta a prévia HTML sem tocar na fonte ou nas etapas já aplicadas.
    /// Concentra a liberação de memória quando a origem é alterada.
    fn limpar_previa_web(&mut self) {
        self.tabelas_web.clear();
        self.itens_arquivo.clear();
        self.origem_web_pendente.clear();
        self.indice_tabela_web = 0;
    }


    /// Só publica a edição avançada quando parser E motor aceitam tudo.
    /// Assim, Cancelar/erro não destroem a consulta anterior.
    /// `Ok(true)`: o script escolheu outra planilha do arquivo, e a fonte precisa ser relida.
    fn aplicar_editor_avancado(&mut self) -> Result<bool, String> {
        let mut texto = self.editor_avancado.as_ref()
            .ok_or(tr!("Editor avançado não está aberto"))?.text();
        // M do Power Query colado: vira o script. Com etapa sem equivalente, para e mostra.
        if power_query::parece_m(&texto) {
            let d = power_query::de_m(&texto).map_err(|e| trf!("Não consegui ler o Power Query: {e}"))?;
            let primeira = script::comandos(&self.codigo).into_iter().next()
                .map(|(_, l)| format!("{l};")).unwrap_or_default();
            texto = d.script(&primeira, self.origem_aba.starts_with("Arquivo: "));
            if d.nao_convertidas > 0 {
                if let Some(ed) = &self.editor_avancado { ed.set_text(&texto); }
                self.aviso_avancado = trf!("Convertido do Power Query. {} etapa(s) sem equivalente ficaram comentadas: confira e clique Aplicar.", d.nao_convertidas);
                return Ok(false);
            }
        }
        let (base, atual) = match self.origem_aba.split_once(" ▸ ") {
            Some((b, i)) => (b.to_owned(), Some(i.to_owned())),
            None => (self.origem_aba.clone(), None),
        };
        let pedida = script::navegacao(&texto);
        let trocar = pedida.is_some() && pedida != atual;
        if trocar && !base.starts_with("Arquivo: ") {
            return Err(tr!("Só uma fonte de arquivo (.xlsx, .fshz…) tem planilhas para escolher.").into());
        }
        let passos = script::ler(&texto, &self.fonte, &self.fontes)?;
        let (gerado, tabela) = script::gerar(&self.origem_aba, &self.fonte, &passos, &self.fontes)?;
        self.codigo = gerado;
        self.passos = passos;
        self.coluna = self.coluna.min(tabela.cabecalho.len().saturating_sub(1));
        self.coluna_numero = (self.coluna + 1).to_string();
        self.resultado = tabela;
        self.passo_previa = None;
        self.tabela_previa = None;
        self.mostrar_manual = false;
        self.editor_avancado = None;
        self.erro_avancado.clear();
        self.erro.clear();
        if trocar { self.item_pedido = pedida; }
        Ok(trocar)
    }

    fn recalcular(&mut self) {
        // Recusa explosões de cardinalidade ANTES de alocar o resultado.
        let tabela = match script::gerar(&self.origem_aba, &self.fonte, &self.passos, &self.fontes) {
            Ok((texto, tabela)) => {
                self.codigo = texto;
                tabela
            }
            Err(erro) => { self.erro = erro; return; }
        };
        self.coluna = self.coluna.min(tabela.cabecalho.len().saturating_sub(1));
        self.coluna_numero = (self.coluna + 1).to_string();
        let n = tabela.cabecalho.len();
        self.selecionadas.retain(|c| *c < n);
        if self.selecionadas.is_empty() { self.selecionadas.push(self.coluna); }
        self.resultado = tabela;
        self.passo_previa = None;
        self.tabela_previa = None;
        self.erro.clear();
        self.aviso_tipo = self.aviso_de_tipo();
    }

    /// Conta o que a última etapa de tipo não conseguiu converter (o valor sumiu).
    // shortcut: roda a consulta até a etapa anterior de novo; só quando a última é de tipo.
    fn aviso_de_tipo(&self) -> Option<AvisoTipo> {
        let i = self.passos.len().checked_sub(1)?;
        let Passo::Tipar { col, para, .. } = &self.passos[i] else { return None };
        if para == "texto" { return None; }
        let antes = executar_query_limitado(self.fonte.clone(), &self.passos[..i], &self.fontes, u64::MAX).ok()?;
        let vazio = |v: Option<&Value>| match v {
            None | Some(Value::Empty) => true,
            Some(Value::Text(s)) => s.trim().is_empty(),
            _ => false,
        };
        let mut falhas = 0;
        let mut exemplos: Vec<String> = Vec::new();
        for (a, d) in antes.linhas.iter().zip(&self.resultado.linhas) {
            let erro = |v: Option<&Value>| matches!(v, Some(Value::Error(_)));
            if !vazio(a.get(*col)) && !erro(a.get(*col)) && (vazio(d.get(*col)) || erro(d.get(*col))) {
                falhas += 1;
                let t = a.get(*col).map(feathersheet_core::celulas::values::to_text).unwrap_or_default();
                if exemplos.len() < 3 && !exemplos.contains(&t) { exemplos.push(t); }
            }
        }
        (falhas > 0).then(|| AvisoTipo {
            col: *col, nome: self.resultado.cabecalho.get(*col).cloned().unwrap_or_default(),
            para: para.clone(), falhas, exemplos,
        })
    }

    /// Do aviso de tipo: os que não converteram viram `#VALOR!` e saem (`remover`) ou viram 0.
    fn tratar_falhas_de_tipo(&mut self, remover: bool) {
        let Some(aviso) = self.aviso_tipo.take() else { return };
        // Consulta antiga (falha vira vazio): só aí a etapa de tipo passa a marcar o erro.
        if let Some(Passo::Tipar { erros, .. }) = self.passos.last_mut() { *erros = true; }
        self.passos.push(if remover { Passo::RemoverErros { col: aviso.col } }
            else { Passo::SubstituirErros { col: aviso.col, para: Value::Number(0.0) } });
        self.registrar_passos();
    }

    fn tabela_visual(&self) -> &Tabela {
        self.tabela_previa.as_ref().unwrap_or(&self.resultado)
    }

    fn prever(&mut self, etapa: Option<usize>) {
        if let Some(i) = etapa {
            if i >= self.passos.len() { return; }
            match executar_query_limitado(
                self.fonte.clone(), &self.passos[..=i], &self.fontes, u64::MAX,
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

    /// As etapas mudaram por fora do script: recalcula, e o script é gerado de novo.
    /// A lista anterior vai para o Ctrl+Z.
    fn registrar_passos(&mut self) {
        self.recalcular();
        let atual = json_passos::escrever(&self.passos).unwrap_or_default();
        if atual != self.etapas_atuais {
            let anterior = std::mem::replace(&mut self.etapas_atuais, atual);
            self.historico.push(anterior);
            if self.historico.len() > MAX_DESFAZER { self.historico.remove(0); }
            self.refazer.clear();
        }
    }

    /// Fonte nova: as etapas da fonte anterior não voltam pelo Ctrl+Z.
    fn esquecer_historico(&mut self) {
        self.historico.clear();
        self.refazer.clear();
        self.etapas_atuais = json_passos::escrever(&self.passos).unwrap_or_default();
    }

    /// Ctrl+Z (`para_tras`) ou Ctrl+Y nas etapas da consulta.
    fn desfazer_etapas(&mut self, para_tras: bool) -> bool {
        let (de, para) = if para_tras { (&mut self.historico, &mut self.refazer) } else { (&mut self.refazer, &mut self.historico) };
        let Some(lista) = de.pop() else { return false };
        match json_passos::ler(&lista) {
            Ok(passos) => {
                para.push(std::mem::replace(&mut self.etapas_atuais, lista));
                self.passos = passos;
                self.modal = None;
                self.recalcular();
                true
            }
            Err(e) => { self.erro = e; false }
        }
    }

    fn adicionar(&mut self, tipo: &str) {
        let col = self.coluna;
        let total = self.resultado.cabecalho.len();
        let sel: Vec<usize> = self.selecionadas.iter().copied().filter(|c| *c < total).collect();
        if col >= total && !matches!(tipo, "indice" | "limite" | "distintas" |
            "promover" | "rebaixar" | "acrescentar" | "transpor" | "inverter") {
            self.erro = tr!("Selecione uma coluna na prévia").into();
            return;
        }
        let passo = match tipo {
            "filtrar" => Passo::Filtrar { col, criterio: Value::Text(self.filtro.clone()) },
            "ordenar" => Passo::Ordenar { por: vec![ChaveOrdenacao { col, desc: Some(false) }] },
            "decrescente" => Passo::Ordenar { por: vec![ChaveOrdenacao { col, desc: Some(true) }] },
            "texto" => Passo::Tipar { col, para: "texto".into(), erros: false },
            "numero" => Passo::Tipar { col, para: "numero".into(), erros: false },
            "renomear" => {
                if self.renomear.trim().is_empty() {
                    self.erro = tr!("Informe o novo nome da coluna").into();
                    return;
                }
                Passo::Renomear { col, nome: self.renomear.trim().into() }
            }
            "remover_coluna" => Passo::Colunas {
                manter: (0..total).filter(|i| *i != col && !sel.contains(i)).collect(),
            },
            "manter_colunas" => Passo::Colunas {
                manter: if sel.is_empty() { vec![col] } else { sel.clone() },
            },
            // Uma coluna só selecionada: linha inteira repetida; várias: só elas.
            "distintas" => Passo::Distintas { cols: (sel.len() > 1).then(|| sel.clone()) },
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
            "transpor" => Passo::Transpor,
            "duplicar" => Passo::DuplicarColuna { col, nome: None },
            "mover_esquerda" | "mover_direita" | "mover_inicio" | "mover_fim" => {
                let destino = match tipo {
                    "mover_esquerda" => col.saturating_sub(1),
                    "mover_direita" => (col + 1).min(total.saturating_sub(1)),
                    "mover_inicio" => 0,
                    _ => total.saturating_sub(1),
                };
                if destino == col { return; }
                let mut ordem: Vec<usize> = (0..total).filter(|c| *c != col).collect();
                ordem.insert(destino, col);
                // A seleção acompanha a coluna, para mover de novo.
                self.coluna = destino;
                self.selecionadas = vec![destino];
                Passo::Reordenar { ordem }
            }
            "inverter" => Passo::Inverter,
            "rebaixar" => Passo::RebaixarCabecalho,
            "remover_erros" => Passo::RemoverErros { col },
            "dividir" => {
                if self.separador.is_empty() {
                    self.erro = tr!("Informe um separador antes de dividir").into();
                    return;
                }
                Passo::Dividir { col, separador: self.separador.clone() }
            }
            "mesclar" if sel.len() > 1 => Passo::Mesclar {
                cols: sel.clone(), separador: self.separador.clone(), nome: Some("Mesclado".into()),
            },
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
                manter: if sel.is_empty() { vec![col] } else { sel.clone() }, nome_atributo: None, nome_valor: None,
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
                        modo: Some("esquerda".into()), semelhanca: None,
                    }
                } else {
                    Passo::Acrescentar { fonte: nome.clone() }
                }
            }
            _ => return,
        };
        empilhar(&mut self.passos, passo, total);
        self.registrar_passos();
    }
}

impl App {
    /// Roteia o evento e mantém a janela própria do FeatherQuery em sincronia
    /// com o estado: abre quando a consulta nasce, fecha quando termina.
    /// Põe a área `area` da aba `aba` como fonte da consulta aberta (tabela ou intervalo).
    fn carregar_fonte_openquery(&mut self, aba: usize, area: Rect, nome: &str) {
        let Some(origem_aba) = self.workbook.aba(aba).map(|a| a.sheet.name.clone()) else { return };
        match ler_tabela(&self.workbook, aba, area.clone()) {
            Ok(fonte) => if let Some(e) = &mut self.openquery {
                e.formatos = formatos_da_fonte(&self.workbook, aba, &area, &fonte.cabecalho);
                e.fonte = fonte;
                e.rolagem = (0.0, 0.0, e.rolagem.2, e.rolagem.3);
                e.origem_aba = origem_aba;
                e.origem = area;
                e.passos.clear();
                e.esquecer_historico();
                e.fontes.clear();
                e.edicao = None;
                e.nome = trf!("Consulta_{nome}");
                e.mostrar_tabelas = false;
                e.mostrar_web = false;
                e.mostrar_manual = false;
                e.assistente = None;
                e.registrar_passos();
                self.status = trf!("FeatherQuery: tabela '{nome}' carregada");
            },
            Err(erro) => if let Some(e) = &mut self.openquery { e.erro = erro; }
        }
    }

    /// Dados → Conexões existentes: as consultas salvas na pasta (id, nome).
    pub(super) fn conexoes_existentes(&self) -> Vec<(String, String)> {
        registro::listar(&self.workbook).unwrap_or_default().into_iter().filter_map(|item| Some((
            item.dados["id"].as_str()?.to_string(),
            item.dados["nome"].as_str()?.to_string(),
        ))).collect()
    }

    /// Guarda arquivo ou URL nas Fontes recentes da aba Dados.
    pub(super) fn lembrar_fonte(&mut self, fonte: String) {
        let lista = &mut self.config.fontes_recentes;
        lista.retain(|f| *f != fonte);
        lista.insert(0, fonte);
        lista.truncate(8);
        self.config.gravar();
    }

    pub(super) fn evento_openquery_janela(&mut self, evento: Evento) -> Task<Msg> {
        if let Evento::AbrirCom(dentro) = evento {
            let abrir = if self.openquery.is_none() { self.evento_openquery_janela(Evento::Abrir) } else { Task::none() };
            return Task::batch([abrir, self.evento_openquery_janela(*dentro)]);
        }
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
                // O FeatherQuery não importa a seleção/aba corrente por padrão.
                // A pessoa escolhe explicitamente Arquivo, Web ou Tabela existente.
                let vazio = Tabela { cabecalho: Vec::new(), linhas: Vec::new() };
                let mut editor = Estado::novo(vazio, String::new(), Rect::new(0, 0, 0, 0));
                editor.salvas = consultas.into_iter().filter_map(|item| Some((
                    item.dados["id"].as_str()?.to_string(),
                    item.dados["nome"].as_str()?.to_string(),
                ))).collect();
                self.openquery = Some(editor);
                self.menu_aberto = false;
                self.contexto = None;
                self.status = tr!("FeatherQuery: escolha um arquivo, uma URL ou uma tabela existente").into();
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
                    self.carregar_fonte_openquery(i, area, &nome);
                } else if let Some(e) = &mut self.openquery {
                    e.erro = trf!("A tabela '{nome}' não existe mais nesta pasta");
                }
            }
            Evento::CarregarAba(i) => {
                let Some(aba) = self.workbook.aba(i) else { return Task::none() };
                let nome = aba.sheet.name.clone();
                let area = importador::sem_titulo(&self.workbook, i, aba.sheet.used_rect());
                self.carregar_fonte_openquery(i, area, &nome);
            }
            // Normalmente tratado no `evento_openquery_janela`, que também abre a janela.
            Evento::AbrirCom(dentro) => {
                if self.openquery.is_none() { let _ = self.evento_openquery(Evento::Abrir); }
                return self.evento_openquery(*dentro);
            }
            Evento::CarregarSelecao => {
                let Some(aba) = self.workbook.aba(self.folha) else { return Task::none() };
                let selecao = self.area_selecionada();
                let usada = aba.sheet.used_rect();
                let area = if selecao.r0 == selecao.r1 && selecao.c0 == selecao.c1 {
                    feathersheet_core::operacoes::ops::regiao_atual(&aba.sheet, selecao)
                } else {
                    Rect::new(selecao.r0, selecao.c0, selecao.r1.min(usada.r1), selecao.c1.min(usada.c1))
                };
                let nome = aba.sheet.name.clone();
                self.carregar_fonte_openquery(self.folha, area, &nome);
            }
            Evento::AbrirRecente(fonte) => {
                if fonte.starts_with("http://") || fonte.starts_with("https://") {
                    if let Some(e) = &mut self.openquery {
                        e.url_web = fonte;
                        e.mostrar_web = true;
                        e.mostrar_tabelas = false;
                        e.mostrar_manual = false;
                    }
                    return self.evento_openquery(Evento::ImportarWeb);
                }
                let Some(e) = &mut self.openquery else { return Task::none() };
                e.limpar_previa_web();
                e.importacao_preservar = false;
                let ticket = IMPORTACAO_SEQ.fetch_add(1, Ordering::Relaxed);
                e.importacao_pendente = Some(ticket);
                return self.evento_openquery(Evento::ArquivoEscolhido(ticket, Some(fonte.into())));
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
                self.lembrar_fonte(url.trim().to_owned());
                let Some(e) = &mut self.openquery else { return Task::none() };
                e.limpar_previa_web();
                e.importacao_pendente = Some(ticket);
                // reqwest normaliza URLs (ex.: https://site => https://site/).
                // Comparar strings destruiria as etapas numa atualização normal.
                let mesma_url = e.origem_aba.strip_prefix("Web: ")
                    .and_then(|antiga| reqwest::Url::parse(antiga).ok())
                    .zip(reqwest::Url::parse(url.trim()).ok())
                    .is_some_and(|(antiga, nova)| antiga == nova);
                e.importacao_preservar = mesma_url && !e.fonte.cabecalho.is_empty();
                self.status = tr!("FeatherQuery: baixando dados Web em tarefa separada").into();
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
                                "FeatherQuery: {} tabelas encontradas; selecione qual importar",
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
                if indice < e.itens_arquivo.len() {
                    let item = e.itens_arquivo.swap_remove(indice);
                    let origem = format!("{} ▸ {}", std::mem::take(&mut e.origem_web_pendente), item.nome);
                    e.limpar_previa_web();
                    e.importacao_pendente = Some(ticket);
                    return ler_item(ticket, item, origem);
                }
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
                if e.modificadores.shift() {
                    let (a, b) = (e.coluna.min(col), e.coluna.max(col));
                    e.selecionadas = (a..=b).collect();
                } else if e.modificadores.control() {
                    match e.selecionadas.iter().position(|c| *c == col) {
                        Some(i) if e.selecionadas.len() > 1 => { e.selecionadas.remove(i); }
                        Some(_) => {}
                        None => { e.selecionadas.push(col); e.selecionadas.sort_unstable(); }
                    }
                } else {
                    e.selecionadas = vec![col];
                }
                e.coluna = col;
                e.coluna_numero = (col + 1).to_string();
                e.menu_coluna = None;
            },
            Evento::MenuColuna(col) => if let Some(e) = &mut self.openquery {
                // Abrir o menu numa coluna fora da seleção seleciona só ela.
                if let Some(c) = col {
                    if !e.selecionadas.contains(&c) { e.selecionadas = vec![c]; }
                    e.coluna = c;
                    e.posicao_menu = e.ponteiro;
                }
                e.menu_coluna = col;
                e.submenu = None;
            },
            Evento::Submenu(i) => if let Some(e) = &mut self.openquery { e.submenu = i; },
            Evento::Ponteiro(x, y) => if let Some(e) = &mut self.openquery { e.ponteiro = (x, y); },
            Evento::MenuTipo(col, posicao) => if let Some(e) = &mut self.openquery {
                e.menu_coluna = None;
                if let Some(c) = col {
                    if !e.selecionadas.contains(&c) { e.selecionadas = vec![c]; }
                    e.coluna = c;
                    e.posicao_menu = posicao.unwrap_or(e.ponteiro);
                }
                e.menu_tipo = col;
            },
            Evento::Tipar(col, para) => if let Some(e) = &mut self.openquery {
                e.menu_tipo = None;
                {
                        // Como o "Tipo Alterado" do Power Query: trocar de novo não empilha etapa.
                        match e.passos.last_mut() {
                            Some(Passo::Tipar { col: c, para: p, .. }) if *c == col => *p = para.into(),
                            // O que não converte vira #VALOR!, como no Power Query: tratar é outra etapa.
                            _ => e.passos.push(Passo::Tipar { col, para: para.into(), erros: true }),
                        }
                        e.registrar_passos();
                    }
            },
            Evento::Modificadores(m) => if let Some(e) = &mut self.openquery { e.modificadores = m; },
            Evento::JanelaRedimensionada(t) => if let Some(e) = &mut self.openquery { e.janela = t; },
            Evento::Formatos(ticket, f) => if let Some(e) = &mut self.openquery {
                if e.importacao_pendente == Some(ticket) { e.formatos = f; }
            },
            Evento::RolarPrevia(v) => if let Some(e) = &mut self.openquery {
                e.rolagem = (v.offset.x, v.offset.y, v.bounds.width, v.bounds.height);
            },
            Evento::RecolherGrupo(i) => if let Some(e) = &mut self.openquery {
                if let Some(r) = e.recolhidos.get_mut(i as usize) { *r = !*r; }
            },
            Evento::EditarPasso(i) => if let Some(e) = &mut self.openquery {
                if let Some(passo) = e.passos.get(i) {
                        let entrada = executar_query_limitado(e.fonte.clone(), &e.passos[..i], &e.fontes, u64::MAX)
                            .unwrap_or(Tabela { cabecalho: Vec::new(), linhas: Vec::new() });
                        e.modal = etapa::Modal::de_passo(passo, i, entrada.cabecalho.clone())
                            .map(|m| m.com_amostra(&entrada, e.fontes.clone()));
                        if let Some(m) = &mut e.modal { if let Some(c) = m.quer_valores() { m.definir_valores(&entrada, c); } }
                        e.erro_modal.clear();
                    }
            },
            Evento::Dependencias(abrir) => {
                let mapa = abrir.then(|| registro::dependencias(&self.workbook));
                if let Some(e) = &mut self.openquery { e.dependencias = mapa; }
            }
            Evento::FalhasDeTipo(acao) => if let Some(e) = &mut self.openquery {
                match acao {
                    0 | 1 => e.tratar_falhas_de_tipo(acao == 0),
                    _ => e.aviso_tipo = None,
                }
            },
            Evento::MenuObter(abrir) => if let Some(e) = &mut self.openquery {
                e.menu_obter = abrir;
                e.submenu = None;
            },
            Evento::DoMenuObter(pedido) => {
                if let Some(e) = &mut self.openquery { e.menu_obter = false; e.submenu = None; }
                return self.evento_openquery(*pedido);
            }
            Evento::ImportarTipo(i) => {
                if let Some(e) = &mut self.openquery { e.filtro_arquivo = Some(i); }
                return self.evento_openquery(Evento::ImportarArquivo);
            }
            Evento::Duplicar | Evento::Referenciar | Evento::ReferenciarSalva(_) => {
                let id = match &evento { Evento::ReferenciarSalva(id) => Some(id.clone()), _ => self.openquery.as_ref().and_then(|e| e.edicao.clone()) };
                let Some(id) = id else {
                    if let Some(e) = &mut self.openquery { e.erro = tr!("Salve a consulta primeiro (Salvar e carregar ou Carregar em…)").into(); }
                    return Task::none();
                };
                let feito = if matches!(evento, Evento::Duplicar) {
                    self.duplicar_consulta(&id).map(|nome| trf!("FeatherQuery: '{nome}' criada como conexão; use Carregar em… para escolher o destino"))
                } else {
                    self.referenciar_consulta(&id).map(|()| tr!("FeatherQuery: consulta nova sobre o resultado da outra; acrescente as etapas e salve").to_string())
                };
                match feito {
                    Ok(msg) => {
                        self.status = msg;
                        let salvas = self.conexoes_existentes();
                        if let Some(e) = &mut self.openquery { e.salvas = salvas; e.erro.clear(); }
                    }
                    Err(erro) => if let Some(e) = &mut self.openquery { e.erro = erro },
                }
            }
            Evento::Desfazer(para_tras) => if let Some(e) = &mut self.openquery {
                if !e.desfazer_etapas(para_tras) {
                    self.status = if para_tras { tr!("FeatherQuery: nada a desfazer").into() } else { tr!("FeatherQuery: nada a refazer").into() };
                }
            },
            Evento::EtapaSenao(t) => if let Some(m) = self.openquery.as_mut().and_then(|e| e.modal.as_mut()) {
                m.definir_senao(t);
            },
            Evento::Etapa(ev) => if let Some(e) = &mut self.openquery {
                match ev {
                    etapa::Evento::Fechar => e.modal = None,
                    etapa::Evento::Aplicar => {
                        let Some(m) = &e.modal else { return Task::none() };
                        let passo = match m.passo() {
                            Ok(p) => p,
                            Err(erro) => { e.erro_modal = erro; return Task::none(); }
                        };
                        let editar = m.editar;
                        match editar {
                            Some(i) if i < e.passos.len() => e.passos[i] = passo,
                            _ => empilhar(&mut e.passos, passo, e.resultado.cabecalho.len()),
                        }
                        e.modal = None;
                        e.registrar_passos();
                    }
                    outro => if let Some(m) = &mut e.modal {
                        m.atualizar(outro);
                        // Filtro por lista: os valores da coluna escolhida, da tabela que chega à etapa.
                        if let Some(c) = m.quer_valores() {
                            match m.editar {
                                Some(i) => {
                                    let entrada = executar_query_limitado(e.fonte.clone(), &e.passos[..i.min(e.passos.len())], &e.fontes, u64::MAX)
                                        .unwrap_or(Tabela { cabecalho: Vec::new(), linhas: Vec::new() });
                                    m.definir_valores(&entrada, c);
                                }
                                None => m.definir_valores(&e.resultado, c),
                            }
                        }
                    },
                }
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
                e.menu_coluna = None;
                if etapa::COM_MODAL.contains(&tipo) && !matches!(tipo, "ordenar" | "tipar") {
                    let colunas = e.resultado.cabecalho.clone();
                    if colunas.is_empty() {
                        e.erro = tr!("Selecione uma fonte de dados para iniciar.").into();
                    } else {
                        let mut m = etapa::Modal::novo(tipo, colunas, e.coluna, &e.selecionadas)
                            .com_amostra(&e.resultado, e.fontes.clone());
                        if tipo == "extrair_texto_nova" { m = m.em_coluna_nova(); }
                        e.modal = Some(m);
                        e.erro_modal.clear();
                    }
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
                if idx < e.passos.len() {
                        e.passos.remove(idx);
                        e.registrar_passos();
                    }
            },
            Evento::Mover(idx, delta) => if let Some(e) = &mut self.openquery {
                {
                        let novo = (idx as isize).saturating_add(delta as isize);
                        if idx < e.passos.len() && novo >= 0 &&
                            (novo as usize) < e.passos.len() {
                            e.passos.swap(idx, novo as usize);
                            e.registrar_passos();
                        }
                    }
            },
            Evento::Prever(etapa) => if let Some(e) = &mut self.openquery {
                e.prever(etapa);
            },
            Evento::AbrirEditorAvancado => if let Some(e) = &mut self.openquery {
                e.editor_avancado = Some(Content::with_text(&e.codigo));
                e.erro_avancado.clear();
                e.aviso_avancado.clear();
            },
            Evento::ConsultasPowerQuery => {
                let lidas = self.arquivo_xlsx_aberto()
                    .ok_or_else(|| tr!("Salve a pasta como .xlsx para ler as consultas do Power Query.").to_owned())
                    .and_then(|caminho| feathersheet_io::power_query::secao_do_arquivo(&caminho))
                    .and_then(|secao| match secao {
                        Some(texto) => power_query::consultas_da_secao(&texto),
                        None => Ok(Vec::new()),
                    });
                if let Some(e) = &mut self.openquery {
                    match lidas {
                        Ok(lista) if lista.is_empty() => e.erro = tr!("Este arquivo não tem consultas do Power Query.").into(),
                        Ok(lista) => { e.erro.clear(); e.consultas_pq = Some(lista); }
                        Err(erro) => e.erro = trf!("Não consegui ler as consultas do Power Query: {erro}"),
                    }
                }
            }
            Evento::AbrirConsultaPowerQuery(i) => if let Some(e) = &mut self.openquery {
                let Some((nome, m)) = e.consultas_pq.take().and_then(|l| l.into_iter().nth(i)) else { return Task::none() };
                match power_query::de_m(&m) {
                    Ok(d) => {
                        let primeira = script::comandos(&e.codigo).into_iter().next()
                            .map(|(_, l)| format!("{l};")).unwrap_or_default();
                        let texto = d.script(&primeira, e.origem_aba.starts_with("Arquivo: "));
                        if e.edicao.is_none() { e.nome = nome.clone(); }
                        e.editor_avancado = Some(Content::with_text(&texto));
                        e.erro_avancado.clear();
                        e.aviso_avancado = if d.nao_convertidas == 0 {
                            trf!("Consulta {nome} do Power Query: confira e clique Aplicar.")
                        } else {
                            trf!("Consulta {nome} do Power Query: {} etapa(s) sem equivalente ficaram comentadas. Confira e clique Aplicar.", d.nao_convertidas)
                        };
                    }
                    Err(erro) => e.erro = trf!("Não consegui ler o Power Query: {erro}"),
                }
            },
            Evento::FecharConsultasPowerQuery => if let Some(e) = &mut self.openquery {
                e.consultas_pq = None;
            },
            Evento::CopiarPowerQuery => if let Some(e) = &mut self.openquery {
                let manual = (e.origem_aba == "Entrada manual").then_some(&e.fonte);
                match power_query::para_m(&e.codigo, manual, &e.fontes) {
                    Ok((texto, sem)) => {
                        e.erro_avancado.clear();
                        e.aviso_avancado = if sem == 0 {
                            tr!("Copiado como Power Query: cole no Editor Avançado do Excel.").into()
                        } else {
                            trf!("Copiado como Power Query. {sem} etapa(s) sem equivalente foram como comentário.")
                        };
                        return Task::write_clipboard(texto);
                    }
                    Err(erro) => e.erro_avancado = erro,
                }
            },
            Evento::EditarEditorAvancado => if let Some(e) = &mut self.openquery {
                if e.editor_avancado.is_some() {
                    e.erro_avancado.clear();
                }
            },
            Evento::AplicarEditorAvancado => if let Some(e) = &mut self.openquery {
                match e.aplicar_editor_avancado() {
                    Err(erro) => e.erro_avancado = erro,
                    Ok(true) => return self.evento_openquery(Evento::ReimportarArquivo),
                    Ok(false) => {}
                }
            },
            Evento::InserirNoEditor(linha) => if let Some(e) = &mut self.openquery {
                if let Some(ed) = &e.editor_avancado {
                    // No meio de uma linha, a etapa nova começa na seguinte.
                    // Cursor na linha 1 (o `let fonte` ou editor ainda sem clique): vai para o fim.
                    let (l, c) = ed.cursor();
                    if l == 1 {
                        let mut texto = ed.text();
                        if !texto.is_empty() && !texto.ends_with('\n') { texto.push('\n'); }
                        ed.set_text(&format!("{texto}{linha}\n"));
                    } else {
                        let meio = c > 1 && ed.line(l).is_some_and(|t| !t.trim().is_empty());
                        ed.insert(&if meio { format!("\n{linha}") } else { format!("{linha}\n") });
                    }
                    e.erro_avancado.clear();
                }
            },
            Evento::FecharEditorAvancado => if let Some(e) = &mut self.openquery {
                e.editor_avancado = None;
                e.erro_avancado.clear();
                e.aviso_avancado.clear();
            },
            Evento::Atualizar => if let Some(e) = &mut self.openquery {
                e.recalcular();
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
                            e.recalcular();
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
                        e.rolagem = (0.0, 0.0, e.rolagem.2, e.rolagem.3);
                        e.origem_aba = "Entrada manual".into();
                        e.origem = Rect::new(0, 0, linhas as u32, colunas.saturating_sub(1) as u32);
                        e.edicao = None;
                        e.assistente = None;
                        e.passos.clear();
                        e.fontes.clear();
                        e.registrar_passos();
                        e.esquecer_historico();
                        e.mostrar_manual = false;
                        self.status = trf!("FeatherQuery: tabela manual importada ({linhas} linhas)");
                    }
                    Err(erro) => e.erro = erro,
                }
            },
            Evento::ImportarPasta => {
                let Some(editor) = &mut self.openquery else { return Task::none() };
                editor.limpar_previa_web();
                editor.importacao_preservar = false;
                let ticket = IMPORTACAO_SEQ.fetch_add(1, Ordering::Relaxed);
                editor.importacao_pendente = Some(ticket);
                return Task::perform(async {
                    rfd::AsyncFileDialog::new().pick_folder().await.map(|p| p.path().to_path_buf())
                }, move |pasta| Msg::OpenQuery(Evento::PastaEscolhida(ticket, pasta)));
            }
            Evento::PastaEscolhida(ticket, pasta) => {
                if self.openquery.as_ref().and_then(|e| e.importacao_pendente) != Some(ticket) {
                    return Task::none();
                }
                let Some(pasta) = pasta else {
                    if let Some(e) = &mut self.openquery { e.importacao_pendente = None; }
                    return Task::none();
                };
                self.status = tr!("FeatherQuery: lendo os arquivos da pasta em tarefa separada").into();
                return Task::perform(async move {
                    tokio::task::spawn_blocking(move || importador::ler_pasta(&pasta))
                        .await.unwrap_or_else(|e| Err(trf!("Falha no importador: {e}")))
                }, move |resultado| match resultado {
                    Ok((tabela, origem, formatos)) => Msg::Lote(vec![
                        Msg::OpenQuery(Evento::Formatos(ticket, formatos)),
                        Msg::OpenQuery(Evento::ArquivoLido(ticket, Ok((tabela, origem)))),
                    ]),
                    Err(erro) => Msg::OpenQuery(Evento::ArquivoLido(ticket, Err(erro))),
                });
            }
            Evento::ImportarArquivo | Evento::ReimportarArquivo => {
                let Some(editor) = &mut self.openquery else { return Task::none() };
                editor.limpar_previa_web();
                editor.importacao_preservar = matches!(evento, Evento::ReimportarArquivo);
                let ticket = IMPORTACAO_SEQ.fetch_add(1, Ordering::Relaxed);
                editor.importacao_pendente = Some(ticket);
                // Trocar de planilha pelo script: o arquivo lido nesta sessão, sem perguntar.
                if editor.importacao_preservar && editor.item_pedido.is_some() {
                    if let Some(caminho) = editor.caminho_arquivo.clone() {
                        return Task::done(Msg::OpenQuery(Evento::ArquivoEscolhido(ticket, Some(caminho))));
                    }
                }
                // Reler a pasta: o caminho já é conhecido e a pessoa pediu (o botão).
                if editor.importacao_preservar {
                    if let Some(pasta) = editor.origem_aba.strip_prefix("Pasta: ").map(std::path::PathBuf::from) {
                        return Task::done(Msg::OpenQuery(Evento::PastaEscolhida(ticket, Some(pasta))));
                    }
                }
                let tipo = editor.filtro_arquivo.take().and_then(|i| importador::TIPOS.get(i));
                return Task::perform(async move {
                    let mut caixa = rfd::AsyncFileDialog::new();
                    // O tipo pedido no menu vem primeiro; "Fontes FeatherQuery" segue como opção.
                    if let Some((nome, extensoes)) = tipo { caixa = caixa.add_filter(crate::idioma::tr(nome), extensoes); }
                    caixa.add_filter(tr!("Fontes FeatherQuery"), &importador::EXTENSOES)
                        .pick_file().await.map(|arquivo| arquivo.path().to_path_buf())
                }, move |arquivo| Msg::OpenQuery(Evento::ArquivoEscolhido(ticket, arquivo)));
            }
            Evento::ArquivoEscolhido(ticket, arquivo) => {
                if self.openquery.as_ref().and_then(|e| e.importacao_pendente) != Some(ticket) {
                    return Task::none();
                }
                let Some(caminho) = arquivo else {
                    if let Some(e) = &mut self.openquery { e.importacao_pendente = None; e.item_pedido = None; }
                    return Task::none();
                };
                self.lembrar_fonte(caminho.display().to_string());
                if let Some(e) = &mut self.openquery { e.caminho_arquivo = Some(caminho.clone()); }
                self.status = tr!("FeatherQuery: importando fonte em tarefa separada").into();
                return Task::perform(async move {
                    tokio::task::spawn_blocking(move || importador::ler_itens(&caminho))
                        .await.unwrap_or_else(|e| Err(trf!("Falha no importador: {e}")))
                }, move |resultado| match resultado {
                    Ok((itens, origem)) => Msg::OpenQuery(Evento::ItensLidos(ticket, itens, origem)),
                    Err(erro) => Msg::OpenQuery(Evento::ArquivoLido(ticket, Err(erro))),
                });
            }
            Evento::ItensLidos(ticket, mut itens, origem) => {
                let Some(e) = &mut self.openquery else { return Task::none() };
                if e.importacao_pendente != Some(ticket) { return Task::none(); }
                // Reimportar: a mesma planilha/tabela de antes, sem perguntar.
                let anterior = e.item_pedido.take()
                    .or_else(|| e.origem_aba.split_once(" ▸ ").map(|(_, n)| n.to_owned()));
                let escolhida = if e.importacao_preservar {
                    match anterior {
                        None => Some(0),
                        Some(n) => match itens.iter().position(|i| i.nome == n) {
                            Some(i) => Some(i),
                            None => {
                                e.importacao_pendente = None;
                                e.importacao_preservar = false;
                                e.erro = trf!("O arquivo não tem {}.", importador::nome_visivel(&n));
                                return Task::none();
                            }
                        },
                    }
                } else if itens.len() == 1 { Some(0) } else { None };
                if let Some(i) = escolhida {
                    let item = itens.swap_remove(i);
                    let origem = if itens.is_empty() && !e.importacao_preservar { origem } else { format!("{origem} ▸ {}", item.nome) };
                    return ler_item(ticket, item, origem);
                }
                e.tabelas_web = itens.iter().map(|i| i.previa.clone()).collect();
                e.itens_arquivo = itens;
                e.origem_web_pendente = origem;
                e.indice_tabela_web = 0;
                e.mostrar_web = true;
                e.erro.clear();
                self.status = trf!("FeatherQuery: {} planilhas e tabelas no arquivo; escolha qual importar", e.tabelas_web.len());
            }
            Evento::ItemLido(ticket, resultado, origem) => {
                let (resultado, formatos) = match resultado {
                    Ok((t, f)) => (Ok((t, origem)), Some(f)),
                    Err(erro) => (Err(erro), None),
                };
                let tarefa = self.evento_openquery(Evento::ArquivoLido(ticket, resultado));
                if let (Some(f), Some(e)) = (formatos, &mut self.openquery) { e.formatos = f; }
                return tarefa;
            }
            Evento::ArquivoLido(ticket, resultado) => {
                let Some(editor) = &mut self.openquery else { return Task::none() };
                if editor.importacao_pendente != Some(ticket) { return Task::none(); }
                editor.importacao_pendente = None;
                match resultado {
                    Err(erro) => {
                        editor.erro = erro.clone();
                        self.status = trf!("FeatherQuery: {erro}");
                    }
                    Ok((fonte, origem)) => {
                        let preservar = editor.importacao_preservar;
                        editor.importacao_preservar = false;
                        if preservar && !(editor.origem_aba.starts_with("Arquivo: ")
                            || editor.origem_aba.starts_with("Pasta: ")
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
                        editor.rolagem = (0.0, 0.0, editor.rolagem.2, editor.rolagem.3);
                        editor.origem_aba = origem.clone();
                        if let Some(url) = origem.strip_prefix("Web: ") {
                            editor.url_web = url.to_owned();
                        }
                        editor.mostrar_web = false;
                        editor.mostrar_tabelas = false;
                        editor.mostrar_manual = false;
                        editor.origem = Rect::new(0, 0, linhas as u32, colunas.saturating_sub(1) as u32);
                        editor.assistente = None;
                        if !preservar {                            editor.edicao = None;
                            editor.passos.clear();
                            editor.fontes.clear();
                        }
                        editor.registrar_passos();
                        self.status = if preservar {
                            trf!("FeatherQuery: reimportadas {linhas} linhas; salve para atualizar o resultado")
                        } else {
                            trf!("FeatherQuery: {origem} carregado ({linhas} linhas)")
                        };
                    }
                }
            },
            Evento::EditarSalva(id) => { self.abrir_consulta_salva(&id); },
            Evento::AtualizarSalva(id) => self.atualizar_consulta_salva(&id),
            Evento::AoAbrir(id) => {
                let ligado = !registro::buscar(&self.workbook, &id).is_ok_and(|s| registro::atualiza_ao_abrir(&s.dados));
                match registro::marcar_ao_abrir(&mut self.workbook, &id, ligado) {
                    Ok(()) => {
                        self.alterado = true;
                        self.status = if ligado { tr!("FeatherQuery: a consulta será atualizada ao abrir o arquivo").into() }
                            else { tr!("FeatherQuery: a consulta não será mais atualizada ao abrir o arquivo").into() };
                    }
                    Err(e) => self.status = e,
                }
            }
            Evento::PedirExclusao(id) => if let Some(e) = &mut self.openquery {
                e.exclusao_pendente = Some(id);
            },
            Evento::CancelarExclusao => if let Some(e) = &mut self.openquery {
                e.exclusao_pendente = None;
            },
            Evento::PedirRenomear(id) => if let Some(e) = &mut self.openquery {
                let nome = e.salvas.iter().find(|(chave, _)| *chave == id).map(|(_, n)| n.clone()).unwrap_or_default();
                e.renomeando = Some((id, nome));
            },
            Evento::TextoRenomear(texto) => if let Some((_, nome)) = self.openquery.as_mut().and_then(|e| e.renomeando.as_mut()) {
                *nome = texto;
            },
            Evento::CancelarRenomear => if let Some(e) = &mut self.openquery {
                e.renomeando = None;
            },
            Evento::ConfirmarRenomear => {
                let Some((id, novo)) = self.openquery.as_ref().and_then(|e| e.renomeando.clone()) else { return Task::none() };
                let antigo = self.openquery.as_ref().and_then(|e| e.salvas.iter().find(|(c, _)| *c == id).map(|(_, n)| n.clone()));
                match registro::renomear(&mut self.workbook, &id, &novo) {
                    Ok(()) => {
                        let novo = novo.trim().to_owned();
                        if let Some(e) = &mut self.openquery {
                            e.renomeando = None;
                            e.erro.clear();
                            for (chave, nome) in e.salvas.iter_mut() { if *chave == id { *nome = novo.clone(); } }
                            if e.edicao.as_deref() == Some(id.as_str()) { e.nome = novo.clone(); }
                            // A consulta aberta que lê a renomeada continua lendo.
                            if antigo.is_some_and(|a| e.origem_aba == format!("Consulta: {a}")) {
                                e.origem_aba = format!("Consulta: {novo}");
                            }
                        }
                        self.alterado = true;
                        self.status = trf!("Consulta renomeada para '{novo}'");
                    }
                    Err(erro) => if let Some(e) = &mut self.openquery { e.erro = erro; },
                }
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
                // A prévia volta como widget novo, no topo.
                e.rolagem.0 = 0.0;
                e.rolagem.1 = 0.0;
                e.mostrar_manual = aba == 2;
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

/// Mensagem do FeatherQuery: abrevia a construção repetida em cada botão.
fn om(evento: Evento) -> Msg {
    Msg::OpenQuery(evento)
}

fn negrito() -> pelicano::Font {
    pelicano::Font { weight: 600, ..pelicano::Font::DEFAULT }
}

/// Vermelho de erro do FeatherQuery (uma só definição).
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

/// O nome `'static` de uma etapa da faixa (para a captura de tela).
pub(super) fn tipo_de_etapa(nome: &str) -> Option<&'static str> {
    etapa::COM_MODAL.iter().chain(["transpor", "inverter", "indice"].iter()).find(|t| **t == nome).copied()
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
        Passo::DividirColunas { .. } => tr!("Dividir coluna"),
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
        Passo::Formatar { .. } => tr!("Formatar texto"),
        Passo::Preencher { .. } => tr!("Preencher"),
        Passo::RemoverLinhas { .. } => tr!("Remover linhas"),
        Passo::Transpor => tr!("Transpor"),
        Passo::Inverter => tr!("Inverter linhas"),
        Passo::DividirPosicoes { .. } => tr!("Dividir por posição"),
        Passo::Personalizada { .. } => tr!("Coluna personalizada"),
        Passo::FiltrarVarias { .. } => tr!("Filtrar (várias condições)"),
        Passo::FiltrarValores { .. } => tr!("Filtrar valores"),
        Passo::ExtrairTexto { .. } => tr!("Extrair texto"),
        Passo::RenomearColunas { .. } => tr!("Colunas renomeadas"),
        Passo::Reordenar { .. } => tr!("Colunas reordenadas"),
        Passo::DuplicarColuna { .. } => tr!("Coluna duplicada"),
    }
}

