//! Referência viva entre dois arquivos abertos (meta da v0.1.0, 16/10/2026):
//! `=[Vendas.fshz]Dados!A1`, `=SOMA([Vendas.fshz]Dados!A1:A10)`,
//! `='/home/x/[Vendas.fshz]Dados'!A1`.
//!
//! O motor (`feathersheet_core::planilha::vinculo` e `workbook::vinculos`)
//! guarda os valores de fora e recalcula só quem os lê; aqui mora o resto:
//!
//! - **destino** (a pasta que cita): resolve cada arquivo citado para um
//!   caminho (relativo à pasta deste arquivo, ou o último caminho guardado),
//!   e então: a origem aberta em outra janela → conecta e assina as áreas
//!   citadas (valores vivos, só as dependentes recalculam); fechada → valem
//!   os últimos valores guardados no próprio `.fshz`, com a faixa "Valores de
//!   [Vendas.fshz] de 16/10 14:32" e o botão "Atualizar valores" (lê do
//!   disco com o leitor de sempre). Nunca carregada e existindo no disco →
//!   lê do disco uma vez, sozinha;
//! - **origem** (o arquivo citado, aberto aqui): anuncia que está aberto
//!   (`canal::registrar_aberto`) e, depois de cada recálculo, manda a quem
//!   assina só as células das áreas assinadas que mudaram;
//! - **Editar links** (Dados → Editar links): lista as origens e deixa
//!   atualizar e trocar o arquivo de cada uma (renomeado ou movido).
//!
//! Para depois (anotado no `CLAUDE.md`): autocompletar o nome do arquivo na
//! fórmula e "clicar na outra janela para apontar".
use super::*;
use feathersheet_core::celulas::values::Value;
use feathersheet_core::planilha::vinculo::{self as motor_vinculo, chave_arquivo, mesmo_arquivo};
use pelicano::futures::StreamExt;
use serde_json::Value as Json;
use std::collections::{BTreeSet, HashMap};

pub(crate) mod canal;
mod tela;
#[cfg(test)]
mod testes;

use canal::{AreaPedida, Chegada};

/// Quanto esperar entre tentativas de achar uma origem aberta.
const INTERVALO_TENTATIVA: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub enum Evento {
    /// Chegou algo pelo canal local (outra janela).
    Chegou(Chegada),
    /// Tique: publicar mudanças (origem) e conferir as origens (destino).
    Pulso,
    /// Atualizar valores: de uma origem (chave) ou de todas (`None`).
    AtualizarValores(Option<String>),
    /// Fim da leitura do disco de uma origem (chave).
    Lido(String, Result<Leitura, String>),
    /// Fecha a faixa "Valores de […] de …" até a próxima abertura.
    FaixaFechar,
    EditarAbrir,
    EditarFechar,
    /// "Trocar origem…" de uma origem (chave): abre o seletor de arquivo.
    TrocarOrigem(String),
    /// O arquivo escolhido para a origem (chave).
    OrigemEscolhida(String, Option<PathBuf>),
}

/// Os valores lidos do disco para as áreas citadas de uma origem.
#[derive(Debug, Clone)]
pub struct Leitura {
    pub abas: Vec<String>,
    pub celulas: Vec<(String, Vec<(u32, u32, Value)>)>,
    /// Data do arquivo lido, em segundos desde 1970.
    pub modificado: Option<i64>,
}

/// Uma origem citada pela pasta (lado do destino).
#[derive(Debug, Clone, Default)]
pub(crate) struct Origem {
    /// O caminho como chave (`canal::chave_caminho`), ou `?nome` quando não
    /// deu para resolver (pasta nunca salva e nome relativo).
    pub chave: String,
    pub caminho: Option<PathBuf>,
    /// O arquivo como as fórmulas escrevem (pode haver mais de uma grafia).
    pub nomes: Vec<String>,
    pub areas: Vec<AreaPedida>,
    pub conexao: Option<u64>,
    /// O retrato da outra janela já chegou: os valores estão vivos.
    pub ao_vivo: bool,
}

impl Origem {
    /// O nome curto para a tela: `Vendas.fshz`.
    pub fn arquivo(&self) -> String {
        let nome = self.nomes.first().map(String::as_str).unwrap_or("");
        nome.rsplit(['/', '\\']).next().unwrap_or(nome).to_string()
    }
}

/// Quem assina um arquivo desta janela (lado da origem).
#[derive(Debug, Default)]
pub(crate) struct Assinante {
    pub areas: Vec<AreaPedida>,
    /// Último valor mandado de cada célula, por aba (minúscula).
    pub enviados: HashMap<String, HashMap<(u32, u32), Value>>,
    /// Versão de cada aba (minúscula) na última conferência.
    pub versoes: HashMap<String, u32>,
}

#[derive(Default)]
pub(crate) struct Estado {
    /// O arquivo que esta janela anunciou como aberto (chave).
    registrado: Option<String>,
    /// O `App::caminho` quando o anúncio foi feito (muda ao salvar com outro nome).
    registrado_de: Option<PathBuf>,
    pub(crate) assinantes: HashMap<u64, Assinante>,
    pub(crate) origens: Vec<Origem>,
    /// Versão das abas na última conta das áreas citadas.
    versoes_citadas: Vec<u32>,
    faixa_fechada: bool,
    pub(crate) editar_aberto: bool,
    /// Origens sendo lidas do disco agora.
    lendo: BTreeSet<String>,
    /// Origens já lidas sozinhas do disco nesta sessão (não repete).
    tentou_disco: BTreeSet<String>,
    /// Pasta aberta do disco: nada é lido de outro arquivo (nem de outra
    /// janela) até a pessoa clicar em "Atualizar valores", como o "Habilitar
    /// conteúdo" do Excel. Um arquivo de estranho poderia citar
    /// `'/home/x/[salarios.xlsx]Plan1'!A1` numa aba oculta (#50).
    aguardando_permissao: bool,
    ultima_tentativa: Option<Instant>,
    /// Esta janela anunciou que está apontando (digitando uma fórmula).
    anunciando_apontar: bool,
    /// Esta janela mandou referência à janela deste pid (Enter/Esc vão para lá).
    apontei_para: Option<u32>,
    /// (quem clicou) A fórmula da outra janela, para a barra fx: (arquivo, texto).
    pub(crate) formula_remota: Option<(String, String)>,
    /// (quem digita) Para onde devolver a fórmula: endereço e token de quem clicou.
    responder_a: Option<(String, String)>,
    /// (quem digita) A última fórmula devolvida (manda só o que muda).
    formula_enviada: Option<String>,
}

/// Segundos desde 1970 de agora.
fn agora_segundos() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// "16/10 14:32" no fuso do sistema (no Linux); nos outros, em UTC com o sufixo.
pub(crate) fn data_hora_local(segundos: i64) -> String {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        let s = segundos as libc::time_t;
        // SAFETY: `localtime_r` só escreve na `tm` que passamos; nada é guardado.
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        if !unsafe { libc::localtime_r(&s, &mut tm) }.is_null() {
            return format!("{:02}/{:02} {:02}:{:02}", tm.tm_mday, tm.tm_mon + 1, tm.tm_hour, tm.tm_min);
        }
    }
    let serial = feathersheet_core::celulas::date_serial::timestamp_to_serial(segundos as f64 * 1000.0);
    let (_, mes, dia) = feathersheet_core::celulas::date_serial::serial_to_ymd(serial.floor() as i64);
    let resto = segundos.rem_euclid(86_400);
    format!("{dia:02}/{mes:02} {:02}:{:02} UTC", resto / 3600, resto / 60 % 60)
}

/// As áreas pedidas agrupadas por aba, com os valores de cada uma (sem
/// repetir célula). `ler` devolve `None` quando a aba não existe.
fn valores_por_aba(areas: &[AreaPedida], ler: impl Fn(&str, &Rect) -> Option<Vec<(u32, u32, Value)>>) -> Vec<(String, Vec<(u32, u32, Value)>)> {
    let mut saida: Vec<(String, Vec<(u32, u32, Value)>)> = Vec::new();
    for a in areas {
        let Some(valores) = ler(&a.aba, &a.area) else { continue };
        let posicao = match saida.iter().position(|(n, _)| n.to_lowercase() == a.aba.to_lowercase()) {
            Some(p) => p,
            None => {
                saida.push((a.aba.clone(), Vec::new()));
                saida.len() - 1
            }
        };
        saida[posicao].1.extend(valores);
    }
    for (_, celulas) in &mut saida {
        celulas.sort_by_key(|(r, c, _)| (*r, *c));
        celulas.dedup_by_key(|(r, c, _)| (*r, *c));
    }
    saida
}

/// Lê uma origem do disco (com o leitor de sempre: `.fshz`, `.xlsx`,
/// `.csv`…) e tira só as áreas citadas. Roda fora da thread da tela.
fn ler_do_disco(caminho: &Path, areas: &[AreaPedida]) -> Result<Leitura, String> {
    let bytes = std::fs::read(caminho).map_err(|e| e.to_string())?;
    let modificado = std::fs::metadata(caminho)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|d| i64::try_from(d.as_secs()).ok());
    let (pasta, _) = App::decodificar(caminho, bytes)?;
    let celulas = valores_por_aba(areas, |aba, area| pasta.valores_da_area(aba, area));
    Ok(Leitura { abas: pasta.nomes(), celulas, modificado })
}

/// As abas só com valores para instalar no motor: todas as abas que a
/// origem tem (as não citadas ficam vazias), com as células recebidas.
fn abas_para_instalar(abas: &[String], celulas: &[(String, Vec<(u32, u32, Value)>)]) -> Vec<feathersheet_core::planilha::sheet::Sheet> {
    let mut nomes: Vec<String> = abas.to_vec();
    for (n, _) in celulas {
        if !nomes.iter().any(|x| x.to_lowercase() == n.to_lowercase()) {
            nomes.push(n.clone());
        }
    }
    nomes
        .iter()
        .map(|nome| {
            let vazio = Vec::new();
            let c = celulas.iter().find(|(n, _)| n.to_lowercase() == nome.to_lowercase()).map_or(&vazio, |(_, c)| c);
            motor_vinculo::aba_de_valores(nome, c)
        })
        .collect()
}

/// O retrato das áreas pedidas por quem assina e o ponto de partida das
/// diferenças (lado da origem).
pub(crate) fn retrato_para(pasta: &Workbook, areas: &[AreaPedida]) -> (Json, Assinante) {
    let celulas = valores_por_aba(areas, |aba, area| pasta.valores_da_area(aba, area));
    let mut assinante = Assinante { areas: areas.to_vec(), ..Default::default() };
    for (aba, c) in &celulas {
        let chave = aba.to_lowercase();
        assinante.enviados.insert(chave.clone(), c.iter().map(|(r, col, v)| ((*r, *col), v.clone())).collect());
    }
    for a in areas {
        if let Some(i) = pasta.indice_por_nome(&a.aba) {
            assinante.versoes.insert(a.aba.to_lowercase(), pasta.versao(i));
        }
    }
    (canal::mensagem_retrato(&pasta.nomes(), &celulas), assinante)
}

/// O que mandar a cada assinante: por aba que mudou de versão, só as
/// células das áreas assinadas cujo valor mudou (vazia vai como vazio).
pub(crate) fn publicar(pasta: &Workbook, assinantes: &mut HashMap<u64, Assinante>) -> Vec<(u64, Json)> {
    let mut saidas = Vec::new();
    for (id, assinante) in assinantes.iter_mut() {
        let mut abas: Vec<String> = Vec::new();
        for a in &assinante.areas {
            if !abas.iter().any(|x| x.to_lowercase() == a.aba.to_lowercase()) {
                abas.push(a.aba.clone());
            }
        }
        for aba in abas {
            let chave = aba.to_lowercase();
            let Some(i) = pasta.indice_por_nome(&aba) else { continue };
            let versao = pasta.versao(i);
            if assinante.versoes.get(&chave) == Some(&versao) {
                continue;
            }
            assinante.versoes.insert(chave.clone(), versao);
            let areas: Vec<AreaPedida> = assinante.areas.iter().filter(|a| a.aba.to_lowercase() == chave).cloned().collect();
            let atual: HashMap<(u32, u32), Value> = valores_por_aba(&areas, |aba, area| pasta.valores_da_area(aba, area))
                .into_iter()
                .flat_map(|(_, c)| c)
                .map(|(r, c, v)| ((r, c), v))
                .collect();
            let velho = assinante.enviados.entry(chave).or_default();
            let mut mudou: Vec<(u32, u32, Value)> = atual
                .iter()
                .filter(|(k, v)| velho.get(*k) != Some(*v))
                .map(|((r, c), v)| (*r, *c, v.clone()))
                .collect();
            mudou.extend(velho.keys().filter(|k| !atual.contains_key(*k)).map(|(r, c)| (*r, *c, Value::Empty)));
            *velho = atual;
            if !mudou.is_empty() {
                mudou.sort_by_key(|(r, c, _)| (*r, *c));
                saidas.push((*id, canal::mensagem_valores(&aba, &mudou)));
            }
        }
    }
    saidas
}

fn fluxo_da_fila() -> pelicano::futures::stream::BoxStream<'static, Msg> {
    match canal::tirar_receptor() {
        Some(rx) => rx.map(|c| Msg::Vinculos(Evento::Chegou(c))).boxed(),
        None => pelicano::futures::stream::pending().boxed(),
    }
}

/// A assinatura do Iced: a fila do canal (depois que ele liga) e o tique,
/// só quando há o que vigiar — mais rápido com alguém assinando (as
/// mudanças chegam ao outro lado em menos de 1 s).
pub(super) fn assinatura(app: &App) -> Subscription<Msg> {
    let fila = if canal::fila_criada() { Subscription::run_with_id("feathersheet.vinculos", fluxo_da_fila()) } else { Subscription::none() };
    let periodo = if !app.vinculos.assinantes.is_empty() {
        Some(Duration::from_millis(400))
    } else if app.caminho != app.vinculos.registrado_de {
        Some(Duration::from_millis(300))
    } else if app.workbook.tem_vinculos_externos() || !app.vinculos.origens.is_empty() {
        Some(Duration::from_secs(1))
    } else {
        None
    };
    let tique = periodo.map_or_else(Subscription::none, |p| Subscription::every(p).map(|_| Msg::Vinculos(Evento::Pulso)));
    Subscription::batch([fila, tique])
}

/// Fecha o canal ao sair (registros, conversas e o socket).
pub(super) fn encerrar() {
    canal::encerrar();
}

impl App {
    pub(super) fn evento_vinculos(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Chegou(Chegada::Apontar { caminho, arquivo, aba, referencia, responder }) => {
                if responder.is_some() { self.vinculos.responder_a = responder; }
                self.receber_apontar(&caminho, &arquivo, &aba, &referencia)
            }
            Evento::Chegou(Chegada::FormulaRemota { arquivo, texto }) => {
                self.vinculos.formula_remota = texto.map(|t| (arquivo, t));
                if self.vinculos.formula_remota.is_none() { self.vinculos.apontei_para = None; }
                Task::none()
            }
            Evento::Chegou(Chegada::ApontarFim { confirmar }) => {
                if !self.editor_formula.ativo { return Task::none(); }
                let tarefa = self.update(if confirmar { Msg::Confirmar } else { Msg::Cancelar });
                Task::batch([tarefa, self.trazer_para_frente()])
            }
            Evento::Chegou(chegada) => {
                self.receber_vinculo(chegada);
                Task::none()
            }
            Evento::Pulso => self.pulso_vinculos(),
            Evento::AtualizarValores(alvo) => self.atualizar_valores_externos(alvo),
            Evento::Lido(chave, resultado) => {
                self.vinculos.lendo.remove(&chave);
                match resultado {
                    Ok(leitura) => {
                        let quando = leitura.modificado.unwrap_or_else(agora_segundos);
                        self.aplicar_valores_de_origem(&chave, &leitura.abas, &leitura.celulas, quando);
                        let arquivo = self.origem_vinculo(&chave).map(Origem::arquivo).unwrap_or_default();
                        self.status = trf!("Valores de [{arquivo}] lidos do disco");
                    }
                    Err(e) => {
                        let arquivo = self.origem_vinculo(&chave).map(Origem::arquivo).unwrap_or_default();
                        let texto = trf!("Não deu para ler [{arquivo}]: {e}");
                        self.monitor.registrar(Nivel::Aviso, tr!("Vínculos"), &texto);
                        self.status = texto;
                    }
                }
                Task::none()
            }
            Evento::FaixaFechar => {
                self.vinculos.faixa_fechada = true;
                Task::none()
            }
            Evento::EditarAbrir => {
                self.vinculos.editar_aberto = true;
                // A lista mostra o estado de agora, sem esperar o próximo tique.
                self.vinculos.versoes_citadas.clear();
                self.pulso_vinculos()
            }
            Evento::EditarFechar => {
                self.vinculos.editar_aberto = false;
                Task::none()
            }
            Evento::TrocarOrigem(chave) => {
                let pasta = self
                    .origem_vinculo(&chave)
                    .and_then(|o| o.caminho.clone())
                    .and_then(|p| p.parent().map(Path::to_path_buf))
                    .filter(|p| p.is_dir())
                    .or_else(|| self.caminho.as_ref().and_then(|c| c.parent().map(Path::to_path_buf)))
                    .unwrap_or_else(dialogos::pasta_padrao);
                Task::perform(
                    async move {
                        rfd::AsyncFileDialog::new()
                            .set_directory(pasta)
                            .add_filter(tr!("Planilhas"), &["fshz", "xlsx", "xlsm", "csv", "tsv"])
                            .pick_file()
                            .await
                            .map(|f| f.path().to_path_buf())
                    },
                    move |escolhido| Msg::Vinculos(Evento::OrigemEscolhida(chave.clone(), escolhido)),
                )
            }
            Evento::OrigemEscolhida(_, None) => Task::none(),
            Evento::OrigemEscolhida(chave, Some(novo)) => self.trocar_origem(&chave, &novo),
        }
    }

    /// Outra pasta na janela (abrir, nova, modelo): as origens da anterior
    /// saem e a faixa volta a valer.
    pub(super) fn vinculos_novo_documento(&mut self) {
        for o in self.vinculos.origens.drain(..) {
            if let Some(id) = o.conexao {
                canal::desligar(id);
            }
        }
        self.vinculos.versoes_citadas.clear();
        self.vinculos.tentou_disco.clear();
        self.vinculos.lendo.clear();
        self.vinculos.aguardando_permissao = false;
        self.vinculos.faixa_fechada = false;
        self.vinculos.editar_aberto = false;
        self.vinculos.ultima_tentativa = None;
    }

    pub(crate) fn origem_vinculo(&self, chave: &str) -> Option<&Origem> {
        self.vinculos.origens.iter().find(|o| o.chave == chave)
    }

    /// O caminho de um arquivo citado: absoluto como está; relativo, a partir
    /// da pasta deste arquivo; senão, o último caminho de onde vieram os
    /// valores (a pasta de destino pode ter mudado de lugar).
    pub(crate) fn resolver_origem(&self, arquivo: &str) -> Option<PathBuf> {
        let p = Path::new(arquivo);
        if p.is_absolute() {
            return Some(p.to_path_buf());
        }
        let guardado = self.workbook.livro_externo(arquivo).and_then(|l| l.caminho.clone()).map(PathBuf::from);
        let base = self.caminho.as_ref().or(self.origem.as_ref()).and_then(|c| c.parent()).map(Path::to_path_buf);
        if let Some(b) = &base {
            let junto = b.join(p);
            if junto.exists() {
                return Some(junto);
            }
        }
        if let Some(g) = guardado.as_ref().filter(|g| g.exists()) {
            return Some(g.clone());
        }
        base.map(|b| b.join(p)).or(guardado)
    }

    /// O tique: anúncio do arquivo aberto, publicação para quem assina e a
    /// conferência das origens citadas.
    pub(super) fn pulso_vinculos(&mut self) -> Task<Msg> {
        // 1. Origem: anunciar o arquivo desta janela (ou trocar o anúncio
        //    depois de "Salvar como").
        if self.caminho != self.vinculos.registrado_de {
            if let Some(antiga) = self.vinculos.registrado.take() {
                canal::tirar_registro(&antiga);
            }
            // Quem assinava o arquivo de antes não recebe o novo.
            for (id, _) in self.vinculos.assinantes.drain() {
                canal::desligar(id);
            }
            if let Some(c) = &self.caminho {
                let chave = canal::chave_caminho(c);
                if canal::registrar_aberto(&chave) {
                    self.vinculos.registrado = Some(chave);
                }
            }
            self.vinculos.registrado_de = self.caminho.clone();
        }
        // 2. Origem: o que mudou desde o último tique, a quem assina.
        if !self.vinculos.assinantes.is_empty() {
            for (id, msg) in publicar(&self.workbook, &mut self.vinculos.assinantes) {
                if !canal::enviar(id, &msg) {
                    self.vinculos.assinantes.remove(&id);
                }
            }
        }
        // 3. Destino: as origens citadas.
        if self.workbook.tem_vinculos_externos() || !self.vinculos.origens.is_empty() {
            return self.conferir_origens();
        }
        Task::none()
    }

    /// Refaz a lista de origens quando alguma aba mudou, reassina quem mudou
    /// de áreas, solta quem não é mais citado e tenta achar as fechadas.
    fn conferir_origens(&mut self) -> Task<Msg> {
        // Os pedidos do motor ficam cobertos pelas áreas citadas.
        let _ = self.workbook.externos_pedidos();
        let versoes: Vec<u32> = (0..self.workbook.len()).map(|i| self.workbook.versao(i)).collect();
        let refez = versoes != self.vinculos.versoes_citadas;
        if refez {
            self.vinculos.versoes_citadas = versoes;
            let novas = self.montar_origens();
            for velha in &self.vinculos.origens {
                if !novas.iter().any(|o| o.chave == velha.chave) {
                    if let Some(id) = velha.conexao {
                        canal::desligar(id);
                    }
                }
            }
            self.vinculos.origens = novas;
        }
        if self.vinculos.aguardando_permissao {
            return Task::none();
        }
        let agora = Instant::now();
        let tentar = refez || self.vinculos.ultima_tentativa.is_none_or(|t| agora.duration_since(t) >= INTERVALO_TENTATIVA);
        if !tentar {
            return Task::none();
        }
        self.vinculos.ultima_tentativa = Some(agora);
        let mut tarefas = Vec::new();
        for i in 0..self.vinculos.origens.len() {
            let o = self.vinculos.origens[i].clone();
            let Some(caminho) = o.caminho.clone() else { continue };
            if o.conexao.is_some() {
                continue;
            }
            if let Some(id) = canal::conectar_origem(&o.chave, &o.areas) {
                self.vinculos.origens[i].conexao = Some(id);
                self.monitor.registrar(Nivel::Info, tr!("Vínculos"), &trf!("Conectado a [{}], aberto em outra janela", o.arquivo()));
                continue;
            }
            // Fechada e nunca carregada: lê do disco uma vez, sozinha.
            let sem_valores = o.nomes.iter().all(|n| self.workbook.livro_externo(n).is_none());
            if sem_valores && caminho.is_file() && !self.vinculos.tentou_disco.contains(&o.chave) && !self.vinculos.lendo.contains(&o.chave) {
                self.vinculos.tentou_disco.insert(o.chave.clone());
                tarefas.push(self.ler_origem_do_disco(&o.chave));
            }
        }
        Task::batch(tarefas)
    }

    /// A pasta veio do disco: os vínculos esperam o "Atualizar valores".
    pub(super) fn vinculos_pedir_permissao(&mut self) {
        self.vinculos.aguardando_permissao = true;
    }

    /// As origens citadas agora, com o estado da conexão das de antes.
    fn montar_origens(&mut self) -> Vec<Origem> {
        let mut novas: Vec<Origem> = Vec::new();
        for citada in self.workbook.areas_citadas() {
            let caminho = self.resolver_origem(&citada.arquivo);
            let chave = caminho
                .as_ref()
                .map(|p| canal::chave_caminho(p))
                .unwrap_or_else(|| format!("?{}", chave_arquivo(&citada.arquivo)));
            let posicao = match novas.iter().position(|o| o.chave == chave) {
                Some(p) => p,
                None => {
                    novas.push(Origem { chave, caminho, ..Default::default() });
                    novas.len() - 1
                }
            };
            let o = &mut novas[posicao];
            if !o.nomes.iter().any(|n| mesmo_arquivo(n, &citada.arquivo)) {
                o.nomes.push(citada.arquivo.clone());
            }
            let pedida = AreaPedida { aba: citada.aba, area: citada.area };
            if !o.areas.contains(&pedida) {
                o.areas.push(pedida);
            }
        }
        for o in &mut novas {
            let Some(velha) = self.vinculos.origens.iter().find(|v| v.chave == o.chave) else { continue };
            o.conexao = velha.conexao;
            o.ao_vivo = velha.ao_vivo;
            if let Some(id) = o.conexao {
                if velha.areas != o.areas && !canal::enviar(id, &canal::mensagem_assinar(&o.chave, &o.areas)) {
                    o.conexao = None;
                    o.ao_vivo = false;
                }
            }
        }
        novas
    }

    fn ler_origem_do_disco(&mut self, chave: &str) -> Task<Msg> {
        let Some(o) = self.origem_vinculo(chave) else { return Task::none() };
        let Some(caminho) = o.caminho.clone() else { return Task::none() };
        let areas = o.areas.clone();
        let chave = chave.to_string();
        self.vinculos.lendo.insert(chave.clone());
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || ler_do_disco(&caminho, &areas))
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()))
            },
            move |resultado| Msg::Vinculos(Evento::Lido(chave.clone(), resultado)),
        )
    }

    /// "Atualizar valores": a origem aberta em outra janela manda o retrato
    /// de novo; fechada, lê do disco.
    fn atualizar_valores_externos(&mut self, alvo: Option<String>) -> Task<Msg> {
        self.vinculos.aguardando_permissao = false;
        if self.vinculos.origens.is_empty() {
            self.vinculos.versoes_citadas.clear();
            let _ = self.pulso_vinculos();
        }
        let chaves: Vec<String> = self
            .vinculos
            .origens
            .iter()
            .filter(|o| alvo.as_ref().is_none_or(|a| *a == o.chave))
            .map(|o| o.chave.clone())
            .collect();
        let mut tarefas = Vec::new();
        for chave in chaves {
            let Some(o) = self.origem_vinculo(&chave).cloned() else { continue };
            if let Some(id) = o.conexao {
                if canal::enviar(id, &canal::mensagem_assinar(&o.chave, &o.areas)) {
                    continue;
                }
                if let Some(x) = self.vinculos.origens.iter_mut().find(|x| x.chave == chave) {
                    x.conexao = None;
                    x.ao_vivo = false;
                }
            }
            if o.caminho.as_ref().is_some_and(|c| c.is_file()) {
                if !self.vinculos.lendo.contains(&chave) {
                    tarefas.push(self.ler_origem_do_disco(&chave));
                }
            } else {
                self.status = trf!("Origem [{}] não encontrada: use Editar links para apontar o arquivo", o.arquivo());
            }
        }
        if !tarefas.is_empty() {
            self.status = tr!("Atualizando valores das origens…").into();
        }
        Task::batch(tarefas)
    }

    /// Instala os valores de uma origem (retrato da outra janela ou leitura
    /// do disco) em cada grafia que as fórmulas usam.
    fn aplicar_valores_de_origem(&mut self, chave: &str, abas: &[String], celulas: &[(String, Vec<(u32, u32, Value)>)], quando: i64) {
        let Some(o) = self.origem_vinculo(chave).cloned() else { return };
        let caminho = o.caminho.as_ref().map(|p| p.to_string_lossy().into_owned());
        for nome in &o.nomes {
            self.workbook.instalar_externo_com(nome, abas_para_instalar(abas, celulas), caminho.clone(), Some(quando));
        }
    }

    // ------------------------------------------------------------ apontar noutra janela

    /// Depois de cada mensagem (só a de fora): quem digita uma fórmula anuncia
    /// que aceita referência de outra janela; quem não digita e mudou a
    /// seleção manda a célula a quem está apontando — como no Excel, digitar
    /// "=" numa pasta e clicar na outra.
    pub(super) fn cuidar_apontar(&mut self, selecao_antes: (usize, u32, u32, (u32, u32))) {
        let aceita = self.editor_formula.ativo && self.edicao.starts_with('=')
            && (self.editor_formula.pode_apontar(&self.edicao) || self.editor_formula.modo_apontar);
        if aceita != self.vinculos.anunciando_apontar {
            canal::anunciar_apontar(aceita);
            self.vinculos.anunciando_apontar = aceita;
        }
        // Quem digita: a fórmula volta para a barra fx de quem clicou, a cada
        // mudança; ao confirmar ou cancelar, avisa que acabou.
        if let Some((endereco, token)) = self.vinculos.responder_a.clone() {
            let meu = self.caminho.as_ref().and_then(|c| c.file_name()).map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| tr!("Pasta sem título").to_string());
            let editando = self.editor_formula.ativo && self.edicao.starts_with('=');
            if editando {
                if self.vinculos.formula_enviada.as_deref() != Some(self.edicao.as_str()) {
                    canal::mandar_para(&endereco, &token, &canal::mensagem_formula_remota(&meu, Some(&self.edicao)));
                    self.vinculos.formula_enviada = Some(self.edicao.clone());
                }
            } else {
                canal::mandar_para(&endereco, &token, &canal::mensagem_formula_remota(&meu, None));
                self.vinculos.responder_a = None;
                self.vinculos.formula_enviada = None;
            }
        }
        if self.editor_formula.ativo || (self.folha, self.linha, self.coluna, self.fim_selecao) == selecao_antes {
            return;
        }
        let Some(pid) = canal::quem_aponta() else {
            self.vinculos.apontei_para = None;
            self.vinculos.formula_remota = None;
            return;
        };
        let Some(caminho) = self.caminho.clone() else {
            self.status = tr!("Salve este arquivo para citá-lo na fórmula da outra janela.").into();
            return;
        };
        let arquivo = caminho.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let aba = self.workbook.nomes().get(self.folha).cloned().unwrap_or_default();
        let area = self.area_selecionada();
        let referencia = endereco_area(&area);
        let msg = canal::mensagem_apontar(&caminho.to_string_lossy(), &arquivo, &aba, &referencia);
        if canal::mandar_para_quem_aponta(&msg) {
            self.vinculos.apontei_para = Some(pid);
            self.status = trf!("[{arquivo}]{aba}!{referencia} vai para a fórmula da outra janela (Enter confirma)");
        }
    }

    /// Enter ou Esc nesta janela depois de apontar para a outra: confirma
    /// ou cancela a fórmula lá. `true` quando a tecla foi usada para isso.
    pub(super) fn tecla_para_quem_aponta(&mut self, confirmar: bool) -> bool {
        if self.editor_formula.ativo { return false; }
        let Some(pid) = self.vinculos.apontei_para else { return false };
        if canal::quem_aponta() != Some(pid) {
            self.vinculos.apontei_para = None;
            return false;
        }
        self.vinculos.apontei_para = None;
        self.vinculos.formula_remota = None;
        canal::mandar_para_quem_aponta(&canal::mensagem_apontar_fim(confirmar))
    }

    /// Chegou a célula clicada noutra janela: entra na fórmula em edição.
    fn receber_apontar(&mut self, caminho: &str, arquivo: &str, aba: &str, referencia: &str) -> Task<Msg> {
        if !self.editor_formula.ativo || !self.edicao.starts_with('=') { return Task::none(); }
        let texto = texto_referencia_externa(self.caminho.as_deref(), Path::new(caminho), arquivo, aba, referencia);
        if self.editor_formula.referencia_pronta(&mut self.edicao, &texto) {
            self.status = trf!("{texto} — Enter confirma, nesta janela ou na outra");
        }
        Task::none()
    }

    /// A janela principal volta para a frente (Enter dado na outra).
    fn trazer_para_frente(&self) -> Task<Msg> {
        self.janelas.principal.map_or_else(Task::none, |id| Task::window_of(id, pelicano::WindowCmd::Focus))
    }

    /// O que chegou pelo canal.
    pub(crate) fn receber_vinculo(&mut self, chegada: Chegada) {
        match chegada {
            Chegada::Assinar { conexao, caminho, areas } => {
                // O registro pode estar atrás (esta janela já tem outro arquivo).
                if self.vinculos.registrado.as_deref() != Some(caminho.as_str()) {
                    canal::desligar(conexao);
                    return;
                }
                let (retrato, assinante) = retrato_para(&self.workbook, &areas);
                if canal::enviar(conexao, &retrato) {
                    self.vinculos.assinantes.insert(conexao, assinante);
                }
            }
            Chegada::AssinanteSaiu(id) => {
                self.vinculos.assinantes.remove(&id);
            }
            Chegada::Retrato { conexao, abas, celulas } => {
                let Some(o) = self.vinculos.origens.iter_mut().find(|o| o.conexao == Some(conexao)) else { return };
                o.ao_vivo = true;
                let (chave, arquivo) = (o.chave.clone(), o.arquivo());
                self.aplicar_valores_de_origem(&chave, &abas, &celulas, agora_segundos());
                self.status = trf!("Valores de [{arquivo}] ao vivo: aberto em outra janela");
            }
            Chegada::Valores { conexao, aba, celulas } => {
                let Some(o) = self.vinculos.origens.iter().find(|o| o.conexao == Some(conexao)) else { return };
                let nomes = o.nomes.clone();
                let quando = Some(agora_segundos());
                for nome in &nomes {
                    self.workbook.atualizar_externo_em(nome, &aba, &celulas, quando);
                }
            }
            Chegada::OrigemFechou(conexao) => {
                let Some(o) = self.vinculos.origens.iter_mut().find(|o| o.conexao == Some(conexao)) else { return };
                o.conexao = None;
                o.ao_vivo = false;
                let arquivo = o.arquivo();
                // Volta a tentar já no próximo tique (pode ter sido só um "Salvar como").
                self.vinculos.ultima_tentativa = None;
                self.vinculos.faixa_fechada = false;
                self.status = trf!("[{arquivo}] foi fechado: ficam os últimos valores");
            }
            // Tratadas no `evento_vinculos` (precisam devolver tarefa).
            Chegada::Apontar { .. } | Chegada::ApontarFim { .. } | Chegada::FormulaRemota { .. } => {}
        }
    }

    /// "Trocar origem": as fórmulas passam a citar o arquivo escolhido. Na
    /// mesma pasta deste arquivo vai só o nome; fora dela, o caminho inteiro.
    fn trocar_origem(&mut self, chave: &str, novo: &Path) -> Task<Msg> {
        let Some(o) = self.origem_vinculo(chave).cloned() else { return Task::none() };
        let mesma_pasta = self.caminho.as_ref().and_then(|c| c.parent()).is_some_and(|p| Some(p) == novo.parent());
        let escrito = if mesma_pasta {
            novo.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
        } else {
            novo.to_string_lossy().into_owned()
        };
        if escrito.is_empty() {
            return Task::none();
        }
        let mut total = 0;
        for nome in &o.nomes {
            total += self.workbook.trocar_origem_externa(nome, &escrito);
        }
        if let Some(id) = o.conexao {
            canal::desligar(id);
        }
        self.vinculos.origens.retain(|x| x.chave != chave);
        self.vinculos.tentou_disco.remove(chave);
        self.vinculos.versoes_citadas.clear();
        self.alterado = true;
        self.status = trf!("Origem trocada para [{escrito}] em {total} fórmulas");
        self.monitor.registrar(Nivel::Info, tr!("Vínculos"), &self.status.clone());
        // A nova origem: conecta (aberta) ou lê do disco já.
        let tarefa = self.pulso_vinculos();
        let nova = canal::chave_caminho(novo);
        let ler = self
            .origem_vinculo(&nova)
            .filter(|x| x.conexao.is_none() && !self.vinculos.lendo.contains(&x.chave))
            .map(|x| x.chave.clone());
        match ler {
            Some(c) => Task::batch([tarefa, self.ler_origem_do_disco(&c)]),
            None => tarefa,
        }
    }
}

/// Como a fórmula escreve a célula de outro arquivo: `[Vendas.fshz]Dados!A1`
/// na mesma pasta e sem espaço; com espaço ou de outra pasta, entre aspas
/// (`'/x/[Vendas.fshz]Minha aba'!A1`), como o Excel.
pub(crate) fn texto_referencia_externa(meu: Option<&Path>, origem: &Path, arquivo: &str, aba: &str, referencia: &str) -> String {
    let mesma_pasta = meu.and_then(Path::parent).is_some_and(|p| Some(p) == origem.parent());
    let pasta = if mesma_pasta {
        String::new()
    } else {
        origem.parent().map(|p| format!("{}/", p.to_string_lossy().trim_end_matches(['/', '\\']))).unwrap_or_default()
    };
    let simples = |t: &str| !t.is_empty() && t.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.');
    if pasta.is_empty() && simples(arquivo) && simples(aba) {
        format!("[{arquivo}]{aba}!{referencia}")
    } else {
        format!("'{}'!{referencia}", format!("{pasta}[{arquivo}]{aba}").replace('\'', "''"))
    }
}
