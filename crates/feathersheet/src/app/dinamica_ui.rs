//! Tabela dinâmica nativa. Sem WebView, cópia densa da origem ou motor TypeScript.
//! O diálogo configura colunas absolutas e passa uma leitura preguiçosa ao core.
use super::*;
use feathersheet_core::{
    celulas::values::{to_text, Value},
    dinamica::{campos, pivot::{AgregacaoOp, AgruparData, DinamicaConfig, MedidaDinamica}},
    operacoes::ops,
};
use feathersheet_core::dinamica::dinamica_layout::{Filtro, Layout};
use feathersheet_core::dinamica::exibicao::{Agrupamento, Eixo as EixoExtra, Extras, MostrarComo, Ordenacao};

mod avancado;
mod painel_avancado;
mod edicao;
mod origem;
mod fluxo;
mod config_campo;
mod detalhes;
mod grafico_dinamico;
mod segmentacao;
mod faixa;
mod mcp;
pub(in crate::app) mod fundo;
#[cfg(test)]
mod testes_arrasto;
pub(super) use segmentacao::Segmentacoes;
use fluxo::Fase;
use avancado::{Calculado, calcular, conferir_nomes_calculados, gravar_opcoes, mapa_json};
use serde_json::json;

#[derive(Debug, Clone, Copy)]
pub enum EixoDinamica { Linhas, Colunas, Valores }

#[derive(Debug, Clone)]
pub enum Evento {
    /// Conta de fundo: mais um pedaço da leitura da origem (pulso).
    FundoPasso,
    /// Conta de fundo terminou (geração, resultado).
    FundoPronto(u64, fundo::Pronto),
    Abrir,
    Recomendadas,
    Preset(u8),
    Mover(EixoDinamica,usize,i32),
    DuplicarValor(usize),
    RemoverValor(usize),
    OperacaoValor(usize),
    OrigemTexto(String),
    OrigemAplicar,
    Fechar,
    Linha(usize),
    Coluna(usize),
    Valor(usize),
    Operacao(usize),
    Agrupar,
    Destino,
    Criar,
    Atualizar,
    Editar,
    Layout,
    RepetirRotulos,
    Subtotais,
    Tema,
    FiltroCampo(usize),
    FiltroValor(String),
    FiltroModo,
    LimparFiltro,
    NomeCalculado(String),
    FormulaCalculado(String),
    PreviaCalculado,
    AdicionarCalculado,
    RemoverCalculado(usize),
    Recolher(String),
    RecolherTodos(bool),
    RecolherNaGrade(u32,u32,String),
    Campo(usize),
    Remover(EixoDinamica,usize),
    RemoverFiltro(usize),
    AlternarAvancado,
    DestinoNova(bool),
    // Painel de campos no modelo do Excel (13/10/2026).
    /// Apertou o mouse num campo: da lista (`None`) ou de uma área.
    ArrastoInicio(usize, Option<(AreaDinamica, usize)>),
    ArrastoMover(pelicano::Point),
    ArrastoSoltar,
    ArrastoSobre(Option<AreaDinamica>),
    /// Passou sobre o campo `i` de uma área durante o arrasto: soltar ali
    /// põe o campo antes dele (reordena a hierarquia na mesma área).
    ArrastoSobreItem(Option<(AreaDinamica, usize)>),
    /// Saiu desta área (só limpa se ainda for a marcada: a vizinha pode ter entrado antes).
    ArrastoSair(AreaDinamica),
    BuscaCampo(String),
    MenuItem(Option<(AreaDinamica, usize)>),
    MoverPara(AreaDinamica, usize, AreaDinamica),
    Reordenar(AreaDinamica, usize, i32),
    FiltroBusca(String),
    FiltroTudo,
    FiltroOk,
    FiltroCancelar,
    LayoutEscolher(bool),
    AgruparEscolher(Option<AgruparData>),
    TemaEscolher(&'static str),
    // Configurações do campo (16/10/2026, v0.1.0): "Mostrar valores como",
    // classificar, agrupar e totais gerais. O motor já fazia; faltava a tela.
    /// Abre (ou fecha, com `None`) as configurações do campo da área.
    ConfigurarCampo(Option<(AreaDinamica, usize)>),
    OperacaoEscolher(usize, AgregacaoOp),
    MostrarComo(usize, MostrarComo),
    Ordenar(EixoExtra, usize, Ordenacao),
    AgruparCampo(EixoExtra, usize, Option<Agrupamento>),
    /// Início, fim ou intervalo (0, 1, 2) das faixas do campo aberto.
    FaixaTexto(usize, String),
    TotaisGerais { linhas: bool, ligado: bool },
    AtualizarSozinha(bool),
    /// Inserir ▸ Gráfico dinâmico.
    GraficoDinamico,
    // Segmentação de dados (básica, v0.1.0).
    SegmentacaoInserir,
    SegmentacaoCampo(usize),
    SegmentacaoFecharEscolha,
    /// (aba da dinâmica, id, coluna do campo, item)
    SegmentacaoItem(usize, String, usize, String),
    SegmentacaoLimpar(usize, String, usize),
    SegmentacaoRemover(usize, String, usize),
    /// Um botão da aba contextual "Tabela Dinâmica" (com ou sem o painel aberto).
    NaFaixa(Box<Evento>),
    /// Clique no funil de um filtro de relatório na grade (linha, coluna).
    FiltroNaGrade(u32, u32),
    /// A conta adiada de uma origem grande (o número da vez: só a última
    /// mudança calcula), depois que a barra de status mostrou o aviso.
    AtualizarAgora(u32),
}

/// As quatro áreas do painel, mais a lista de campos (soltar nela tira o campo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AreaDinamica { Filtros, Colunas, Linhas, Valores, Lista }

/// O que é só do painel (não vai para o arquivo).
#[derive(Debug, Clone, Default)]
pub(super) struct PainelUi {
    /// Vez da última conta adiada ([`Evento::AtualizarAgora`]).
    vez_adiada: u32,
    busca_campo: String,
    arrasto: Option<Arrasto>,
    /// Área sob o mouse durante o arrasto.
    sob: Option<AreaDinamica>,
    /// Campo de uma área sob o mouse durante o arrasto (onde vai entrar).
    sob_item: Option<(AreaDinamica, usize)>,
    /// Item com o menu ▾ aberto.
    menu_item: Option<(AreaDinamica, usize)>,
    filtro_busca: String,
    /// Filtros como estavam ao abrir a lista: o Cancelar volta para eles.
    filtros_antes: Option<Vec<Filtro>>,
    /// Campo com as configurações abertas embaixo das áreas.
    config_campo: Option<(AreaDinamica, usize)>,
    /// Itens do campo base de "% de"/"Diferença de" (lidos ao abrir).
    itens_base: Vec<String>,
    /// Início, fim e intervalo das faixas como digitados.
    faixa_txt: [String; 3],
}

#[derive(Debug, Clone)]
struct Arrasto {
    coluna: usize,
    origem: Option<(AreaDinamica, usize)>,
    inicio: Option<pelicano::Point>,
    pos: pelicano::Point,
    /// Só vira arrasto depois de andar uns pixels; antes disso é clique.
    movido: bool,
}

#[derive(Debug, Clone)]
pub(super) struct Estado {
    origem: Rect,
    origem_texto: String,
    recomendacoes: bool,
    folha_origem: usize,
    nomes: Vec<String>,
    linhas: Vec<usize>,
    colunas: Vec<usize>,
    valores: Vec<MedidaDinamica>,
    agrupar_data: Option<AgruparData>,
    nova_aba: bool,
    editando: Option<String>,
    calculados: Vec<Calculado>,
    novo_nome: String,
    nova_formula: String,
    previa_calculado: Option<Vec<f64>>,
    layout: Layout,
    repetir_rotulos: bool,
    subtotais: bool,
    tema: String,
    recolhidos: Vec<String>,
    grupos: Vec<String>,
    filtros: Vec<Filtro>,
    campo_filtro: Option<usize>,
    /// (chave que o filtro compara, rótulo formatado como na célula)
    valores_filtro: Vec<(String, String)>,
    filtro_cortado: bool,
    fase: Fase,
    avancado: bool,
    ui: PainelUi,
    /// "Mostrar valores como", faixas, ordem e totais gerais vindos do
    /// arquivo (`.xlsx`). Valem enquanto os campos forem os de `extras_campos`.
    extras: Extras,
    /// (linhas, colunas, colunas das medidas) para as quais `extras` foi lido.
    extras_campos: (Vec<usize>, Vec<usize>, Vec<usize>),
    /// Recalcula sozinha quando a aba de origem muda (opção, desligada).
    atualizar_sozinha: bool,
    /// Criada pelo botão Gráfico dinâmico: ao criar, o gráfico vem junto.
    com_grafico: bool,
    /// Segmentações de dados: (coluna do campo, itens escolhidos).
    segmentacoes: Vec<(usize, Vec<String>)>,
}

impl Estado {
    /// Há um campo sendo arrastado (a janela passa a escutar o mouse todo).
    pub(super) fn arrastando(&self) -> bool { self.ui.arrasto.is_some() }

    fn tirar_de(&mut self, area: AreaDinamica, i: usize) {
        // As posições mudam: as configurações abertas apontariam para outro campo.
        self.ui.config_campo = None;
        match area {
            AreaDinamica::Linhas if i < self.linhas.len() => { self.linhas.remove(i); }
            AreaDinamica::Colunas if i < self.colunas.len() => { self.colunas.remove(i); }
            AreaDinamica::Valores if i < self.valores.len() => { self.valores.remove(i); }
            AreaDinamica::Filtros if i < self.filtros.len() => { self.filtros.remove(i); }
            _ => {}
        }
    }

    /// Põe o campo na área, como o Excel: Linhas, Colunas e Filtros não
    /// repetem o mesmo campo entre si; Valores aceita qualquer campo.
    fn por_em(&mut self, col: usize, area: AreaDinamica, numerico: bool) {
        self.por_em_posicao(col, area, numerico, None);
    }

    /// [`Self::por_em`] numa posição da área (`None` = no fim).
    fn por_em_posicao(&mut self, col: usize, area: AreaDinamica, numerico: bool, posicao: Option<usize>) {
        let real = self.origem.c0 as usize + self.nomes.len().saturating_sub(self.calculados.len());
        let calculado = col >= real;
        match area {
            AreaDinamica::Valores => self.valores.push(MedidaDinamica {
                coluna: col, op: if numerico { AgregacaoOp::Soma } else { AgregacaoOp::Contagem },
            }),
            _ if calculado => {}
            AreaDinamica::Linhas | AreaDinamica::Colunas | AreaDinamica::Filtros => {
                self.linhas.retain(|&c| c != col);
                self.colunas.retain(|&c| c != col);
                if area != AreaDinamica::Filtros { self.filtros.retain(|f| f.coluna != col); }
                match area {
                    AreaDinamica::Linhas => self.linhas.push(col),
                    AreaDinamica::Colunas => self.colunas.push(col),
                    _ => if !self.filtros.iter().any(|f| f.coluna == col) {
                        // Sem valores escolhidos e no modo excluir: "(Tudo)".
                        self.filtros.push(Filtro { coluna: col, valores: Default::default(), excluir: true });
                    },
                }
            }
            AreaDinamica::Lista => {}
        }
        // Entrou no fim: leva para a posição pedida.
        if let Some(p) = posicao {
            fn levar<T>(v: &mut Vec<T>, p: usize) {
                if let Some(x) = v.pop() { v.insert(p.min(v.len()), x); }
            }
            match area {
                AreaDinamica::Linhas => levar(&mut self.linhas, p),
                AreaDinamica::Colunas => levar(&mut self.colunas, p),
                AreaDinamica::Valores => levar(&mut self.valores, p),
                AreaDinamica::Filtros if self.filtros.last().is_some_and(|f| f.coluna == col) => levar(&mut self.filtros, p),
                _ => {}
            }
        }
    }

    /// Leva o campo `de` da área para antes do campo `para` (`para` pode
    /// ser o tamanho da área: vai para o fim).
    fn reordenar_para(&mut self, area: AreaDinamica, de: usize, para: usize) {
        fn mover<T>(v: &mut Vec<T>, de: usize, para: usize) {
            if de >= v.len() || para == de || para == de + 1 { return; }
            let x = v.remove(de);
            v.insert(if para > de { para - 1 } else { para }.min(v.len()), x);
        }
        self.ui.config_campo = None;
        match area {
            AreaDinamica::Linhas => mover(&mut self.linhas, de, para),
            AreaDinamica::Colunas => mover(&mut self.colunas, de, para),
            AreaDinamica::Valores => mover(&mut self.valores, de, para),
            AreaDinamica::Filtros => mover(&mut self.filtros, de, para),
            AreaDinamica::Lista => {}
        }
    }

    /// Os filtros de relatório: campo na área Filtros (não em Linhas/Colunas,
    /// cujo filtro de itens também mora em `filtros`).
    fn filtros_de_relatorio(&self) -> Vec<&Filtro> {
        self.filtros.iter().filter(|f| !self.linhas.contains(&f.coluna) && !self.colunas.contains(&f.coluna)).collect()
    }

    fn coluna_em(&self, area: AreaDinamica, i: usize) -> Option<usize> {
        match area {
            AreaDinamica::Linhas => self.linhas.get(i).copied(),
            AreaDinamica::Colunas => self.colunas.get(i).copied(),
            AreaDinamica::Valores => self.valores.get(i).map(|v| v.coluna),
            AreaDinamica::Filtros => self.filtros.get(i).map(|f| f.coluna),
            AreaDinamica::Lista => None,
        }
    }
}

fn nome_operacao(op: AgregacaoOp) -> &'static str {
    match op {
        AgregacaoOp::Soma => tr!("Soma"),
        AgregacaoOp::Contagem => tr!("Contagem"),
        AgregacaoOp::Media => tr!("Média"),
        AgregacaoOp::Min => tr!("Mínimo"),
        AgregacaoOp::Max => tr!("Máximo"),
        AgregacaoOp::DesvioPadrao => tr!("Desvio padrão"),
        AgregacaoOp::Moda => tr!("Moda"),
    }
}

fn proxima_operacao(op: AgregacaoOp) -> AgregacaoOp {
    match op {
        AgregacaoOp::Soma => AgregacaoOp::Contagem,
        AgregacaoOp::Contagem => AgregacaoOp::Media,
        AgregacaoOp::Media => AgregacaoOp::Min,
        AgregacaoOp::Min => AgregacaoOp::Max,
        AgregacaoOp::Max => AgregacaoOp::DesvioPadrao,
        AgregacaoOp::DesvioPadrao => AgregacaoOp::Moda,
        AgregacaoOp::Moda => AgregacaoOp::Soma,
    }
}

fn proximo_agrupamento(modo: Option<AgruparData>) -> Option<AgruparData> {
    match modo {
        None => Some(AgruparData::Mes),
        Some(AgruparData::Mes) => Some(AgruparData::Trimestre),
        Some(AgruparData::Trimestre) => Some(AgruparData::Ano),
        Some(AgruparData::Ano) => None,
    }
}

fn rotulo_agrupamento(modo: Option<AgruparData>) -> &'static str {
    match modo {
        None => tr!("Sem agrupamento"),
        Some(AgruparData::Mes) => tr!("Mês"),
        Some(AgruparData::Trimestre) => tr!("Trimestre"),
        Some(AgruparData::Ano) => tr!("Ano"),
    }
}

fn marcar(campo: &mut Vec<usize>, coluna: usize) {
    if let Some(pos) = campo.iter().position(|&c| c == coluna) {
        campo.remove(pos);
    } else {
        campo.push(coluna);
        campo.sort_unstable();
    }
}

fn texto_celula(valor: &Value) -> String {
    match valor {
        // Rótulos da origem são dados, nunca fórmulas executáveis no destino.
        Value::Text(s) if s.starts_with('=') => format!("'{s}"),
        _ => to_text(valor),
    }
}

/// Lê um intervalo persistido sem admitir coordenadas fora da planilha.
fn retangulo_json(v: &serde_json::Value) -> Option<Rect> {
    let r0 = u32::try_from(v["r0"].as_u64()?).ok()?;
    let r1 = u32::try_from(v["r1"].as_u64()?).ok()?;
    let c0 = u32::try_from(v["c0"].as_u64()?).ok()?;
    let c1 = u32::try_from(v["c1"].as_u64()?).ok()?;
    (r0 <= r1 && c0 <= c1 && r1 < MAX_ROWS && c1 < MAX_COLS)
        .then(|| Rect::new(r0, c0, r1, c1))
}

/// Testes: os campos com que a dinâmica abria antes de nascer vazia (17/10)
/// — 1ª coluna em Linhas e a última numérica em Valores (soma).
#[cfg(test)]
pub(in crate::app) fn campos_de_antes(app: &mut App) {
    let Some(e) = app.dinamica.as_mut() else { return };
    let campos = campos::campos(e.origem.clone(), |r, c| app.workbook.valor(e.folha_origem, r, c));
    let medida = campos.iter().enumerate().rev().find(|(_, c)| c.numerico)
        .map_or(e.origem.c1 as usize, |(i, _)| e.origem.c0 as usize + i);
    e.linhas = vec![e.origem.c0 as usize];
    e.valores = vec![MedidaDinamica { coluna: medida, op: AgregacaoOp::Soma }];
}

/// Acima disto (linhas da origem) a conta espera a barra de status avisar.
const ORIGEM_GRANDE: u32 = 100_000;

impl App {
    /// O painel lateral "Campos da Tabela Dinâmica" está aberto (a janela
    /// de escolher a origem não conta: ela fica no meio da tela).
    pub(in crate::app) fn dinamica_lateral_aberta(&self) -> bool {
        self.dinamica.as_ref().is_some_and(|e| e.fase != Fase::Origem)
    }

    pub(super) fn evento_dinamica(&mut self, evento: Evento) -> Task<Msg> {
        let ao_vivo = Self::evento_atualiza_ao_vivo(&evento);
        let medir = ao_vivo || matches!(evento, Evento::Criar | Evento::NaFaixa(..));
        let nome = format!("{evento:?}");
        let nome = nome.split(['(', ' ', '{']).next().unwrap_or("").to_owned();
        let inicio = std::time::Instant::now();
        if let Evento::AtualizarAgora(vez) = evento {
            if self.dinamica.as_ref().is_some_and(|e| e.ui.vez_adiada == vez) {
                self.atualizar_ao_vivo();
                let ms = inicio.elapsed().as_millis();
                feathersheet_core::diario::anotar(trf!("[dinâmica] {nome}: {ms} ms no total"));
            }
            return Task::none();
        }
        let tarefa = self.evento_dinamica_base(evento);
        // Origem grande: primeiro a barra de status avisa (a tela redesenha),
        // depois a conta. Antes a janela parava sem dizer nada.
        let grande = self.dinamica.as_ref().is_some_and(|e| e.origem.r1 - e.origem.r0 > ORIGEM_GRANDE);
        if ao_vivo && grande {
            let Some(e) = self.dinamica.as_mut() else { return tarefa };
            e.ui.vez_adiada = e.ui.vez_adiada.wrapping_add(1);
            let vez = e.ui.vez_adiada;
            self.status = tr!("Atualizando a tabela dinâmica…").into();
            let adiada = Task::perform(tokio::time::sleep(std::time::Duration::from_millis(30)),
                move |_| Msg::Dinamica(Evento::AtualizarAgora(vez)));
            return Task::batch([tarefa, adiada]);
        }
        if ao_vivo { self.atualizar_ao_vivo(); }
        if medir {
            let ms = inicio.elapsed().as_millis();
            feathersheet_core::diario::anotar(trf!("[dinâmica] {nome}: {ms} ms no total"));
        }
        tarefa
    }

    fn evento_dinamica_base(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Abrir => {
                self.confirmar();
                let Some(aba) = self.workbook.aba(self.folha) else {
                    self.status = tr!("Não foi possível localizar a aba de origem.").into();
                    return Task::none();
                };
                let selecionada = self.area_selecionada();
                let origem = if selecionada.area() == 1 {
                    ops::regiao_atual(&aba.sheet, selecionada)
                } else {
                    selecionada.normalized()
                };
                if origem.r1 <= origem.r0 {
                    self.status = tr!("Selecione uma tabela com cabeçalho e ao menos uma linha de dados.").into();
                    return Task::none();
                }
                let campos = campos::campos(origem.clone(), |r, c| self.workbook.valor(self.folha, r, c));
                if campos.is_empty() {
                    self.status = tr!("Não foram encontrados campos na origem.").into();
                    return Task::none();
                }
                self.dinamica = Some(Estado {
                    origem: origem.clone(),
                    origem_texto: endereco_area(&origem),
                    recomendacoes: false,
                    folha_origem: self.folha,
                    nomes: campos.into_iter().map(|c| c.nome).collect(),
                    // Vazia, como no Excel: a origem só é lida quando o
                    // primeiro campo entra (com o ID em Linhas, criar
                    // montava 1 milhão de linhas e a janela travava).
                    linhas: Vec::new(),
                    colunas: Vec::new(),
                    valores: Vec::new(),
                    agrupar_data: None,
                    nova_aba: true,
                    editando: None,
                    calculados: Vec::new(),
                    novo_nome: String::new(),
                    nova_formula: String::new(),
                    previa_calculado: None,
                    layout: Layout::Tabular,
                    repetir_rotulos: true,
                    subtotais: false,
                    tema: "padrao".into(),
                    recolhidos: Vec::new(),
                    grupos: Vec::new(),
                    filtros: Vec::new(),
                    campo_filtro: None,
                    valores_filtro: Vec::new(),
                    filtro_cortado: false,
                    fase: Fase::Origem,
                    avancado: false,
                    ui: PainelUi::default(),
                    extras: Extras::default(),
                    extras_campos: Default::default(),
                    atualizar_sozinha: false,
                    com_grafico: false,
                    segmentacoes: Vec::new(),
                });
                self.menu_aberto = false;
                self.contexto = None;
            }
            Evento::Recomendadas => {
                let tarefa = self.evento_dinamica_base(Evento::Abrir);
                if let Some(e) = &mut self.dinamica { e.recomendacoes = true; }
                return tarefa;
            }
            Evento::Preset(id) => self.preset_dinamica(id),
            Evento::Mover(eixo, indice, delta) => {
                if let Some(estado) = &mut self.dinamica {
                    let novo = (indice as i64) + (delta as i64);
                    if novo >= 0 {
                        let destino = novo as usize;
                        match eixo {
                            EixoDinamica::Linhas if indice < estado.linhas.len() && destino < estado.linhas.len() =>
                                estado.linhas.swap(indice,destino),
                            EixoDinamica::Colunas if indice < estado.colunas.len() && destino < estado.colunas.len() =>
                                estado.colunas.swap(indice,destino),
                            EixoDinamica::Valores if indice < estado.valores.len() && destino < estado.valores.len() =>
                                estado.valores.swap(indice,destino),
                            _ => {}
                        }
                    }
                }
            },
            Evento::DuplicarValor(i) => if let Some(e) = &mut self.dinamica {
                if let Some(mut medida) = e.valores.get(i).cloned() {
                    medida.op = proxima_operacao(medida.op);
                    e.valores.insert(i+1,medida);
                }
            },
            Evento::RemoverValor(i) => if let Some(e) = &mut self.dinamica {
                if i < e.valores.len() { e.valores.remove(i); }
            },
            Evento::OperacaoValor(i) => if let Some(e) = &mut self.dinamica {
                if let Some(m) = e.valores.get_mut(i) { m.op = proxima_operacao(m.op); }
            },
            Evento::OrigemTexto(texto) => if let Some(e) = &mut self.dinamica {
                e.origem_texto = texto;
            },
            Evento::OrigemAplicar => self.aplicar_nova_origem_dinamica(),
            Evento::Fechar => {
                self.dinamica = None;
                self.dinamica_fundo.soltar();
            }
            Evento::FundoPasso => return self.passo_dinamica_fundo(),
            Evento::FundoPronto(geracao, pronto) => self.dinamica_fundo_pronta(geracao, pronto),
            Evento::Linha(i) => {
                if let Some(e) = &mut self.dinamica {
                    if i < e.nomes.len() { marcar(&mut e.linhas, e.origem.c0 as usize + i); }
                }
            }
            Evento::Coluna(i) => {
                if let Some(e) = &mut self.dinamica {
                    if i < e.nomes.len() { marcar(&mut e.colunas, e.origem.c0 as usize + i); }
                }
            }
            Evento::Valor(i) => {
                if let Some(e) = &mut self.dinamica {
                    if i < e.nomes.len() {
                        let col = e.origem.c0 as usize + i;
                        if let Some(pos) = e.valores.iter().position(|v| v.coluna == col) {
                            e.valores.remove(pos);
                        } else {
                            e.valores.push(MedidaDinamica { coluna: col, op: AgregacaoOp::Soma });
                            e.valores.sort_by_key(|v| v.coluna);
                        }
                    }
                }
            }
            Evento::Operacao(i) => {
                if let Some(e) = &mut self.dinamica {
                    let col = e.origem.c0 as usize + i;
                    if let Some(v) = e.valores.iter_mut().find(|v| v.coluna == col) {
                        v.op = proxima_operacao(v.op);
                    }
                }
            }
            Evento::Agrupar => {
                if let Some(e) = &mut self.dinamica {
                    e.agrupar_data = proximo_agrupamento(e.agrupar_data);
                }
            }
            Evento::Destino => {
                if let Some(e) = &mut self.dinamica { e.nova_aba = !e.nova_aba; }
            }
            Evento::Criar => {
                let primeiro_passo = self.dinamica.as_ref()
                    .is_some_and(|e| e.fase == Fase::Origem && e.editando.is_none());
                if primeiro_passo && !self.aplicar_texto_antes_de_criar() { return Task::none(); }
                let com_grafico = self.dinamica.as_ref().is_some_and(|e| e.com_grafico);
                self.criar_dinamica_nativa();
                let criada = self.dinamica.is_none();
                // Criada: abre o painel de campos sobre a dinâmica recém-criada.
                if primeiro_passo && criada { self.abrir_edicao_dinamica(); }
                // Pelo botão Gráfico dinâmico: o gráfico vem junto.
                if com_grafico && criada { return self.inserir_grafico_dinamico(); }
            }
            Evento::DestinoNova(nova) => if let Some(e) = &mut self.dinamica { e.nova_aba = nova; },
            Evento::AlternarAvancado => if let Some(e) = &mut self.dinamica { e.avancado = !e.avancado; },
            Evento::Remover(eixo, i) => if let Some(e) = &mut self.dinamica {
                e.ui.config_campo = None;
                match eixo {
                    EixoDinamica::Linhas if i < e.linhas.len() => { e.linhas.remove(i); }
                    EixoDinamica::Colunas if i < e.colunas.len() => { e.colunas.remove(i); }
                    EixoDinamica::Valores if i < e.valores.len() => { e.valores.remove(i); }
                    _ => {}
                }
            },
            Evento::RemoverFiltro(col) => if let Some(e) = &mut self.dinamica {
                e.filtros.retain(|f| f.coluna != col);
            },
            Evento::Campo(i) => {
                let numerico = self.dinamica.as_ref().map(|e| {
                    let real = e.nomes.len().saturating_sub(e.calculados.len());
                    if i >= real { return true; }
                    campos::campos(e.origem.clone(), |r,c| self.workbook.valor(e.folha_origem,r,c))
                        .get(i).is_some_and(|c| c.numerico)
                }).unwrap_or(false);
                if let Some(e) = &mut self.dinamica {
                    if i < e.nomes.len() {
                        let col = e.origem.c0 as usize + i;
                        let presente = e.linhas.contains(&col) || e.colunas.contains(&col)
                            || e.valores.iter().any(|v| v.coluna == col);
                        if presente {
                            e.linhas.retain(|&c| c != col);
                            e.colunas.retain(|&c| c != col);
                            e.valores.retain(|v| v.coluna != col);
                        } else if numerico {
                            e.valores.push(MedidaDinamica { coluna: col, op: AgregacaoOp::Soma });
                        } else {
                            e.linhas.push(col);
                        }
                    }
                }
            }
            Evento::Editar => self.abrir_edicao_dinamica(),
            Evento::Layout => if let Some(e) = &mut self.dinamica {
                e.layout = if e.layout == Layout::Compacto { Layout::Tabular } else { Layout::Compacto };
            },
            Evento::RepetirRotulos => if let Some(e) = &mut self.dinamica {
                e.repetir_rotulos = !e.repetir_rotulos;
            },
            Evento::Subtotais => if let Some(e) = &mut self.dinamica {
                e.subtotais = !e.subtotais;
            },
            Evento::Tema => if let Some(e) = &mut self.dinamica {
                const TEMAS: &[&str] = &["padrao","azul","verde","laranja","vermelho","roxo","cinza"];
                let indice = TEMAS.iter().position(|&t| t == e.tema).unwrap_or(0);
                e.tema = TEMAS[(indice+1)%TEMAS.len()].to_owned();
            },
            Evento::FiltroCampo(i) => {
                let alvo = self.dinamica.as_ref()
                    .filter(|e| i < e.nomes.len().saturating_sub(e.calculados.len()))
                    .map(|e| e.origem.c0 as usize + i);
                if let Some(col) = alvo {
                    let (amostra,cortado) = self.dinamica.as_ref()
                        .map(|e| avancado::opcoes_de_filtro(self, e, col))
                        .unwrap_or_default();
                    if let Some(e) = &mut self.dinamica {
                        e.campo_filtro = Some(col);
                        e.valores_filtro = amostra;
                        e.filtro_cortado = cortado;
                        e.ui.filtros_antes = Some(e.filtros.clone());
                        e.ui.filtro_busca.clear();
                        e.ui.menu_item = None;
                    }
                }
            }
            Evento::FiltroValor(valor) => if let Some(e) = &mut self.dinamica {
                if let Some(col) = e.campo_filtro {
                    if let Some(filtro) = e.filtros.iter_mut().find(|f| f.coluna == col) {
                        if !filtro.valores.remove(&valor) { filtro.valores.insert(valor); }
                    } else {
                        e.filtros.push(Filtro {
                            coluna: col,
                            valores: [valor].into_iter().collect(),
                            excluir: true,
                        });
                    }
                }
            },
            Evento::FiltroModo => if let Some(e) = &mut self.dinamica {
                if let Some(col) = e.campo_filtro {
                    // Com amostra cortada, só se pode EXCLUIR valores vistos.
                    // Inclusão perderia os valores que não apareceram no menu.
                    let modo_atual_excluir = e.filtros.iter().find(|f| f.coluna == col)
                        .map(|f| f.excluir).unwrap_or(true);
                    let novo_excluir = !modo_atual_excluir;
                    if e.filtro_cortado && !novo_excluir {
                        self.status = tr!("A lista foi limitada. Para não ocultar dados não vistos, use Excluir escolhidos.").into();
                    } else if let Some(filtro) = e.filtros.iter_mut().find(|f| f.coluna == col) {
                        filtro.excluir = novo_excluir;
                    } else {
                        e.filtros.push(Filtro {
                            coluna: col,
                            valores: std::collections::HashSet::new(),
                            excluir: novo_excluir,
                        });
                    }
                }
            },
            Evento::LimparFiltro => if let Some(e) = &mut self.dinamica {
                if let Some(col) = e.campo_filtro { e.filtros.retain(|f| f.coluna != col); }
            },
            Evento::NomeCalculado(nome) => if let Some(e) = &mut self.dinamica {
                e.novo_nome = nome;
            },
            Evento::FormulaCalculado(formula) => if let Some(e) = &mut self.dinamica {
                e.nova_formula = formula;
                e.previa_calculado = None;
            },
            Evento::PreviaCalculado => {
                let resultado = self.dinamica.as_ref().map(|e| {
                    campos::previa(e.origem.clone(), &e.nova_formula, 3,
                        |r,c| self.workbook.valor(e.folha_origem,r,c))
                });
                if let Some(resultado) = resultado {
                    match resultado {
                        Ok(numeros) => if let Some(e) = &mut self.dinamica {
                            e.previa_calculado = Some(numeros);
                        },
                        Err(erro) => {
                            if let Some(e) = &mut self.dinamica { e.previa_calculado = None; }
                            self.status = trf!("Prévia inválida: {erro}");
                        }
                    }
                }
            },
            Evento::AdicionarCalculado => self.adicionar_campo_calculado(),
            Evento::RemoverCalculado(i) => self.remover_campo_calculado(i),
            Evento::Recolher(caminho) => if let Some(e) = &mut self.dinamica {
                if let Some(i) = e.recolhidos.iter().position(|p| p == &caminho) {
                    e.recolhidos.remove(i);
                } else { e.recolhidos.push(caminho); }
            },
            Evento::RecolherTodos(recolher) => if let Some(e) = &mut self.dinamica {
                e.recolhidos = if recolher { e.grupos.clone() } else { Vec::new() };
            },
            Evento::RecolherNaGrade(linha,coluna,caminho) =>
                self.alternar_grupo_no_resultado(linha,coluna,caminho),
            Evento::Atualizar => self.atualizar_dinamica_nativa(),
            Evento::ArrastoInicio(coluna, origem) => if let Some(e) = &mut self.dinamica {
                e.ui.menu_item = None;
                e.ui.arrasto = Some(Arrasto {
                    coluna, origem, inicio: None, pos: pelicano::Point::ORIGIN, movido: false,
                });
            },
            Evento::ArrastoMover(p) => if let Some(a) = self.dinamica.as_mut().and_then(|e| e.ui.arrasto.as_mut()) {
                let inicio = *a.inicio.get_or_insert(p);
                a.pos = p;
                if !a.movido && ((p.x - inicio.x).abs() + (p.y - inicio.y).abs()) > 5.0 { a.movido = true; }
            },
            Evento::ArrastoSobre(area) => if let Some(e) = &mut self.dinamica { e.ui.sob = area; },
            Evento::ArrastoSobreItem(item) => if let Some(e) = &mut self.dinamica {
                match item {
                    Some(item) => { e.ui.sob_item = Some(item); e.ui.sob = Some(item.0); }
                    None => e.ui.sob_item = None,
                }
            },
            Evento::ArrastoSair(area) => if let Some(e) = &mut self.dinamica {
                if e.ui.sob == Some(area) { e.ui.sob = None; }
            },
            Evento::ArrastoSoltar => {
                let Some(a) = self.dinamica.as_mut().and_then(|e| e.ui.arrasto.take()) else {
                    return Task::none();
                };
                let numerico = self.campo_numerico(a.coluna);
                let Some(e) = &mut self.dinamica else { return Task::none() };
                if !a.movido {
                    match a.origem {
                        // Clique sem arrastar no nome de um campo da lista = marcar/desmarcar.
                        None => {
                            let i = a.coluna - e.origem.c0 as usize;
                            return self.evento_dinamica_base(Evento::Campo(i));
                        }
                        // Clique num campo de Filtros: a lista de itens.
                        Some((AreaDinamica::Filtros, _)) => {
                            let i = a.coluna.saturating_sub(e.origem.c0 as usize);
                            return self.evento_dinamica_base(Evento::FiltroCampo(i));
                        }
                        // Clique num campo das outras áreas: as configurações
                        // dele (Resumir por, Mostrar valores como, classificar),
                        // como no Excel. Clicar de novo fecha.
                        Some(item) => {
                            let alvo = (e.ui.config_campo != Some(item)).then_some(item);
                            return self.evento_dinamica_base(Evento::ConfigurarCampo(alvo));
                        }
                    }
                } else {
                    // Sobre um campo da área de destino: entra antes dele.
                    let posicao = e.ui.sob_item.filter(|(area, _)| Some(*area) == e.ui.sob).map(|(_, j)| j);
                    match (a.origem, e.ui.sob) {
                        // Mesma área: reordena (soltar no vazio leva para o fim).
                        (Some((de, i)), Some(para)) if de == para => {
                            let fim = match para {
                                AreaDinamica::Linhas => e.linhas.len(),
                                AreaDinamica::Colunas => e.colunas.len(),
                                AreaDinamica::Valores => e.valores.len(),
                                AreaDinamica::Filtros => e.filtros.len(),
                                AreaDinamica::Lista => 0,
                            };
                            e.reordenar_para(para, i, posicao.unwrap_or(fim));
                        }
                        (origem, Some(para)) if para != AreaDinamica::Lista => {
                            if let Some((de, i)) = origem { e.tirar_de(de, i); }
                            e.por_em_posicao(a.coluna, para, numerico, posicao);
                        }
                        // Fora das áreas (ou de volta na lista): sai, como no Excel.
                        (Some((de, i)), _) => e.tirar_de(de, i),
                        (None, _) => {}
                    }
                }
                e.ui.sob = None;
                e.ui.sob_item = None;
            }
            Evento::BuscaCampo(t) => if let Some(e) = &mut self.dinamica { e.ui.busca_campo = t; },
            Evento::MenuItem(item) => if let Some(e) = &mut self.dinamica {
                e.ui.menu_item = if e.ui.menu_item == item { None } else { item };
            },
            Evento::MoverPara(de, i, para) => {
                let col = self.dinamica.as_ref().and_then(|e| e.coluna_em(de, i));
                if let Some(col) = col {
                    let numerico = self.campo_numerico(col);
                    if let Some(e) = &mut self.dinamica {
                        e.tirar_de(de, i);
                        e.por_em(col, para, numerico);
                        e.ui.menu_item = None;
                    }
                }
            }
            Evento::Reordenar(area, i, delta) => if let Some(e) = &mut self.dinamica {
                e.ui.config_campo = None;
                let j = i as i64 + delta as i64;
                if j >= 0 {
                    let j = j as usize;
                    match area {
                        AreaDinamica::Linhas if j < e.linhas.len() => e.linhas.swap(i, j),
                        AreaDinamica::Colunas if j < e.colunas.len() => e.colunas.swap(i, j),
                        AreaDinamica::Valores if j < e.valores.len() => e.valores.swap(i, j),
                        AreaDinamica::Filtros if j < e.filtros.len() => e.filtros.swap(i, j),
                        _ => {}
                    }
                    e.ui.menu_item = None;
                }
            },
            Evento::FiltroBusca(t) => if let Some(e) = &mut self.dinamica { e.ui.filtro_busca = t; },
            Evento::FiltroTudo => if let Some(e) = &mut self.dinamica {
                if let Some(col) = e.campo_filtro {
                    let todos: std::collections::HashSet<String> =
                        e.valores_filtro.iter().map(|(chave, _)| chave.clone()).collect();
                    let filtro = e.filtros.iter_mut().find(|f| f.coluna == col);
                    let tudo_visivel = filtro.as_ref().is_none_or(|f| f.excluir && f.valores.is_empty());
                    match filtro {
                        // Tudo marcado → desmarca tudo (só no modo excluir, para não
                        // esconder o que a amostra não mostrou).
                        Some(f) if tudo_visivel => { f.excluir = true; f.valores = todos; }
                        Some(f) => { f.excluir = true; f.valores.clear(); }
                        None => e.filtros.push(Filtro { coluna: col, valores: todos, excluir: true }),
                    }
                }
            },
            Evento::FiltroOk => if let Some(e) = &mut self.dinamica {
                e.campo_filtro = None;
                e.ui.filtros_antes = None;
            },
            Evento::FiltroCancelar => if let Some(e) = &mut self.dinamica {
                if let Some(antes) = e.ui.filtros_antes.take() { e.filtros = antes; }
                e.campo_filtro = None;
            },
            Evento::LayoutEscolher(tabular) => if let Some(e) = &mut self.dinamica {
                e.layout = if tabular { Layout::Tabular } else { Layout::Compacto };
            },
            Evento::AgruparEscolher(modo) => if let Some(e) = &mut self.dinamica { e.agrupar_data = modo; },
            Evento::TemaEscolher(tema) => if let Some(e) = &mut self.dinamica { e.tema = tema.to_owned(); },
            evento @ (Evento::ConfigurarCampo(_) | Evento::OperacaoEscolher(..) | Evento::MostrarComo(..)
                | Evento::Ordenar(..) | Evento::AgruparCampo(..) | Evento::FaixaTexto(..)
                | Evento::TotaisGerais { .. }) => self.evento_config_campo(evento),
            Evento::AtualizarSozinha(v) => if let Some(e) = &mut self.dinamica { e.atualizar_sozinha = v; },
            Evento::GraficoDinamico => return self.inserir_grafico_dinamico(),
            Evento::NaFaixa(evento) => return self.evento_na_faixa(*evento),
            // Tratada em `evento_dinamica` (antes de chegar aqui).
            Evento::AtualizarAgora(_) => {}
            Evento::FiltroNaGrade(r, c) => self.abrir_filtro_na_grade(r, c),
            evento @ (Evento::SegmentacaoInserir | Evento::SegmentacaoCampo(_) | Evento::SegmentacaoFecharEscolha
                | Evento::SegmentacaoItem(..) | Evento::SegmentacaoLimpar(..) | Evento::SegmentacaoRemover(..)) =>
                self.evento_segmentacao(evento),
        }
        Task::none()
    }

    /// Campo numérico vai para Valores como Soma; texto, como Contagem.
    fn campo_numerico(&self, col: usize) -> bool {
        let Some(e) = &self.dinamica else { return false };
        let i = col.saturating_sub(e.origem.c0 as usize);
        let real = e.nomes.len().saturating_sub(e.calculados.len());
        if i >= real { return true; }
        campos::campos(e.origem.clone(), |r, c| self.workbook.valor(e.folha_origem, r, c))
            .get(i).is_some_and(|c| c.numerico)
    }

    fn criar_dinamica_nativa(&mut self) {
        let Some(e) = self.dinamica.clone() else { return; };
        if let Err(erro) = conferir_nomes_calculados(&e) {
            self.status = erro;
            return;
        }
        if let Some(id) = e.editando.as_deref() {
            self.salvar_edicao_dinamica(&e, id);
            return;
        }
        let largura = e.origem.c1 - e.origem.c0 + 1;
        // Valores lidos sob demanda; layout, filtros e calculados executados pelo core.
        let resultado = match calcular(self, &e) {
            Ok(resultado) => resultado,
            Err(erro) => { self.status = erro; return; }
        };
        let matriz = &resultado.matriz;
        let altura = matriz.height();
        let colunas = matriz.rows().iter().map(Vec::len).max().unwrap_or(0);
        if altura == 0 || colunas == 0 {
            self.status = tr!("A tabela dinâmica não gerou resultados.").into();
            return;
        }
        if altura > MAX_ROWS as usize || colunas > MAX_COLS as usize {
            self.status = tr!("O resultado ultrapassa o limite de linhas ou colunas da planilha.").into();
            return;
        }
        let inicio_col = if e.nova_aba { 0 } else {
            match e.origem.c1.checked_add(2) {
                Some(c) => c,
                None => {
                    self.status = tr!("Não existe espaço para criar a dinâmica à direita da origem.").into();
                    return;
                }
            }
        };
        if (inicio_col as usize) + colunas > MAX_COLS as usize {
            self.status = tr!("Não existe espaço suficiente para a dinâmica nesta aba. Selecione Nova aba.").into();
            return;
        }
        if !e.nova_aba {
            let Some(aba) = self.workbook.aba(e.folha_origem) else { return };
            // Nunca substituir dados existentes, inclusive fora da seleção.
            for (r, linha) in matriz.rows().iter().enumerate() {
                for c in 0..linha.len() {
                    if !aba.sheet.get_raw(r as u32, inicio_col + c as u32).is_empty() {
                        self.status = tr!("O destino contém dados. Use Nova aba para não sobrescrever células.").into();
                        return;
                    }
                }
            }
        }
        // Preservar metadados existentes; uma lista inválida nunca é substituída silenciosamente.
        let mut existentes: Vec<serde_json::Value> = if e.nova_aba {
            Vec::new()
        } else {
            match self.workbook.aba(e.folha_origem).and_then(|a| a.sheet.dn.as_deref()) {
                None => Vec::new(),
                Some(raw) => match serde_json::from_str(raw) {
                    Ok(v) => v,
                    Err(_) => {
                        self.status = tr!("As definições das dinâmicas existentes estão inválidas. Nada foi alterado.").into();
                        return;
                    }
                }
            }
        };
        // Metadados de outras dinâmicas também reservam células vazias.
        // Verificar só get_raw não impediria escrever em lacunas do resultado.
        if !e.nova_aba {
            let novo = Rect::new(0, inicio_col, (altura - 1) as u32,
                inicio_col + colunas as u32 - 1);
            if existentes.iter().any(|d| d.get("areaResultado")
                .and_then(retangulo_json)
                .is_some_and(|antigo| antigo.r0 <= novo.r1 && novo.r0 <= antigo.r1 &&
                    antigo.c0 <= novo.c1 && novo.c0 <= antigo.c1)) {
                self.status = tr!("O local reservado cruza outra tabela dinâmica. Use Nova aba.").into();
                return;
            }
        }
        let origem_nome = self.workbook.aba(e.folha_origem)
            .map(|a| a.sheet.name.clone()).unwrap_or_default();
        let indice = if e.nova_aba {
            let nomes = self.workbook.nomes();
            let nome = (1..).map(|n| format!("Dinâmica{n}"))
                .find(|n| !nomes.iter().any(|x| x.eq_ignore_ascii_case(n)))
                .expect("um nome livre sempre existe");
            self.workbook.criar_aba(nome)
        } else { e.folha_origem };

        let id = (1..).map(|n| format!("dinamica_nativa_{indice}_{n}"))
            .find(|candidato| !existentes.iter().any(|v| v["id"].as_str() == Some(candidato.as_str())))
            .expect("sempre existe identificador de dinâmica livre");
        let mut meta = json!({
            "id": id,
            "fonte": {"aba": origem_nome,
                "area": {"r0":e.origem.r0,"r1":e.origem.r1,"c0":e.origem.c0,"c1":e.origem.c1}},
            "destino": {"row":0,"col":inicio_col},
            "linhas": [], "colunas": [], "valores": [], "datas": [],
            "areaResultado": {"r0":0,"c0":inicio_col,
                "r1":(altura-1) as u32,"c1":inicio_col+colunas as u32-1},
        });
        avancado::gravar_campos(&mut meta, &e);
        gravar_opcoes(&mut meta, &e, mapa_json(&resultado.linhas, resultado.colunas_de_rotulo));
        existentes.push(meta);
        let metadados = serde_json::to_string(&existentes)
            .expect("metadados JSON em memória são serializáveis");
        let extras = avancado::extras_vigentes(&e);
        // Escrever em uma transação, preservando o histórico de edição.
        self.workbook.editar(indice, |sheet| {
            sheet.abrir_transacao();
            // Compactar o histórico por lotes e, se necessário, descarregá-lo
            // em disco. Um resultado de 1M de linhas não pode segurar milhões
            // de snapshots de Ctrl+Z descompactados na RAM.
            let mut escritas = 0usize;
            for (r, linha) in matriz.rows().iter().enumerate() {
                for (c, valor) in linha.iter().enumerate() {
                    sheet.set_cell_divisao(r as u32, inicio_col + c as u32, &texto_celula(valor));
                    escritas += 1;
                    if escritas % 16_384 == 0 { sheet.compactar_lote_divisao(); }
                }
            }
            sheet.compactar_lote_divisao();
            config_campo::formatar_valores(sheet,
                &Rect::new(0, inicio_col, (altura - 1) as u32, inicio_col + colunas as u32 - 1),
                &resultado, &extras, e.valores.len());
            // Metadados e células compõem uma única operação de Ctrl+Z.
            sheet.alterar_dinamicas(Some(metadados),
                &Rect::new(0,inicio_col,(altura-1) as u32,inicio_col+colunas as u32-1));
            sheet.fechar_transacao();
        });
        avancado::guardar_caminhos(self, &e, &resultado);
        self.dinamica = None;
        self.folha = indice;
        self.linha_inicio = 0;
        self.coluna_inicio = inicio_col;
        self.linha = 0;
        self.coluna = inicio_col;
        self.fim_selecao = ((altura - 1) as u32, inicio_col + colunas as u32 - 1);
        self.endereco = cell_name(0, inicio_col);
        self.faixa_ativa = AbaFaixa::Inserir;
        self.alterado = true;
        self.status = trf!("Tabela dinâmica criada: {} linhas × {} colunas ({largura} campos na origem).", altura, colunas);
    }

    /// Recalcula a definição persistida com o mesmo caminho da edição nativa.
    fn atualizar_dinamica_nativa(&mut self) {
        self.atualizar_dinamica_existente();
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn criar_nasce_vazia_com_o_aviso_como_no_excel() {
        let mut app = App::default();
        app.workbook.editar(0, |s| {
            s.set_cell(0,0,"ID"); s.set_cell(0,1,"Vendas");
            s.set_cell(1,0,"1"); s.set_cell(1,1,"10");
            s.set_cell(2,0,"2"); s.set_cell(2,1,"20");
        });
        app.linha=0; app.coluna=0; app.fim_selecao=(2,1);
        app.evento_dinamica(Evento::Abrir);
        app.evento_dinamica(Evento::Criar);
        let e = app.dinamica.as_ref().expect("painel de campos aberto");
        assert!(e.linhas.is_empty() && e.valores.is_empty(), "nenhum campo escolhido sozinho");
        let aviso = app.workbook.aba(app.folha).unwrap().sheet.get_raw(0, 0);
        assert!(aviso.contains("escolha os campos"), "{aviso}");
    }

    #[test]
    fn monta_e_grava_dinamica_em_nova_aba_sem_apagar_origem() {
        let mut app = App::default();
        app.workbook.editar(0, |s| {
            s.set_cell(0,0,"Região"); s.set_cell(0,1,"Vendas");
            s.set_cell(1,0,"Sul"); s.set_cell(1,1,"10");
            s.set_cell(2,0,"Norte"); s.set_cell(2,1,"20");
        });
        app.linha=0; app.coluna=0; app.fim_selecao=(2,1);
        app.evento_dinamica(Evento::Abrir);
        crate::app::dinamica_ui::campos_de_antes(&mut app);
        app.evento_dinamica(Evento::Criar);
        assert_eq!(app.workbook.len(), 2);
        assert_eq!(app.workbook.valor(0,1,1), Value::Number(10.));
        assert_eq!(app.workbook.valor(1,0,0), Value::Text("Região".into()));
        assert_eq!(app.workbook.valor(1,1,1), Value::Number(20.));
        assert!(app.workbook.aba(1).unwrap().sheet.dn.is_some());
    }

    #[test]
    fn atualizacao_recalcula_resultado_sem_afetar_a_origem() {
        let mut app = App::default();
        app.workbook.editar(0, |s| {
            s.set_cell(0,0,"Região"); s.set_cell(0,1,"Valor");
            s.set_cell(1,0,"Sul"); s.set_cell(1,1,"10");
            s.set_cell(2,0,"Norte"); s.set_cell(2,1,"20");
        });
        app.fim_selecao = (2,1);
        app.evento_dinamica(Evento::Abrir);
        crate::app::dinamica_ui::campos_de_antes(&mut app);
        app.evento_dinamica(Evento::Criar);
        app.workbook.editar(0, |s| s.set_cell(1,1,"30"));
        app.evento_dinamica(Evento::Atualizar);
        assert_eq!(app.workbook.valor(1, 2, 1), Value::Number(30.));
        assert_eq!(app.workbook.valor(0, 1, 1), Value::Number(30.));
    }

    #[test]
    fn refresh_nao_apaga_dados_ao_expandir_resultado() {
        let mut app = App::default();
        app.workbook.editar(0, |sheet| {
            sheet.set_cell(0, 0, "Região"); sheet.set_cell(0, 1, "Valor");
            sheet.set_cell(1, 0, "Sul"); sheet.set_cell(1, 1, "10");
            sheet.set_cell(2, 0, "Sul"); sheet.set_cell(2, 1, "20");
        });
        app.fim_selecao = (2, 1);
        app.evento_dinamica(Evento::Abrir);
        crate::app::dinamica_ui::campos_de_antes(&mut app);
        app.evento_dinamica(Evento::Criar);
        assert_eq!(app.workbook.valor(1, 1, 1), Value::Number(30.));
        app.workbook.editar(1, |sheet| sheet.set_cell(3, 0, "Dado que deve ser preservado"));
        app.workbook.editar(0, |sheet| sheet.set_cell(2, 0, "Norte"));
        app.evento_dinamica(Evento::Atualizar);
        assert!(app.status.contains("expansão"));
        assert_eq!(app.workbook.valor(1, 3, 0), Value::Text("Dado que deve ser preservado".into()));
        assert_eq!(app.workbook.valor(1, 1, 1), Value::Number(30.));
    }

    #[test]
    fn atualizar_sem_selecionar_a_dinamica_nao_altera_planilha() {
        let mut app = App::default();
        app.evento_dinamica(Evento::Atualizar);
        assert!(app.status.contains("não possui"));
        assert_eq!(app.workbook.len(), 1);
    }

    #[test]
    fn criacao_nao_pode_ocupar_area_reservada_por_outra_dinamica() {
        let mut app = App::default();
        app.workbook.editar(0,|s|{
            s.set_cell(0,0,"Setor");s.set_cell(0,1,"Vendas");
            s.set_cell(1,0,"X");s.set_cell(1,1,"10");
            // Em D1:E3 a tabela anterior reservou células ainda vazias.
            s.dn=Some(serde_json::json!([{"id":"outra","areaResultado":{
                "r0":0,"c0":3,"r1":2,"c1":4
            }}]).to_string());
        });
        app.fim_selecao=(1,1);
        app.evento_dinamica(Evento::Abrir);
        crate::app::dinamica_ui::campos_de_antes(&mut app);
        app.evento_dinamica(Evento::Destino);
        app.evento_dinamica(Evento::Criar);
        assert_eq!(app.workbook.len(),1);
        assert!(app.status.contains("outra tabela dinâmica"));
        assert!(app.dinamica.is_some());
    }

    #[test]
    fn filtro_cortado_nao_permite_inclusao_de_valores_incompletos() {
        let mut app = App::default();
        app.workbook.editar(0,|s|{
            s.set_cell(0,0,"Categoria"); s.set_cell(0,1,"Valor");
            s.set_cell(1,0,"A"); s.set_cell(1,1,"20");
        });
        app.fim_selecao=(1,1);
        app.evento_dinamica(Evento::Abrir);
        crate::app::dinamica_ui::campos_de_antes(&mut app);
        let e = app.dinamica.as_mut().unwrap();
        e.campo_filtro = Some(0);
        e.filtro_cortado = true;
        app.evento_dinamica(Evento::FiltroModo);
        assert!(app.dinamica.as_ref().unwrap().filtros.is_empty());
        assert!(app.status.contains("limitada"));
    }

    #[test]
    fn destino_na_aba_atual_nao_sobrescreve_celula_ocupada() {
        let mut app = App::default();
        app.workbook.editar(0, |s| {
            s.set_cell(0,0,"Região"); s.set_cell(0,1,"Vendas");
            s.set_cell(1,0,"Sul"); s.set_cell(1,1,"10");
            s.set_cell(0,3,"Ocupada");
        });
        app.fim_selecao=(1,1);
        app.evento_dinamica(Evento::Abrir);
        crate::app::dinamica_ui::campos_de_antes(&mut app);
        app.evento_dinamica(Evento::Destino);
        app.evento_dinamica(Evento::Criar);
        assert_eq!(app.workbook.len(), 1);
        assert_eq!(app.workbook.valor(0,0,3), Value::Text("Ocupada".into()));
        assert!(app.dinamica.is_some());
    }

    #[test]
    fn selecao_sem_dados_recusada_e_modo_agrupamento_percorre_opcoes() {
        let mut app=App::default();
        app.evento_dinamica(Evento::Abrir);
        crate::app::dinamica_ui::campos_de_antes(&mut app);
        assert!(app.dinamica.is_none());
        assert!(app.status.contains("Selecione"));
        assert_eq!(proximo_agrupamento(Some(AgruparData::Ano)),None);
    }

    #[test]
    fn fluxo_novo_origem_ok_abre_painel_e_campos_atualizam_a_grade() {
        let mut app = App::default();
        app.workbook.editar(0, |s| {
            s.set_cell(0,0,"Região"); s.set_cell(0,1,"Vendas");
            s.set_cell(1,0,"Sul"); s.set_cell(1,1,"10");
            s.set_cell(2,0,"Norte"); s.set_cell(2,1,"20");
        });
        app.fim_selecao = (2,1);
        let _ = app.evento_dinamica(Evento::Abrir);
        crate::app::dinamica_ui::campos_de_antes(&mut app);
        assert!(app.dinamica.as_ref().unwrap().fase == Fase::Origem);
        let _ = app.evento_dinamica(Evento::Criar); // OK do primeiro passo
        let e = app.dinamica.as_ref().expect("painel de campos aberto");
        assert!(e.fase == Fase::Campos && e.editando.is_some());
        assert_eq!(app.workbook.len(), 2);
        // Marcar/desmarcar campos e trocar a agregação atualiza a grade na hora (Contagem = 1).
        let _ = app.evento_dinamica(Evento::Campo(1)); // remove Vendas
        let _ = app.evento_dinamica(Evento::Campo(1)); // marca de novo (numérico -> Valores)
        assert_eq!(app.dinamica.as_ref().unwrap().valores.len(), 1);
        let _ = app.evento_dinamica(Evento::Operacao(1));
        assert!(app.dinamica.is_some());
        let _ = app.evento_dinamica(Evento::Fechar);
        assert!(app.dinamica.is_none());
        assert_eq!(app.workbook.valor(1,2,1), Value::Number(1.));
    }
}
