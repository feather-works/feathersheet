//! Impressão — paridade com ui/caixas/print-dialog.ts, as ações
//! print/setPrintArea/clearPrintArea/toggleShowPrintArea de
//! acoes-do-menu.ts e a moldura de ui/grade/impressao.ts.
//! A área de impressão e as quebras ficam só em memória, por id de aba
//! (como `sheet.printArea`/`showPrintArea` no TS): não vão para o arquivo.
use super::*;
use crate::grade::{impressao::calcular_quebras, ImpressaoVisual};
use feathersheet_core::{
    celulas::{refs::col_name, style::{effective_align, Align}, values::Value},
    planilha::impressao::{AreaImpressao, ConfigPagina, Escala, Margens, Orientacao, Papel},
};
use iced::widget::{canvas as cv, checkbox, pick_list, radio};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Linhas por página da prévia (aproximação do TS: 35 retrato, 22 paisagem).
const LINHAS_RETRATO: u32 = 35;
const LINHAS_PAISAGEM: u32 = 22;
/// Página A4 retrato em pixels úteis, usada nas quebras da grade (TS).
const PAGINA_L: f32 = 714.0;
const PAGINA_A: f32 = 1043.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Escopo { Planilha, Selecao, Area }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PapelUi { A4, Carta, Oficio }
impl PapelUi {
    const TODOS: [PapelUi; 3] = [PapelUi::A4, PapelUi::Carta, PapelUi::Oficio];
    fn papel(self) -> Papel {
        match self { Self::A4 => Papel::A4, Self::Carta => Papel::Carta, Self::Oficio => Papel::Oficio }
    }
    fn sigla(self) -> &'static str {
        match self { Self::A4 => "A4", Self::Carta => tr!("CARTA"), Self::Oficio => tr!("OFÍCIO") }
    }
}
impl std::fmt::Display for PapelUi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::A4 => tr!("A4 (210 x 297 mm)"),
            Self::Carta => tr!("Carta / Letter (216 x 279 mm)"),
            Self::Oficio => tr!("Ofício / Legal (216 x 356 mm)"),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscalaUi { Cem, Pagina, Largura }
impl EscalaUi {
    const TODAS: [EscalaUi; 3] = [EscalaUi::Cem, EscalaUi::Pagina, EscalaUi::Largura];
    fn escala(self) -> Escala {
        match self {
            Self::Cem => Escala::Porcentagem(100),
            Self::Pagina => Escala::AjustarPagina,
            Self::Largura => Escala::AjustarLargura,
        }
    }
}
impl std::fmt::Display for EscalaUi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Cem => tr!("100% - Tamanho real"),
            Self::Pagina => tr!("Ajustar à 1 página"),
            Self::Largura => tr!("Ajustar largura das colunas"),
        })
    }
}

/// Margens prontas, como no Margens ▾ do Excel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MargemUi { Normal, Estreita, Larga }
impl MargemUi {
    fn margens(self) -> Margens {
        let (lados, topo) = match self {
            Self::Normal => return Margens::default(),
            Self::Estreita => (6.4, 19.1),
            Self::Larga => (25.4, 25.4),
        };
        Margens { esquerda_mm: lados, direita_mm: lados, topo_mm: topo, base_mm: topo }
    }
}

/// A configuração da página de uma aba: a aba Layout da Página e o diálogo
/// de impressão leem e gravam a mesma, por id de aba (só em memória, como a
/// área de impressão).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Pagina {
    pub orientacao: Orientacao,
    pub papel: PapelUi,
    pub escala: EscalaUi,
    pub margem: MargemUi,
    pub grade: bool,
    pub cabecalhos: bool,
}
impl Default for Pagina {
    fn default() -> Self {
        Pagina {
            orientacao: Orientacao::Retrato, papel: PapelUi::A4, escala: EscalaUi::Cem,
            margem: MargemUi::Normal, grade: true, cabecalhos: true,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Evento {
    Abrir,
    DefinirArea,
    AdicionarArea,
    LimparArea,
    AlternarQuebras,
    Escopo(Escopo),
    Orientacao(Orientacao),
    Papel(PapelUi),
    Escala(EscalaUi),
    Grade(bool),
    Cabecalhos(bool),
    /// Textos do cabeçalho e do rodapé da página (vazios = os padrões).
    TextoCabecalho(String),
    TextoRodape(String),
    Margem(MargemUi),
    Anterior,
    Proxima,
    SalvarPdf,
    DestinoPdf(Option<PathBuf>),
    Imprimir,
    Concluido(Result<String, String>),
    Fechar,
}

/// Estado do diálogo; a seleção é copiada ao abrir, como no TS.
pub(super) struct Estado {
    folha: usize,
    selecao: Rect,
    escopo: Escopo,
    orientacao: Orientacao,
    papel: PapelUi,
    escala: EscalaUi,
    grade: bool,
    cabecalhos: bool,
    margem: MargemUi,
    cabecalho: String,
    rodape: String,
    pagina: u32,
}

type Quebras = (Rc<Vec<u32>>, Rc<Vec<u32>>);

/// Estado de impressão da GUI: por id de aba, só em memória.
#[derive(Default)]
pub(super) struct Dados {
    /// As áreas de impressão de cada aba, na ordem (o Excel aceita várias).
    areas: HashMap<u32, Vec<Rect>>,
    quebras_visiveis: HashSet<u32>,
    paginas: HashMap<u32, Pagina>,
    /// Cabeçalho e rodapé de cada aba.
    textos: HashMap<u32, (String, String)>,
    dialogo: Option<Estado>,
    cache: RefCell<Option<(u32, u32, Quebras)>>,
}

// Ponto único de chamada do gerador de PDF da área de impressão.
fn gerar_pdf(wb: &Workbook, folha: usize, area: AreaImpressao,
    config: &ConfigPagina) -> Result<Vec<u8>, String> {
    feathersheet_io::impressao_pdf::gerar(wb, folha, area, config)
}

fn data_hoje() -> String {
    let segundos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let dias = (segundos / 86_400) as i64 + 719_468;
    let era = dias.div_euclid(146_097);
    let doe = dias.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = doy - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let ano = yoe + era * 400 + if mes <= 2 { 1 } else { 0 };
    format!("{dia:02}/{mes:02}/{ano}")
}

fn enviar_para_impressora(caminho: &Path) -> Result<String, String> {
    #[cfg(windows)]
    {
        // Sem `cmd /c start`: abriria uma janela de console.
        std::process::Command::new("rundll32")
            .arg("url.dll,FileProtocolHandler").arg(caminho).spawn()
            .map_err(|e| trf!("Não foi possível abrir o PDF para imprimir: {e}"))?;
        Ok(tr!("PDF aberto no programa padrão para imprimir").into())
    }
    #[cfg(not(windows))]
    {
        match std::process::Command::new("lp").arg(caminho).output() {
            Ok(saida) if saida.status.success() => Ok(tr!("Documento enviado para a impressora (lp)").into()),
            Ok(saida) => Err(trf!("O comando lp falhou: {}",
                String::from_utf8_lossy(&saida.stderr).trim())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound =>
                Err(tr!("Comando 'lp' não encontrado. Instale o CUPS ou use \"Salvar PDF…\".").into()),
            Err(e) => Err(trf!("Falha ao executar lp: {e}")),
        }
    }
}

fn rect_texto(r: &Rect) -> String {
    format!("{}{}:{}{}", col_name(r.c0), r.r0 + 1, col_name(r.c1), r.r1 + 1)
}

impl Estado {
    fn linhas_por_pagina(&self) -> u32 {
        if self.orientacao == Orientacao::Retrato { LINHAS_RETRATO } else { LINHAS_PAISAGEM }
    }
    fn config(&self) -> ConfigPagina {
        ConfigPagina {
            papel: self.papel.papel(), orientacao: self.orientacao,
            escala: self.escala.escala(), linhas_de_grade: self.grade,
            cabecalhos: self.cabecalhos, margens: self.margem.margens(),
            cabecalho: self.cabecalho.clone(), rodape: self.rodape.clone(),
        }
    }
    fn total_paginas(&self, faixa: &Rect) -> u32 {
        (faixa.r1 - faixa.r0 + 1).div_ceil(self.linhas_por_pagina()).max(1)
    }
}

impl App {
    fn id_aba_atual(&self) -> Option<u32> { self.workbook.aba(self.folha).map(|a| a.id) }

    pub(super) fn area_impressao(&self) -> Option<Rect> {
        self.impressao.areas.get(&self.id_aba_atual()?)?.first().cloned()
    }

    /// Todas as áreas de impressão da aba ativa.
    pub(super) fn areas_impressao(&self) -> Vec<Rect> {
        self.id_aba_atual().and_then(|id| self.impressao.areas.get(&id).cloned()).unwrap_or_default()
    }

    /// A área de impressão definida na aba de `id` (o PDF pedido por macro).
    pub(super) fn area_impressao_da_aba(&self, id: u32) -> Option<Rect> {
        self.impressao.areas.get(&id)?.first().cloned()
    }

    #[cfg(test)]
    pub(super) fn definir_area_impressao_da_aba(&mut self, id: u32, area: Rect) {
        self.impressao.areas.insert(id, vec![area]);
    }

    /// As faixas que o diálogo imprime, na ordem: todas as áreas de
    /// impressão da aba dele, a seleção ou a planilha usada.
    fn faixas_impressao(&self, e: &Estado) -> Vec<Rect> {
        let areas = self.workbook.aba(e.folha).and_then(|a| self.impressao.areas.get(&a.id)).cloned().unwrap_or_default();
        match e.escopo {
            Escopo::Selecao => vec![e.selecao.clone()],
            Escopo::Area if !areas.is_empty() => areas,
            _ => vec![self.workbook.aba(e.folha).map(|a| {
                let u = a.sheet.used_rect();
                Rect::new(0, 0, u.r1, u.c1)
            }).unwrap_or_else(|| Rect::new(0, 0, 0, 0))],
        }
    }

    /// Cada área começa em página nova: o total soma as páginas de todas.
    fn total_paginas_dialogo(&self, e: &Estado) -> u32 {
        self.faixas_impressao(e).iter().map(|f| e.total_paginas(f)).sum::<u32>().max(1)
    }

    /// A faixa da página `pagina` (contada no total) e a página dentro dela.
    fn pagina_na_faixa(&self, e: &Estado, pagina: u32) -> (Rect, u32) {
        let faixas = self.faixas_impressao(e);
        let mut resto = pagina.max(1);
        for f in &faixas {
            let n = e.total_paginas(f);
            if resto <= n { return (f.clone(), resto); }
            resto -= n;
        }
        let ultima = faixas.last().cloned().unwrap_or_else(|| Rect::new(0, 0, 0, 0));
        let n = e.total_paginas(&ultima);
        (ultima, n)
    }

    /// A configuração da página da aba ativa (aba Layout da Página).
    pub(super) fn pagina_atual(&self) -> Pagina {
        self.id_aba_atual().and_then(|id| self.impressao.paginas.get(&id).copied()).unwrap_or_default()
    }

    /// Muda a página da aba do diálogo aberto (ou da ativa) e leva a mudança
    /// ao diálogo, para a faixa e a prévia nunca discordarem.
    fn mudar_pagina(&mut self, f: impl FnOnce(&mut Pagina)) {
        let folha = self.impressao.dialogo.as_ref().map_or(self.folha, |e| e.folha);
        let Some(id) = self.workbook.aba(folha).map(|a| a.id) else { return };
        let mut p = self.impressao.paginas.get(&id).copied().unwrap_or_default();
        f(&mut p);
        self.impressao.paginas.insert(id, p);
        if let Some(e) = self.impressao.dialogo.as_mut() {
            if e.orientacao != p.orientacao { e.pagina = 1; }
            e.orientacao = p.orientacao; e.papel = p.papel; e.escala = p.escala;
            e.margem = p.margem; e.grade = p.grade; e.cabecalhos = p.cabecalhos;
        }
    }

    /// Cabeçalho/rodapé do diálogo, guardados na aba dele para a próxima vez.
    fn mudar_textos(&mut self, f: impl FnOnce(&mut Estado)) {
        let Some(e) = self.impressao.dialogo.as_mut() else { return };
        f(e);
        let par = (e.cabecalho.clone(), e.rodape.clone());
        if let Some(id) = self.workbook.aba(e.folha).map(|a| a.id) { self.impressao.textos.insert(id, par); }
    }

    /// A aba Layout da Página da faixa, como no Excel: Configurar Página,
    /// Dimensionar para Ajustar, Opções de Planilha e Imprimir.
    pub(super) fn grupos_layout_pagina(&self) -> Vec<Element<'_, Msg>> {
        let p = self.pagina_atual();
        let menu = |rotulo: &str, icone: &str, tipo: &'static str| self.faixa_grande_seta(
            rotulo, icone, Some(Msg::AlternarMenuEdicao(tipo)), self.edicao_menu == Some(tipo));
        vec![
            self.faixa_grupo(tr!("Configurar Página"), self.faixa_linha(vec![
                menu(tr!("Margens"), "borda_externa_espessa", "pg_margens"),
                menu(tr!("Orientação"), "orientacao_pagina", "pg_orientacao"),
                menu(tr!("Tamanho"), "arrows-maximize", "pg_tamanho"),
                menu(tr!("Área de impressão"), "area_impressao", "pg_area"),
                self.faixa_grande(tr!("Quebras"), "quebra_pagina",
                    Some(Msg::Impressao(Evento::AlternarQuebras)), self.quebras_mostradas()),
            ])),
            self.faixa_grupo(tr!("Dimensionar"), self.faixa_linha(vec![
                menu(tr!("Escala"), "zoom-out", "pg_escala"),
            ])),
            self.faixa_grupo(tr!("Opções de Planilha"), self.faixa_pilha(vec![
                self.faixa_pequeno(tr!("Imprimir linhas de grade"), "linhas_grade",
                    Some(Msg::Impressao(Evento::Grade(!p.grade))), p.grade),
                self.faixa_pequeno(tr!("Imprimir títulos (A, B, 1, 2)"), "faixas_colunas",
                    Some(Msg::Impressao(Evento::Cabecalhos(!p.cabecalhos))), p.cabecalhos),
            ])),
            self.faixa_grupo(tr!("Imprimir"), self.faixa_linha(vec![
                self.faixa_grande(tr!("Visualizar"), "imprimir", Some(Msg::Impressao(Evento::Abrir)),
                    self.janelas.aberta(Auxiliar::Imprimir)),
            ])),
        ]
    }

    pub(super) fn quebras_mostradas(&self) -> bool {
        self.id_aba_atual().is_some_and(|id| self.impressao.quebras_visiveis.contains(&id))
    }

    /// Dados para a moldura/quebras da grade. As quebras são calculadas
    /// uma vez por (aba, versão) até a última célula usada, como o cache do TS.
    pub(super) fn impressao_visual(&self) -> Option<ImpressaoVisual> {
        let aba = self.workbook.aba(self.folha)?;
        let mut areas = self.impressao.areas.get(&aba.id).cloned().unwrap_or_default();
        let area = (!areas.is_empty()).then(|| areas.remove(0));
        let mostrar = self.impressao.quebras_visiveis.contains(&aba.id);
        if area.is_none() && !mostrar { return None; }
        let mut visual = ImpressaoVisual { area, outras: areas, ..ImpressaoVisual::default() };
        if mostrar {
            let mut cache = self.impressao.cache.borrow_mut();
            if !matches!(&*cache, Some((id, v, _)) if *id == aba.id && *v == aba.versao) {
                let usado = aba.sheet.used_rect();
                let (fim_l, fim_c) = (usado.r1 + 5, usado.c1 + 5);
                let linhas = calcular_quebras(fim_l, PAGINA_A,
                    |r| if aba.sheet.linha_oculta(r) { 0.0 } else {
                        aba.sheet.row_heights.get(&r).map(|v| *v as f32)
                            .unwrap_or(crate::grade::ALTURA_PADRAO)
                    });
                let colunas = calcular_quebras(fim_c, PAGINA_L,
                    |c| if aba.sheet.coluna_oculta(c) { 0.0 } else {
                        aba.sheet.col_widths.get(&c).map(|v| *v as f32)
                            .unwrap_or_else(|| crate::grade::largura_padrao_da(&aba.sheet))
                    });
                *cache = Some((aba.id, aba.versao, (Rc::new(linhas), Rc::new(colunas))));
            }
            if let Some((_, _, (l, c))) = &*cache {
                visual.quebras_linhas = l.clone();
                visual.quebras_colunas = c.clone();
            }
        }
        Some(visual)
    }

    pub(super) fn abrir_impressao(&mut self) -> Task<Msg> {
        self.confirmar();
        if self.janelas.aberta(Auxiliar::Imprimir) { return Task::none(); }
        if self.workbook.aba(self.folha).is_none() { return Task::none(); }
        let tem_area = self.area_impressao().is_some();
        let p = self.pagina_atual();
        let (cabecalho, rodape) = self.id_aba_atual().and_then(|id| self.impressao.textos.get(&id).cloned()).unwrap_or_default();
        self.impressao.dialogo = Some(Estado {
            folha: self.folha, selecao: self.area_selecionada().normalized(),
            escopo: if tem_area { Escopo::Area } else { Escopo::Planilha },
            orientacao: p.orientacao, papel: p.papel, escala: p.escala,
            grade: p.grade, cabecalhos: p.cabecalhos, margem: p.margem, cabecalho, rodape, pagina: 1,
        });
        self.alternar(Auxiliar::Imprimir)
    }

    /// Define a área pela seleção; com várias seleções (Ctrl+clique), cada
    /// uma vira uma área, como no Excel.
    fn definir_area(&mut self) {
        let Some(id) = self.id_aba_atual() else { return };
        let areas: Vec<Rect> = self.areas_selecionadas().into_iter().map(|a| a.normalized()).collect();
        self.impressao.areas.insert(id, areas);
        // O TS (setPrintAreaFromSelection) também mostra as quebras ao definir.
        self.impressao.quebras_visiveis.insert(id);
        self.status = tr!("Área de impressão definida.").into();
    }

    /// "Adicionar à área de impressão": a seleção entra como mais uma área.
    fn adicionar_area(&mut self) {
        let Some(id) = self.id_aba_atual() else { return };
        let novas: Vec<Rect> = self.areas_selecionadas().into_iter().map(|a| a.normalized()).collect();
        let lista = self.impressao.areas.entry(id).or_default();
        for a in novas {
            if !lista.contains(&a) { lista.push(a); }
        }
        self.impressao.quebras_visiveis.insert(id);
        self.status = trf!("Área de impressão com {} parte(s).", lista.len());
    }

    fn limpar_area(&mut self) {
        if let Some(id) = self.id_aba_atual() { self.impressao.areas.remove(&id); }
        self.status = tr!("Área de impressão removida.").into();
    }

    pub(super) fn evento_impressao(&mut self, ev: Evento) -> Task<Msg> {
        match ev {
            Evento::Abrir => return self.abrir_impressao(),
            Evento::DefinirArea => {
                self.definir_area();
                if let Some(e) = self.impressao.dialogo.as_mut() {
                    e.escopo = Escopo::Area; e.pagina = 1;
                }
            }
            Evento::AdicionarArea => {
                self.adicionar_area();
                if let Some(e) = self.impressao.dialogo.as_mut() {
                    e.escopo = Escopo::Area; e.pagina = 1;
                }
            }
            Evento::LimparArea => {
                self.limpar_area();
                if let Some(e) = self.impressao.dialogo.as_mut() {
                    e.escopo = Escopo::Planilha; e.pagina = 1;
                }
            }
            Evento::AlternarQuebras => {
                if let Some(id) = self.id_aba_atual() {
                    if !self.impressao.quebras_visiveis.remove(&id) {
                        self.impressao.quebras_visiveis.insert(id);
                    }
                }
            }
            Evento::Fechar => return self.janelas.fechar_tipo(Auxiliar::Imprimir),
            Evento::Escopo(escopo) => {
                if escopo == Escopo::Area && self.area_impressao().is_none() {
                    return Task::none();
                }
                if let Some(e) = self.impressao.dialogo.as_mut() {
                    e.escopo = escopo; e.pagina = 1;
                }
            }
            Evento::Orientacao(o) => self.mudar_pagina(|p| p.orientacao = o),
            Evento::Papel(v) => self.mudar_pagina(|p| p.papel = v),
            Evento::Escala(s) => self.mudar_pagina(|p| p.escala = s),
            Evento::Grade(b) => self.mudar_pagina(|p| p.grade = b),
            Evento::Cabecalhos(b) => self.mudar_pagina(|p| p.cabecalhos = b),
            Evento::TextoCabecalho(t) => self.mudar_textos(|e| e.cabecalho = t),
            Evento::TextoRodape(t) => self.mudar_textos(|e| e.rodape = t),
            Evento::Margem(m) => self.mudar_pagina(|p| p.margem = m),
            Evento::Anterior => if let Some(e) = self.impressao.dialogo.as_mut() {
                e.pagina = e.pagina.saturating_sub(1).max(1);
            },
            Evento::Proxima => {
                let total = self.impressao.dialogo.as_ref()
                    .map(|e| self.total_paginas_dialogo(e)).unwrap_or(1);
                if let Some(e) = self.impressao.dialogo.as_mut() {
                    e.pagina = (e.pagina + 1).min(total);
                }
            }
            Evento::SalvarPdf => {
                if self.impressao.dialogo.is_none() { return Task::none(); }
                let nome = self.workbook.aba(self.folha)
                    .map(|a| a.sheet.name.clone()).unwrap_or_else(|| tr!("Planilha").into());
                let nome: String = nome.chars().map(|c|
                    if "/\\:*?\"<>|".contains(c) { '_' } else { c }).collect();
                return Task::perform(async move {
                    rfd::AsyncFileDialog::new().set_directory(dialogos::pasta_padrao())
                        .add_filter("PDF", &["pdf"]).set_file_name(&format!("{nome}.pdf"))
                        .save_file().await.map(|f| f.path().to_path_buf())
                }, |d| Msg::Impressao(Evento::DestinoPdf(d)));
            }
            Evento::DestinoPdf(None) => {}
            Evento::DestinoPdf(Some(destino)) => {
                let bytes = match self.pdf_do_dialogo() {
                    Ok(b) => b,
                    Err(e) => { self.status = trf!("Não foi possível gerar o PDF: {e}"); return Task::none(); }
                };
                return Task::perform(async move {
                    tokio::task::spawn_blocking(move || std::fs::write(&destino, bytes)
                        .map(|_| trf!("PDF salvo em {}", destino.display()))
                        .map_err(|e| trf!("Erro ao gravar o PDF: {e}")))
                        .await.map_err(|e| trf!("Erro ao gravar o PDF: {e}"))?
                }, |r| Msg::Impressao(Evento::Concluido(r)));
            }
            Evento::Imprimir => {
                let bytes = match self.pdf_do_dialogo() {
                    Ok(b) => b,
                    Err(e) => { self.status = trf!("Não foi possível gerar o PDF: {e}"); return Task::none(); }
                };
                let temporario = std::env::temp_dir()
                    .join(format!("feathersheet-impressao-{}.pdf", std::process::id()));
                return Task::perform(async move {
                    tokio::task::spawn_blocking(move || {
                        std::fs::write(&temporario, bytes)
                            .map_err(|e| trf!("Erro ao gravar o PDF temporário: {e}"))?;
                        enviar_para_impressora(&temporario)
                    }).await.map_err(|e| trf!("Erro ao imprimir: {e}"))?
                }, |r| Msg::Impressao(Evento::Concluido(r)));
            }
            Evento::Concluido(r) => {
                self.status = match r { Ok(m) | Err(m) => m };
            }
        }
        Task::none()
    }

    fn pdf_do_dialogo(&self) -> Result<Vec<u8>, String> {
        let e = self.impressao.dialogo.as_ref().ok_or(tr!("diálogo fechado"))?;
        let faixas = self.faixas_impressao(e);
        // Várias áreas de impressão: cada uma em páginas próprias, na ordem.
        if faixas.len() > 1 {
            let areas: Vec<AreaImpressao> = faixas.iter()
                .map(|a| AreaImpressao { linha0: a.r0, coluna0: a.c0, linha1: a.r1, coluna1: a.c1 }).collect();
            return feathersheet_io::impressao_pdf::gerar_varias(&self.workbook, e.folha, &areas, &e.config());
        }
        let f = &faixas[0];
        let area = AreaImpressao { linha0: f.r0, coluna0: f.c0, linha1: f.r1, coluna1: f.c1 };
        gerar_pdf(&self.workbook, e.folha, area, &e.config())
    }

    pub(super) fn view_impressao(&self) -> Element<'_, Msg> {
        let Some(e) = self.impressao.dialogo.as_ref() else {
            return container(text(tr!("Janela fechada"))).into();
        };
        let area = self.area_impressao();
        let total = self.total_paginas_dialogo(e);
        let pagina = e.pagina.min(total);
        let (faixa, local) = self.pagina_na_faixa(e, pagina);
        let secao = |t: &str| text(t.to_owned()).size(12).color(COR_TEXTO_MUTED());
        let ev = |x: Evento| Msg::Impressao(x);

        let r_sel = trf!("Seleção atual ({})", rect_texto(&e.selecao));
        let todas = self.areas_impressao();
        let r_area = match &area {
            Some(_) => trf!("Área de Impressão definida ({})",
                todas.iter().map(rect_texto).collect::<Vec<_>>().join("; ")),
            None => tr!("Área de Impressão definida (Nenhuma)").into(),
        };
        let escopo = iced::widget::column![
            secao(tr!("O que imprimir")),
            radio(tr!("Planilha ativa"), Escopo::Planilha, Some(e.escopo),
                |v| Msg::Impressao(Evento::Escopo(v))).size(15).text_size(13),
            radio(r_sel, Escopo::Selecao, Some(e.escopo),
                |v| Msg::Impressao(Evento::Escopo(v))).size(15).text_size(13),
            radio(r_area, Escopo::Area, Some(e.escopo),
                |v| Msg::Impressao(Evento::Escopo(v))).size(15).text_size(13),
        ].spacing(5);
        let area_botoes = iced::widget::column![
            secao(tr!("Área de Impressão")),
            row![
                button(text(tr!("Definir Seleção")).size(12)).padding([6, 10])
                    .style(estilo_botao_secundario).on_press(ev(Evento::DefinirArea)),
                button(text(tr!("Adicionar Seleção")).size(12)).padding([6, 10])
                    .style(estilo_botao_secundario).on_press(ev(Evento::AdicionarArea)),
                button(text(tr!("Limpar Área")).size(12)).padding([6, 10])
                    .style(estilo_botao_secundario)
                    .on_press_maybe(area.is_some().then(|| ev(Evento::LimparArea))),
            ].spacing(8),
        ].spacing(5);
        let orient = |rotulo: &str, o: Orientacao| button(text(rotulo.to_owned()).size(12))
            .padding([6, 12]).width(Length::Fill)
            .style(estilo_botao_pequeno(e.orientacao == o))
            .on_press(ev(Evento::Orientacao(o)));
        let orientacao = iced::widget::column![
            secao(tr!("Orientação")),
            row![orient(tr!("Retrato"), Orientacao::Retrato),
                orient(tr!("Paisagem"), Orientacao::Paisagem)].spacing(6),
        ].spacing(5);
        let papel = iced::widget::column![
            secao(tr!("Tamanho do Papel")),
            pick_list(PapelUi::TODOS, Some(e.papel), |p| Msg::Impressao(Evento::Papel(p)))
                .text_size(13).width(Length::Fill),
        ].spacing(5);
        let escala = iced::widget::column![
            secao(tr!("Escala e Dimensionamento")),
            pick_list(EscalaUi::TODAS, Some(e.escala), |s| Msg::Impressao(Evento::Escala(s)))
                .text_size(13).width(Length::Fill),
        ].spacing(5);
        let exibicao = iced::widget::column![
            secao(tr!("Opções de Exibição")),
            checkbox(tr!("Imprimir linhas de grade"), e.grade)
                .on_toggle(|b| Msg::Impressao(Evento::Grade(b))).size(15).text_size(13),
            checkbox(tr!("Imprimir títulos (A,B,C e 1,2,3)"), e.cabecalhos)
                .on_toggle(|b| Msg::Impressao(Evento::Cabecalhos(b))).size(15).text_size(13),
        ].spacing(5);
        let textos = iced::widget::column![
            secao(tr!("Cabeçalho e rodapé")),
            iced::widget::text_input(tr!("Cabeçalho (vazio: nome da planilha)"), &e.cabecalho)
                .on_input(|t| Msg::Impressao(Evento::TextoCabecalho(t))).size(13),
            iced::widget::text_input(tr!("Rodapé (vazio: padrão)"), &e.rodape)
                .on_input(|t| Msg::Impressao(Evento::TextoRodape(t))).size(13),
        ].spacing(5);
        let rotulo_botao = |icone: &str, t: &str| row![
            feather_ui::icones::icone(icone, 14.0), text(t.to_owned()).size(13),
        ].spacing(6).align_y(Alignment::Center);
        let acoes = iced::widget::column![
            button(rotulo_botao("file-export", tr!("Salvar PDF…"))).padding([8, 12])
                .width(Length::Fill).style(estilo_botao_primario)
                .on_press(ev(Evento::SalvarPdf)),
            button(rotulo_botao("printer", tr!("Imprimir"))).padding([8, 12])
                .width(Length::Fill).style(estilo_botao_primario)
                .on_press(ev(Evento::Imprimir)),
            button(text(tr!("Cancelar")).size(13)).padding([8, 12]).width(Length::Fill)
                .style(estilo_botao_secundario).on_press(ev(Evento::Fechar)),
        ].spacing(6);
        let lateral = container(iced::widget::column![
            escopo, area_botoes, orientacao, papel, escala, exibicao, textos,
            Space::with_height(Length::Fill), acoes,
        ].spacing(10).height(Length::Fill))
            .padding(14).width(Length::Fixed(300.)).height(Length::Fill)
            .style(estilo_painel_faixa);

        let nav = row![
            button(feather_ui::icones::icone("chevron-left", 14.0)).padding([5, 8])
                .style(estilo_botao_pequeno(false))
                .on_press_maybe((pagina > 1).then(|| ev(Evento::Anterior))),
            text(trf!("Página {pagina} de {total}")).size(13),
            button(feather_ui::icones::icone("chevron-right", 14.0)).padding([5, 8])
                .style(estilo_botao_pequeno(false))
                .on_press_maybe((pagina < total).then(|| ev(Evento::Proxima))),
            Space::with_width(Length::Fill),
            text(format!("{} · {}", e.papel.sigla(),
                if e.orientacao == Orientacao::Retrato { tr!("Retrato") } else { tr!("Paisagem") }))
                .size(12).color(COR_TEXTO_MUTED()),
        ].spacing(8).align_y(Alignment::Center);
        let previa = cv::Canvas::new(self.previa(e, &faixa, local, pagina, total))
            .width(Length::Fill).height(Length::Fill);
        let direita = iced::widget::column![nav, previa]
            .spacing(8).width(Length::Fill).height(Length::Fill);
        container(row![lateral, direita].spacing(12).padding(12))
            .width(Length::Fill).height(Length::Fill)
            .style(estilo_container_topo).into()
    }

    fn previa(&self, e: &Estado, faixa: &Rect, local: u32, pagina: u32, total: u32) -> Previa {
        let por_pagina = e.linhas_por_pagina();
        let r0 = faixa.r0 + (local - 1) * por_pagina;
        let r1 = faixa.r1.min(r0 + por_pagina - 1);
        let mut linhas = Vec::new();
        if let Some(aba) = self.workbook.aba(e.folha) {
            for r in r0..=r1 {
                let mut celulas = Vec::new();
                for c in faixa.c0..=faixa.c1 {
                    let estilo = aba.sheet.estilo_resolvido(r, c);
                    let valor = self.workbook.valor(e.folha, r, c);
                    let alinh = effective_align(&valor, estilo.as_ref());
                    let texto = match &valor {
                        Value::Number(_) => feathersheet_core::celulas::style::format_value(&valor, estilo.as_ref()),
                        Value::Empty => String::new(),
                        Value::Text(s) => s.clone(),
                        Value::Bool(b) => if *b { "VERDADEIRO".into() } else { "FALSO".into() },
                        Value::Error(err) => err.code.as_str().into(),
                    };
                    let v = crate::estilos_comuns::projetar_estilo(estilo.as_ref(), None);
                    let cor = |c: [u8; 4]| iced::Color::from_rgba8(c[0], c[1], c[2], c[3] as f32 / 255.0);
                    celulas.push(CelulaPrevia {
                        texto, fundo: v.comum.basico.fundo.map(cor),
                        cor: v.comum.basico.texto.map(cor), negrito: v.comum.basico.negrito,
                        direita: alinh == Align::Right, centro: alinh == Align::Center,
                    });
                }
                linhas.push((r + 1, celulas));
            }
        }
        Previa {
            retrato: e.orientacao == Orientacao::Retrato,
            proporcao: { let (l, a) = e.papel.papel().tamanho_pt(); l / a },
            titulo: trf!("Planilha: {}", self.workbook.aba(e.folha)
                .map(|a| a.sheet.name.as_str()).filter(|n| !n.is_empty()).unwrap_or(tr!("Sem título"))),
            data: data_hoje(), pagina, total,
            colunas: (faixa.c0..=faixa.c1).map(col_name).collect(),
            cabecalhos: e.cabecalhos, grade: e.grade, linhas,
        }
    }
}

struct CelulaPrevia {
    texto: String, fundo: Option<iced::Color>, cor: Option<iced::Color>,
    negrito: bool, direita: bool, centro: bool,
}

/// Aproximação simples da página (como no TS): papel branco com cabeçalho,
/// tabela proporcional e rodapé "Página N de M".
struct Previa {
    retrato: bool, proporcao: f32, titulo: String, data: String,
    pagina: u32, total: u32, colunas: Vec<String>, cabecalhos: bool, grade: bool,
    linhas: Vec<(u32, Vec<CelulaPrevia>)>,
}

impl cv::Program<Msg> for Previa {
    type State = ();
    fn draw(&self, _: &Self::State, renderer: &iced::Renderer, _: &iced::Theme,
        bounds: iced::Rectangle, _: mouse::Cursor) -> Vec<cv::Geometry> {
        use iced::{alignment, Color, Point, Size};
        let mut f = cv::Frame::new(renderer, bounds.size());
        let fundo = Color::from_rgb8(120, 126, 134);
        f.fill_rectangle(Point::ORIGIN, bounds.size(), Color { a: 0.12, ..fundo });
        let razao = if self.retrato { self.proporcao } else { 1.0 / self.proporcao };
        let margem = 10.0;
        let (dispo_l, dispo_a) = ((bounds.width - 2.0 * margem).max(10.0), (bounds.height - 2.0 * margem).max(10.0));
        let (pl, pa) = if dispo_l / dispo_a > razao { (dispo_a * razao, dispo_a) }
            else { (dispo_l, dispo_l / razao) };
        let (px, py) = ((bounds.width - pl) / 2.0, (bounds.height - pa) / 2.0);
        f.fill_rectangle(Point::new(px, py), Size::new(pl, pa), Color::WHITE);
        f.stroke(&cv::Path::rectangle(Point::new(px, py), Size::new(pl, pa)),
            cv::Stroke::default().with_color(Color::from_rgb8(190, 195, 202)).with_width(1.0));
        let cinza = Color::from_rgb8(110, 116, 124);
        let texto = |f: &mut cv::Frame, t: String, x: f32, y: f32, tam: f32, cor: Color,
            h: alignment::Horizontal, negrito: bool| f.fill_text(cv::Text {
            content: t, position: Point::new(x, y), color: cor, size: iced::Pixels(tam),
            horizontal_alignment: h, vertical_alignment: alignment::Vertical::Center,
            font: if negrito { iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT } }
                else { iced::Font::DEFAULT },
            ..cv::Text::default()
        });
        let pad = pl * 0.06;
        let tam_cab = (pa * 0.016).clamp(6.0, 10.0);
        texto(&mut f, self.titulo.clone(), px + pad, py + pad * 0.55, tam_cab, cinza,
            alignment::Horizontal::Left, false);
        texto(&mut f, self.data.clone(), px + pl - pad, py + pad * 0.55, tam_cab, cinza,
            alignment::Horizontal::Right, false);
        texto(&mut f, "FeatherSheet".into(), px + pad, py + pa - pad * 0.55, tam_cab, cinza,
            alignment::Horizontal::Left, false);
        texto(&mut f, trf!("Página {} de {}", self.pagina, self.total), px + pl - pad,
            py + pa - pad * 0.55, tam_cab, cinza, alignment::Horizontal::Right, false);

        let area_x = px + pad;
        let area_y = py + pad * 1.1;
        let area_l = pl - 2.0 * pad;
        let area_a = pa - pad * 2.2;
        let n_linhas = self.linhas.len() + usize::from(self.cabecalhos);
        let alt = (area_a / n_linhas.max(1) as f32).min(area_a / 28.0).max(4.0);
        let larg_num = if self.cabecalhos { (area_l * 0.07).clamp(14.0, 30.0) } else { 0.0 };
        let n_col = self.colunas.len().max(1) as f32;
        let larg = ((area_l - larg_num) / n_col).max(8.0);
        let tam = (alt * 0.58).clamp(4.0, 10.0);
        let grade = Color::from_rgb8(200, 204, 210);
        let preto = Color::from_rgb8(30, 35, 40);
        let max_chars = |l: f32| ((l - 4.0) / (tam * 0.55)).floor().max(1.0) as usize;
        let mut y = area_y;
        if self.cabecalhos {
            f.fill_rectangle(Point::new(area_x, y), Size::new(area_l, alt), Color::from_rgb8(238, 240, 243));
            for (i, nome) in self.colunas.iter().enumerate() {
                texto(&mut f, nome.clone(), area_x + larg_num + larg * (i as f32 + 0.5), y + alt / 2.0,
                    tam, cinza, alignment::Horizontal::Center, false);
            }
            y += alt;
        }
        for (numero, celulas) in &self.linhas {
            if self.cabecalhos {
                f.fill_rectangle(Point::new(area_x, y), Size::new(larg_num, alt), Color::from_rgb8(238, 240, 243));
                texto(&mut f, numero.to_string(), area_x + larg_num / 2.0, y + alt / 2.0, tam, cinza,
                    alignment::Horizontal::Center, false);
            }
            for (i, c) in celulas.iter().enumerate() {
                let x = area_x + larg_num + larg * i as f32;
                if let Some(bg) = c.fundo { f.fill_rectangle(Point::new(x, y), Size::new(larg, alt), bg); }
                if c.texto.is_empty() { continue; }
                let limite = max_chars(larg);
                let t = if c.texto.chars().count() > limite {
                    format!("{}…", c.texto.chars().take(limite.saturating_sub(1)).collect::<String>())
                } else { c.texto.clone() };
                let (tx, h) = if c.direita { (x + larg - 2.0, alignment::Horizontal::Right) }
                    else if c.centro { (x + larg / 2.0, alignment::Horizontal::Center) }
                    else { (x + 2.0, alignment::Horizontal::Left) };
                texto(&mut f, t, tx, y + alt / 2.0, tam, c.cor.unwrap_or(preto), h, c.negrito);
            }
            y += alt;
        }
        if self.grade {
            let x0 = area_x + larg_num;
            let topo = area_y + if self.cabecalhos { alt } else { 0.0 };
            let linha = |f: &mut cv::Frame, a: Point, b: Point|
                f.stroke(&cv::Path::line(a, b), cv::Stroke::default().with_color(grade).with_width(0.5));
            for i in 0..=self.colunas.len() {
                let x = x0 + larg * i as f32;
                linha(&mut f, Point::new(x, area_y), Point::new(x, y));
            }
            for i in 0..=self.linhas.len() {
                let yy = topo + alt * i as f32;
                linha(&mut f, Point::new(area_x, yy), Point::new(x0 + larg * self.colunas.len() as f32, yy));
            }
        }
        vec![f.into_geometry()]
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn app_com_dados() -> App {
        let mut app = App::default();
        for r in 0..40 {
            app.workbook.escrever_celula(0, r, 0, format!("{r}").into());
            app.workbook.escrever_celula(0, r, 1, "x".into());
        }
        app
    }

    #[test]
    fn area_e_quebras_ficam_por_aba_so_em_memoria() {
        let mut app = app_com_dados();
        app.update(Msg::Selecionar(2, 0));
        app.update(Msg::Estender(5, 1));
        let alterado = app.alterado;
        let _ = app.update(Msg::Impressao(Evento::DefinirArea));
        let a = app.area_impressao().unwrap();
        assert_eq!((a.r0, a.c0, a.r1, a.c1), (2, 0, 5, 1));
        assert!(app.quebras_mostradas());
        assert_eq!(app.alterado, alterado, "não mexe no documento");
        let visual = app.impressao_visual().unwrap();
        assert!(visual.area.is_some());
        let _ = app.update(Msg::Impressao(Evento::AlternarQuebras));
        assert!(!app.quebras_mostradas());
        let _ = app.update(Msg::Impressao(Evento::LimparArea));
        assert!(app.area_impressao().is_none());
        assert!(app.impressao_visual().is_none());
    }

    #[test]
    fn duas_areas_somam_as_paginas_da_previa_e_do_pdf() {
        let mut app = app_com_dados();
        app.update(Msg::Selecionar(0, 0));
        app.update(Msg::Estender(4, 0));
        let _ = app.update(Msg::Impressao(Evento::DefinirArea));
        app.update(Msg::Selecionar(10, 1));
        app.update(Msg::Estender(12, 1));
        let _ = app.update(Msg::Impressao(Evento::AdicionarArea));
        assert_eq!(app.areas_impressao().len(), 2);
        let _ = app.update(Msg::Impressao(Evento::Abrir));
        let e = app.impressao.dialogo.as_ref().unwrap();
        assert_eq!(e.escopo, Escopo::Area);
        assert_eq!(app.total_paginas_dialogo(e), 2, "uma página por área");
        let (f, local) = app.pagina_na_faixa(e, 2);
        assert_eq!((f.r0, f.c0, local), (10, 1, 1));
        let _ = app.update(Msg::Impressao(Evento::Proxima));
        assert_eq!(app.impressao.dialogo.as_ref().unwrap().pagina, 2);
        assert!(app.pdf_do_dialogo().is_ok());
    }

    #[test]
    fn dialogo_pagina_pelo_modelo_do_ts_e_gera_pdf_pelo_ponto_unico() {
        let mut app = app_com_dados();
        let _ = app.update(Msg::Impressao(Evento::Abrir));
        assert!(app.janelas.aberta(Auxiliar::Imprimir));
        let e = app.impressao.dialogo.as_ref().unwrap();
        let faixa = app.faixas_impressao(e)[0].clone();
        assert_eq!(e.total_paginas(&faixa), 2, "40 linhas / 35 por página");
        let _ = app.update(Msg::Impressao(Evento::Orientacao(Orientacao::Paisagem)));
        let e = app.impressao.dialogo.as_ref().unwrap();
        assert_eq!(e.total_paginas(&faixa), 2, "40 linhas / 22 por página");
        let _ = app.update(Msg::Impressao(Evento::Proxima));
        assert_eq!(app.impressao.dialogo.as_ref().unwrap().pagina, 2);
        let _: Element<'_, Msg> = app.view_impressao();
        // Área inexistente não pode ser escolhida como escopo.
        let _ = app.update(Msg::Impressao(Evento::Escopo(Escopo::Area)));
        assert_ne!(app.impressao.dialogo.as_ref().unwrap().escopo, Escopo::Area);
    }

    #[test]
    fn aba_layout_da_pagina_grava_por_aba_e_o_dialogo_abre_com_ela() {
        let mut app = app_com_dados();
        app.faixa_ativa = AbaFaixa::Layout;
        let _: Vec<Element<'_, Msg>> = app.grupos_layout_pagina();
        let _ = app.update(Msg::Impressao(Evento::Orientacao(Orientacao::Paisagem)));
        let _ = app.update(Msg::Impressao(Evento::Papel(PapelUi::Oficio)));
        let _ = app.update(Msg::Impressao(Evento::Margem(MargemUi::Larga)));
        let _ = app.update(Msg::Impressao(Evento::Grade(false)));
        let p = app.pagina_atual();
        assert_eq!((p.orientacao, p.papel, p.margem, p.grade), (Orientacao::Paisagem, PapelUi::Oficio, MargemUi::Larga, false));
        // O diálogo abre com o que a faixa escolheu, e o PDF leva as margens.
        let _ = app.update(Msg::Impressao(Evento::Abrir));
        let e = app.impressao.dialogo.as_ref().unwrap();
        assert_eq!((e.orientacao, e.papel, e.grade), (Orientacao::Paisagem, PapelUi::Oficio, false));
        assert_eq!(e.config().margens.esquerda_mm, 25.4);
        // Mudar no diálogo volta para a faixa.
        let _ = app.update(Msg::Impressao(Evento::Escala(EscalaUi::Pagina)));
        assert_eq!(app.pagina_atual().escala, EscalaUi::Pagina);
        // Outra aba começa no padrão.
        app.update(Msg::AdicionarAba);
        assert_eq!(app.pagina_atual(), Pagina::default());
    }

    #[test]
    fn gerar_pdf_produz_bytes_pdf() {
        let app = app_com_dados();
        let area = AreaImpressao { linha0: 0, coluna0: 0, linha1: 4, coluna1: 1 };
        let bytes = gerar_pdf(&app.workbook, 0, area, &ConfigPagina::default()).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn menu_e_atalho_de_impressao_estao_ligados() {
        let app = App::default();
        for id in ["print", "setPrintArea", "clearPrintArea", "toggleShowPrintArea"] {
            let acao = menu_nativo::Acao { id: id.into(), args: vec![] };
            assert!(app.evento_menu(&acao).is_some(), "{id} deve estar ligada");
        }
    }

    #[test]
    fn data_civil_do_epoch() {
        assert!(data_hoje().len() == 10);
    }
}
