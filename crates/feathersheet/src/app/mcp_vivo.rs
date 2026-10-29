//! MCP ao vivo: o `feathersheet-mcp` que a IA abre repassa cada chamada de
//! ferramenta para cá, pelo canal local de `feathersheet_mcp::ao_vivo` (só
//! nesta máquina). A thread do canal entrega o pedido ao `update` por uma
//! subscription e espera a resposta: as ferramentas rodam na mesma pasta da
//! tela, e cada chamada é um Ctrl+Z.
//!
//! O que só existe com a janela aberta — selecionar, comandos da faixa e
//! atalhos, foto da janela — é implementado aqui, no [`HospedeApp`].
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::time::Duration;

use feathersheet_core::celulas::refs::Rect;
use feathersheet_mcp::{ao_vivo, hospede::Hospede, rpc};
use pelicano::futures::{channel::mpsc as canal, StreamExt};
use serde_json::{json, Value as Json};

use super::*;

/// Um pedido vindo do canal, com o caminho de volta da resposta.
#[derive(Clone)]
pub struct PedidoMcp(Arc<(Json, Mutex<Option<mpsc::Sender<Json>>>)>);

impl std::fmt::Debug for PedidoMcp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PedidoMcp({})", self.0 .0.get("method").and_then(Json::as_str).unwrap_or("?"))
    }
}

impl PedidoMcp {
    fn msg(&self) -> &Json { &self.0 .0 }
    fn responder(&self, resposta: Json) {
        if let Some(tx) = self.0 .1.lock().ok().and_then(|mut g| g.take()) {
            let _ = tx.send(resposta);
        }
    }
    fn responder_ferramenta(&self, conteudo: Json, erro: bool) {
        let id = self.msg().get("id").cloned().unwrap_or(Json::Null);
        self.responder(json!({ "jsonrpc": "2.0", "id": id, "result": { "content": conteudo, "isError": erro } }));
    }
}

static RECEPTOR: OnceLock<Mutex<Option<canal::UnboundedReceiver<PedidoMcp>>>> = OnceLock::new();
static ENVIO: OnceLock<Mutex<canal::UnboundedSender<PedidoMcp>>> = OnceLock::new();

/// Configurações → IA (MCP): ligado e somente leitura. Lidos pela thread do
/// canal a cada pedido; desligado, ninguém chega à planilha.
static ATIVO: AtomicBool = AtomicBool::new(false);
static SO_LEITURA: AtomicBool = AtomicBool::new(false);
/// O canal está escutando (o ponto de conexão existe).
static ESCUTANDO: AtomicBool = AtomicBool::new(false);
/// Pedidos atendidos nesta sessão (mostrado nas Configurações).
static PEDIDOS: AtomicU64 = AtomicU64::new(0);

/// Quanto a thread do canal espera o aplicativo (abrir arquivo grande demora).
const ESPERA: Duration = Duration::from_secs(120);

/// O app está ocupado (macro ou cálculo em fundo, com a pasta noutra thread):
/// responde já, dizendo por quê, em vez de deixar o pedido cair.
pub(super) fn recusar_ocupado(pedido: &PedidoMcp, texto: &str) {
    let id = pedido.msg().get("id").cloned().unwrap_or(Json::Null);
    pedido.responder(recusa(id, texto));
}

fn recusa(id: Json, texto: &str) -> Json {
    json!({ "jsonrpc": "2.0", "id": id, "result": { "isError": true, "content": [{ "type": "text", "text": texto }] } })
}

pub(super) fn somente_leitura(sim: bool) { SO_LEITURA.store(sim, Ordering::Relaxed); }
pub(super) fn pedidos() -> u64 { PEDIDOS.load(Ordering::Relaxed) }
pub(super) fn escutando() -> bool { ESCUTANDO.load(Ordering::Relaxed) }

/// Liga ou desliga o canal (abertura do app e Configurações). Ligar cria o
/// ponto de conexão local; desligar o remove, e um pedido que ainda chegue é
/// recusado. Fica desligado nos testes, na captura e com `FEATHERSHEET_MCP=0`;
/// um segundo FeatherSheet aberto não toma o canal do primeiro.
pub(super) fn ligar(ativo: bool) -> String {
    ATIVO.store(ativo, Ordering::Relaxed);
    if !ativo {
        if ESCUTANDO.swap(false, Ordering::Relaxed) { ao_vivo::encerrar(); }
        return tr!("MCP desligado: nenhuma IA alcança a planilha.").into();
    }
    if std::env::var("FEATHERSHEET_MCP").is_ok_and(|v| v == "0")
        || std::env::var_os("FEATHERSHEET_CAPTURAR").is_some() || cfg!(test) {
        return tr!("MCP indisponível neste modo de execução.").into();
    }
    if ESCUTANDO.load(Ordering::Relaxed) {
        return trf!("MCP ao vivo escutando em {}", ao_vivo::onde_escuta().display());
    }
    if ENVIO.get().is_none() {
        let (tx, rx) = canal::unbounded::<PedidoMcp>();
        let _ = RECEPTOR.set(Mutex::new(Some(rx)));
        let _ = ENVIO.set(Mutex::new(tx));
    }
    let tratar = move |msg: Json| -> Option<Json> {
        let id = msg.get("id")?.clone();
        if msg.get("method").and_then(Json::as_str) == Some(ao_vivo::METODO_INSTANCIA) {
            return Some(json!({ "jsonrpc": "2.0", "id": id, "result": { "ok": false } }));
        }
        if !ATIVO.load(Ordering::Relaxed) {
            return Some(recusa(id, "O MCP está desligado no FeatherSheet (Configurações → IA (MCP))."));
        }
        if SO_LEITURA.load(Ordering::Relaxed) && msg.get("method").and_then(Json::as_str) == Some("tools/call") {
            let nome = msg["params"]["name"].as_str().unwrap_or("");
            if !feathersheet_mcp::permissoes::so_le(nome, &msg["params"]["arguments"]) {
                return Some(recusa(id, &format!("{nome}: o FeatherSheet está em somente leitura para a IA (Configurações → IA (MCP)).")));
            }
        }
        PEDIDOS.fetch_add(1, Ordering::Relaxed);
        let (rtx, rrx) = mpsc::channel();
        ENVIO.get()?.lock().ok()?.unbounded_send(PedidoMcp(Arc::new((msg, Mutex::new(Some(rtx)))))).ok()?;
        Some(rrx.recv_timeout(ESPERA).unwrap_or_else(|_| recusa(id,
            "O FeatherSheet não respondeu a tempo (ocupado ou com um diálogo aberto).")))
    };
    match ao_vivo::servir(tratar) {
        Ok(true) => {
            ESCUTANDO.store(true, Ordering::Relaxed);
            trf!("MCP ao vivo escutando em {}", ao_vivo::onde_escuta().display())
        }
        Ok(false) => tr!("MCP ao vivo: outro FeatherSheet já atende o canal").into(),
        Err(e) => trf!("MCP ao vivo indisponível: {e}"),
    }
}

pub(super) fn encerrar() {
    if ESCUTANDO.swap(false, Ordering::Relaxed) { ao_vivo::encerrar(); }
}

fn fluxo() -> pelicano::futures::stream::BoxStream<'static, Msg> {
    match RECEPTOR.get().and_then(|m| m.lock().ok()?.take()) {
        Some(rx) => rx.map(Msg::Mcp).boxed(),
        None => pelicano::futures::stream::pending().boxed(),
    }
}

pub(super) fn assinatura() -> Subscription<Msg> {
    // O receptor é criado na primeira vez que o MCP liga; a assinatura
    // segue de pé mesmo se ele for desligado depois (o canal só recusa).
    if RECEPTOR.get().is_some() { Subscription::run_with_id("feathersheet.mcp", fluxo()) } else { Subscription::none() }
}

/// (nome, o que faz) — o `sheet_comando listar`.
const COMANDOS: &[(&str, &str)] = &[
    ("faixa", "abre uma aba da faixa: args {aba: inicio|inserir|formulas|dados|exibir|macros|ferramentas}"),
    ("tecla", "aperta um atalho como se fosse no teclado: args {combo: \"F9\", \"Ctrl+Z\", \"Ctrl+/\", \"Shift+F9\", \"Escape\"…}"),
    ("menu", "executa um comando dos menus pelo rótulo (veja os rótulos com menus): args {texto: \"Inserir linha\"}"),
    ("menus", "lista os rótulos aceitos por menu, com o caminho: args {filtro?: \"congelar\"}"),
    ("editar", "digita na célula ativa e confirma com Enter, pelo mesmo caminho da barra fx: args {texto}"),
    ("rastrear_precedentes", "Fórmulas → Rastrear precedentes (um nível por chamada)"),
    ("rastrear_dependentes", "Fórmulas → Rastrear dependentes"),
    ("remover_setas", "Fórmulas → Remover setas"),
    ("calculo", "Opções de cálculo: args {manual: true|false}"),
    ("calcular_agora", "F9"),
    ("calcular_planilha", "Shift+F9"),
    ("menu_formulas", "abre um menu da aba Fórmulas: args {nome: Lógica|Texto|Mais|Recentes|Nomes|Cálculo…}"),
    ("criar_da_selecao", "abre Criar nomes a partir da seleção"),
    ("criar_da_selecao_borda", "marca/desmarca uma borda: args {lado: 0 topo, 1 esquerda, 2 base, 3 direita}"),
    ("criar_da_selecao_confirmar", "botão Criar"),
    ("fechar", "fecha o menu ou diálogo aberto (Esc)"),
    ("congelar", "congela painéis na aba ativa: args {linhas: 1, colunas: 0} (até 30 × 16; 0 e 0 descongela)"),
    ("selecionar_derramada", "Ctrl+/: seleciona a matriz derramada da célula ativa"),
    ("inspecao_abrir", "abre a Janela de inspeção"),
    ("inspecao_adicionar", "Adicionar inspeção (a seleção)"),
    ("inspecao_ir", "vai até a inspeção: args {indice}"),
    ("inspecao_excluir", "exclui a inspeção: args {indice}"),
    ("avaliar_abrir", "abre Avaliar fórmula na célula ativa"),
    ("avaliar_passo", "botão Avaliar (calcula o trecho sublinhado)"),
    ("avaliar_entrar", "botão Entrar (abre a fórmula da célula sublinhada)"),
    ("avaliar_sair", "botão Sair"),
    ("avaliar_reiniciar", "botão Reiniciar"),
    ("mostrar_formulas", "liga/desliga Mostrar fórmulas"),
    ("explicar_erro", "abre o diagnóstico do erro da célula ativa"),
    ("gerenciador_nomes", "abre o Gerenciador de nomes"),
    ("autosoma", "AutoSoma"),
    ("desfazer", "Ctrl+Z"),
    ("refazer", "Ctrl+Y"),
];

/// "Ctrl+Shift+F9" → tecla e modificadores, como o teclado entregaria.
fn tecla(combo: &str) -> Option<(keyboard::Key, keyboard::Modifiers)> {
    use keyboard::key::Named;
    let mut mods = keyboard::Modifiers::empty();
    let partes: Vec<&str> = combo.split('+').map(str::trim).collect();
    // "Ctrl++" termina em vazio: a tecla é o próprio "+".
    let (nome, prefixo) = match partes.split_last()? {
        (&"", resto) => ("+", &resto[..resto.len().saturating_sub(1)]),
        (n, resto) => (*n, resto),
    };
    for m in prefixo {
        match m.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => mods |= keyboard::Modifiers::CTRL,
            "shift" => mods |= keyboard::Modifiers::SHIFT,
            "alt" => mods |= keyboard::Modifiers::ALT,
            _ => return None,
        }
    }
    let nomeada = match nome.to_ascii_lowercase().as_str() {
        "enter" => Some(Named::Enter), "escape" | "esc" => Some(Named::Escape), "tab" => Some(Named::Tab),
        "delete" | "del" => Some(Named::Delete), "backspace" => Some(Named::Backspace), "space" => Some(Named::Space),
        "up" | "arrowup" => Some(Named::ArrowUp), "down" | "arrowdown" => Some(Named::ArrowDown),
        "left" | "arrowleft" => Some(Named::ArrowLeft), "right" | "arrowright" => Some(Named::ArrowRight),
        "home" => Some(Named::Home), "end" => Some(Named::End), "pageup" => Some(Named::PageUp), "pagedown" => Some(Named::PageDown),
        "f1" => Some(Named::F1), "f2" => Some(Named::F2), "f3" => Some(Named::F3), "f4" => Some(Named::F4),
        "f5" => Some(Named::F5), "f6" => Some(Named::F6), "f7" => Some(Named::F7), "f8" => Some(Named::F8),
        "f9" => Some(Named::F9), "f10" => Some(Named::F10), "f11" => Some(Named::F11), "f12" => Some(Named::F12),
        _ => None,
    };
    let key = match nomeada {
        Some(n) => keyboard::Key::Named(n),
        None if nome.chars().count() == 1 => keyboard::Key::Character(nome.to_lowercase().into()),
        None => return None,
    };
    Some((key, mods))
}

fn auxiliar_por_nome(nome: &str) -> Option<Auxiliar> {
    Some(match nome {
        "inspecao" => Auxiliar::Inspecao, "ajuda" => Auxiliar::Ajuda, "configuracoes" => Auxiliar::Configuracoes,
        "monitor" => Auxiliar::Monitor, "macros" => Auxiliar::Macros, "formatar" => Auxiliar::FormatarCelulas,
        "imprimir" => Auxiliar::Imprimir, "openquery" => Auxiliar::OpenQuery,
        "dividir" => Auxiliar::DividirTexto, "duplicados" => Auxiliar::RemoverDuplicados,
        "condicional" => Auxiliar::Condicional, "formulario" => Auxiliar::Formulario,
        _ => return None,
    })
}

const AUXILIARES: [(Auxiliar, &str); 12] = [
    (Auxiliar::Configuracoes, "Configurações"), (Auxiliar::Ajuda, "Ajuda"), (Auxiliar::Monitor, "Monitor"),
    (Auxiliar::Macros, "Editor de macros"), (Auxiliar::FormatarCelulas, "Formatar células"),
    (Auxiliar::DividirTexto, "Dividir texto"), (Auxiliar::RemoverDuplicados, "Remover duplicados"),
    (Auxiliar::Imprimir, "Imprimir"), (Auxiliar::OpenQuery, "OpenQuery"), (Auxiliar::Inspecao, "Janela de inspeção"),
    (Auxiliar::Condicional, "Formatação condicional"), (Auxiliar::Formulario, "Formulário da macro"),
];

/// O `App` visto pelas ferramentas. As tarefas que um comando gera (abrir
/// janela, gravar) saem daqui para o `update` devolver ao Iced.
pub(super) struct HospedeApp<'a> {
    app: &'a mut App,
    tarefas: Vec<Task<Msg>>,
}

impl HospedeApp<'_> {
    fn enviar(&mut self, msg: Msg) {
        let t = self.app.update(msg);
        self.tarefas.push(t);
    }

    /// Procura o rótulo nos menus (igual primeiro, depois "contém").
    /// Os itens de menu que o `menu` aciona, um por linha, com o caminho
    /// ("Exibir › Congelar › 1 linha"); `filtro` corta por trecho do caminho.
    fn rotulos_de_menu(&self, filtro: &str) -> String {
        fn juntar(app: &App, itens: &[menu_nativo::Item], caminho: &str, filtro: &str, saida: &mut Vec<String>) {
            for item in itens {
                let aqui = format!("{caminho} › {}", item.rotulo.trim());
                if item.acao.as_ref().is_some_and(|a| app.evento_menu(a).is_some()) && aqui.to_lowercase().contains(filtro) {
                    saida.push(aqui.clone());
                }
                juntar(app, &item.filhos, &aqui, filtro, saida);
            }
        }
        let filtro = filtro.trim().to_lowercase();
        let mut saida = Vec::new();
        for m in &menu_nativo::catalogo().menus {
            juntar(self.app, &m.itens, m.titulo.trim(), &filtro, &mut saida);
        }
        if saida.is_empty() { format!("Nenhum item de menu com \"{filtro}\".") } else { saida.join("\n") }
    }

    fn menu(&mut self, texto: &str) -> Result<String, String> {
        fn achar(app: &App, itens: &[menu_nativo::Item], alvo: &str, exato: bool) -> Option<(String, Msg)> {
            for item in itens {
                let rotulo = item.rotulo.trim().to_lowercase();
                let bate = if exato { rotulo == alvo } else { rotulo.contains(alvo) };
                if bate {
                    if let Some(msg) = item.acao.as_ref().and_then(|a| app.evento_menu(a)) {
                        return Some((item.rotulo.clone(), msg));
                    }
                }
                if let Some(r) = achar(app, &item.filhos, alvo, exato) { return Some(r); }
            }
            None
        }
        let alvo = texto.trim().to_lowercase();
        let menus = &menu_nativo::catalogo().menus;
        let achado = [true, false].into_iter().find_map(|exato|
            menus.iter().find_map(|m| achar(self.app, &m.itens, &alvo, exato)));
        let (rotulo, msg) = achado.ok_or_else(|| format!("Nenhum comando de menu com \"{texto}\". Veja os rótulos com comando=\"menus\"."))?;
        self.enviar(msg);
        Ok(rotulo)
    }
}

impl Hospede for HospedeApp<'_> {
    fn pasta(&self) -> &feathersheet_core::planilha::workbook::Workbook { &self.app.workbook }
    fn pasta_mut(&mut self) -> &mut feathersheet_core::planilha::workbook::Workbook { &mut self.app.workbook }
    fn aba_ativa(&self) -> usize { self.app.folha }
    fn definir_aba_ativa(&mut self, indice: usize) {
        if indice != self.app.folha { self.enviar(Msg::Aba(indice)); }
    }
    fn caminho(&self) -> Option<PathBuf> { self.app.caminho.clone() }
    fn marcar_alterado(&mut self) {
        // A IA mexeu na pasta: mostrar a planilha, não a tela inicial.
        self.app.alterado = true;
        self.app.em_tela_inicio = false;
    }
    fn alterado(&self) -> bool { self.app.alterado }
    fn ao_vivo(&self) -> bool { true }
    // Depois da resposta: a pasta ainda é lida para montar o texto dela.
    fn calcular_muitas(&mut self) { self.enviar(Msg::CalcularEmFundo); }

    fn abrir(&mut self, caminho: &Path) -> Result<String, String> {
        if self.app.alterado {
            return Err("A pasta na tela tem alterações não salvas: salve antes (sheet_salvar) ou pergunte ao usuário.".into());
        }
        if !caminho.is_file() { return Err(format!("Arquivo não encontrado: {}", caminho.display())); }
        self.app.em_tela_inicio = false;
        self.app.carregando = true;
        self.app.status = format!("Abrindo {}", caminho.display());
        self.tarefas.push(App::carregar_em_background(caminho.to_path_buf(), self.app.config.separador_csv()));
        Ok(format!("Abrindo {} no aplicativo; confira com sheet_resumo em seguida.", caminho.display()))
    }

    fn salvar(&mut self, caminho: Option<&Path>) -> Result<String, String> {
        let destino = caminho.map(Path::to_path_buf).or_else(|| self.app.caminho.clone())
            .ok_or("A pasta da tela ainda não tem arquivo: informe `caminho` (.fshz).")?;
        if destino.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("fshz")) {
            let t = self.app.gravar(destino.clone());
            self.tarefas.push(t);
            Ok(format!("Gravando {} em segundo plano (o título perde o * quando terminar).", destino.display()))
        } else {
            feathersheet_mcp::arquivo::salvar(&self.app.workbook, self.app.folha, &destino)?;
            Ok(format!("Exportado para {} (o arquivo da tela não muda).", destino.display()))
        }
    }

    fn selecao(&self) -> Option<Rect> { Some(self.app.area_selecionada()) }

    fn selecionar(&mut self, area: Rect) -> Result<(), String> {
        let texto = feathersheet_mcp::texto::nome_area(&area);
        self.enviar(Msg::IrPara(texto));
        Ok(())
    }

    fn comandos(&self) -> Vec<(&'static str, &'static str)> { COMANDOS.to_vec() }

    fn refluir_estrutura(&mut self, aba: usize, coluna: bool, at: u32, quantas: u32, removendo: bool) -> Result<(), String> {
        // As mesmas duas etapas do botão Inserir/Excluir da faixa.
        let tabela = tabelas::refluir(&mut self.app.workbook, aba, coluna, at, quantas, removendo);
        let mescla = mesclas::refluir(&mut self.app.workbook, aba, coluna, at, quantas, removendo);
        // Como no botão: o Ctrl+Z não volta para antes da estrutura (#46).
        if let Some(a) = self.app.workbook.aba_mut(aba) { a.sheet.esquecer_historico(); }
        if aba == self.app.folha { self.app.trocar_selecao(self.app.linha, self.app.coluna); }
        match (tabela, mescla) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(e), Ok(())) => Err(format!("tabela requer revisão: {e}")),
            (Ok(()), Err(e)) => Err(format!("mesclagem requer revisão: {e}")),
            (Err(a), Err(b)) => Err(format!("tabela: {a}; mesclagem: {b}")),
        }
    }
    fn reflui_tabelas(&self) -> bool { true }

    /// O mesmo caminho das setas do cabeçalho (`tabelas::filtrar`): o
    /// AutoFiltro guardado na aba, linhas escondidas e um Ctrl+Z.
    fn filtrar(&mut self, aba: usize, area: Rect, acao: &str, coluna: Option<u32>, criterio: Option<Json>) -> Result<String, String> {
        use feathersheet_core::celulas::refs::{cell_name, col_name};
        let w = &mut self.app.workbook;
        let area_da = |t: &Json| -> Option<Rect> {
            let a = &t["area"];
            let n = |k: &str| a[k].as_u64().and_then(|v| u32::try_from(v).ok());
            Some(Rect::new(n("r0")?, n("c0")?, n("r1")?, n("c1")?))
        };
        let nome = endereco_area;
        let existente = tabelas::sob_qualquer(w, aba, area.r0, area.c0);
        let texto = match acao {
            "listar" => match existente {
                None => format!("Sem AutoFiltro nem tabela em {}.", cell_name(area.r0, area.c0)),
                Some(t) => {
                    let a = area_da(&t).ok_or("Área do filtro inválida.")?;
                    let filtros = t["filtros"].as_array().cloned().unwrap_or_default();
                    let linhas: Vec<String> = filtros.iter().map(|f| format!("  coluna {}: {}",
                        col_name(a.c0 + f["col"].as_u64().unwrap_or(0) as u32), f["criterio"])).collect();
                    format!("{} em {}{}", if t["soFiltro"] == true { "AutoFiltro" } else { "Tabela" }, nome(&a),
                        if linhas.is_empty() { ", sem critérios.".into() } else { format!(":\n{}", linhas.join("\n")) })
                }
            },
            "aplicar" => {
                let coluna = coluna.ok_or("Falta `coluna`.")?;
                let criterio = criterio.ok_or("Falta `criterio`.")?;
                let t = match existente {
                    Some(t) => t,
                    None => {
                        tabelas::alternar_auto_filtro(w, aba, area.clone(), area.r0, area.c0)?;
                        tabelas::sob_qualquer(w, aba, area.r0, area.c0).ok_or("Não consegui ligar o AutoFiltro nesse intervalo.")?
                    }
                };
                let a = area_da(&t).ok_or("Área do filtro inválida.")?;
                if coluna < a.c0 || coluna > a.c1 {
                    return Err(format!("A coluna {} está fora do filtro ({}).", col_name(coluna), nome(&a)));
                }
                let id = t["id"].as_str().unwrap_or("").to_string();
                let escondidas = tabelas::filtrar(w, aba, &id, coluna - a.c0, Some(criterio.clone()))?;
                format!("Filtro em {} (coluna {}: {criterio}): {escondidas} linha(s) escondida(s). Um Ctrl+Z desfaz.", nome(&a), col_name(coluna))
            }
            "limpar" => {
                let t = existente.ok_or("Não há AutoFiltro nem tabela nesse intervalo.")?;
                let a = area_da(&t).ok_or("Área do filtro inválida.")?;
                let id = t["id"].as_str().unwrap_or("").to_string();
                match coluna {
                    Some(c) if c >= a.c0 && c <= a.c1 => {
                        tabelas::filtrar(w, aba, &id, c - a.c0, None)?;
                        format!("Critério da coluna {} removido; as linhas dela voltaram.", col_name(c))
                    }
                    Some(c) => return Err(format!("A coluna {} está fora do filtro ({}).", col_name(c), nome(&a))),
                    None => format!("{} critério(s) removido(s) de {}; todas as linhas à vista.", tabelas::limpar_todos_filtros(w, aba, &id)?, nome(&a)),
                }
            }
            "remover" => {
                let t = existente.ok_or("Não há AutoFiltro nesse intervalo.")?;
                if t["soFiltro"] != true {
                    return Err("Isso é uma tabela (Ctrl+T): o filtro faz parte dela. Use acao=limpar para tirar os critérios.".into());
                }
                let id = t["id"].as_str().unwrap_or("").to_string();
                tabelas::limpar_todos_filtros(w, aba, &id)?;
                tabelas::alternar_auto_filtro(w, aba, area.clone(), area.r0, area.c0)?
            }
            outra => return Err(format!("`acao` desconhecida: {outra}. Use aplicar, limpar, remover ou listar.")),
        };
        if acao != "listar" && aba == self.app.folha {
            self.app.trocar_selecao(self.app.linha, self.app.coluna);
        }
        Ok(texto)
    }

    fn comando(&mut self, nome: &str, args: &Json) -> Result<String, String> {
        use formulas_ui::Evento as F;
        use inspecao_ui::Evento as I;
        let texto = |campo: &str| args.get(campo).and_then(Json::as_str).unwrap_or("").to_string();
        let numero = |campo: &str| args.get(campo).and_then(Json::as_u64).unwrap_or(0) as usize;
        let antes = self.app.status.clone();
        let mut extra = String::new();
        match nome {
            "faixa" => {
                let aba = texto("aba");
                let chave = aba.to_lowercase().replace('ó', "o").replace('í', "i").replace('ã', "a");
                if !["inicio", "inserir", "formulas", "dados", "revisao", "exibir", "macros", "ferramentas"].contains(&chave.as_str()) {
                    return Err(format!("Aba da faixa desconhecida: {aba}"));
                }
                self.enviar(Msg::AbaFaixa(AbaFaixa::de_chave_persistencia(&chave)));
            }
            "tecla" => {
                let combo = texto("combo");
                let (key, mods) = tecla(&combo).ok_or_else(|| format!("Atalho não reconhecido: \"{combo}\"."))?;
                let id = self.app.janelas.principal.ok_or("Janela principal fechada")?;
                let t = self.app.processar_teclado(id, key, mods);
                self.tarefas.push(t);
            }
            "menu" => extra = format!(" (menu: {})", self.menu(&texto("texto"))?),
            "menus" => return Ok(self.rotulos_de_menu(&texto("filtro"))),
            "editar" => {
                self.enviar(Msg::Editar(texto("texto")));
                self.enviar(Msg::EnviarEditor);
            }
            "rastrear_precedentes" => self.enviar(Msg::Formulas(F::RastrearPrecedentes)),
            "rastrear_dependentes" => self.enviar(Msg::Formulas(F::RastrearDependentes)),
            "remover_setas" => self.enviar(Msg::Formulas(F::RemoverSetas)),
            "calculo" => self.enviar(Msg::Formulas(F::Manual(args.get("manual").and_then(Json::as_bool).unwrap_or(false)))),
            "calcular_agora" => self.enviar(Msg::Formulas(F::CalcularAgora)),
            "calcular_planilha" => self.enviar(Msg::Formulas(F::CalcularPlanilha)),
            "menu_formulas" => {
                let menu = formulas_ui::menu_por_nome(&texto("nome")).ok_or("Menu desconhecido")?;
                self.app.faixa_ativa = AbaFaixa::Formulas;
                self.enviar(Msg::Formulas(F::Menu(menu)));
            }
            "criar_da_selecao" => self.enviar(Msg::Formulas(F::CriarAbrir)),
            "criar_da_selecao_borda" => self.enviar(Msg::Formulas(F::CriarBorda(numero("lado").min(3) as u8))),
            "criar_da_selecao_confirmar" => self.enviar(Msg::Formulas(F::CriarConfirmar)),
            "fechar" => {
                let id = self.app.janelas.principal.ok_or("Janela principal fechada")?;
                let t = self.app.processar_teclado(id, keyboard::Key::Named(keyboard::key::Named::Escape), keyboard::Modifiers::empty());
                self.tarefas.push(t);
            }
            "congelar" => self.app.definir_congelamento(numero("linhas").min(999) as u32, numero("colunas").min(999) as u32),
            "selecionar_derramada" => self.enviar(Msg::Formulas(F::SelecionarDerramada)),
            "inspecao_abrir" => if !self.app.janelas.aberta(Auxiliar::Inspecao) { self.enviar(Msg::Inspecao(I::Abrir)) },
            "inspecao_adicionar" => self.enviar(Msg::Inspecao(I::Adicionar)),
            "inspecao_ir" => self.enviar(Msg::Inspecao(I::Ir(numero("indice")))),
            "inspecao_excluir" => self.enviar(Msg::Inspecao(I::Excluir(numero("indice")))),
            "avaliar_abrir" => self.enviar(Msg::Avaliar(avaliar_ui::Evento::Abrir)),
            "avaliar_passo" => self.enviar(Msg::Avaliar(avaliar_ui::Evento::Avaliar)),
            "avaliar_entrar" => self.enviar(Msg::Avaliar(avaliar_ui::Evento::Entrar)),
            "avaliar_sair" => self.enviar(Msg::Avaliar(avaliar_ui::Evento::Sair)),
            "avaliar_reiniciar" => self.enviar(Msg::Avaliar(avaliar_ui::Evento::Reiniciar)),
            "mostrar_formulas" => self.enviar(Msg::AlternarMostrarFormulas),
            "explicar_erro" => self.enviar(Msg::ExplicacaoErroAbrir),
            "gerenciador_nomes" => self.enviar(Msg::NomesAbrir(false)),
            "autosoma" => self.enviar(Msg::AutoSoma),
            "desfazer" => self.enviar(Msg::Desfazer),
            "refazer" => self.enviar(Msg::Refazer),
            outro => return Err(format!("Comando desconhecido: {outro}. Veja sheet_comando com comando=\"listar\".")),
        }
        let status = if self.app.status != antes { self.app.status.clone() } else { String::new() };
        Ok(format!("{status}{extra}"))
    }

    fn tela(&self) -> Option<String> {
        let a = &self.app;
        let mut s = format!("Aba da faixa: {:?}{}\n", a.faixa_ativa, if a.config.faixa_recolhida { " (faixa recolhida)" } else { "" });
        s.push_str(&format!("Planilha: {} · célula ativa {} · seleção {}\n",
            a.workbook.nomes().get(a.folha).cloned().unwrap_or_default(), cell_name(a.linha, a.coluna),
            feathersheet_mcp::texto::nome_area(&a.area_selecionada())));
        if !a.edicao.is_empty() || a.editor_formula.ativo {
            s.push_str(&format!("Barra fx: \"{}\"{}\n", a.edicao, if a.editor_formula.ativo { " (editando)" } else { "" }));
        }
        if let Some(m) = a.formulas_menu { s.push_str(&format!("Menu aberto: {m:?}\n")); }
        if let Some(b) = a.criar_nomes { s.push_str(&format!("Diálogo aberto: Criar nomes a partir da seleção {b:?}\n")); }
        if let Some(d) = a.descrever_avaliar() { s.push_str(&d); s.push('\n'); }
        if a.nomes_modal.is_some() { s.push_str("Diálogo aberto: Gerenciador de nomes\n"); }
        if a.erro_explicando.is_some() { s.push_str("Painel aberto: Diagnóstico de fórmula\n"); }
        if a.congelar_menu_aberto { s.push_str("Menu aberto: Congelar\n"); }
        if a.menu_aberto { s.push_str("Menu principal aberto\n"); }
        // #12: painéis, seletores e menus flutuantes também cobrem a grade.
        if a.condicional_aberto { s.push_str("Janela aberta: Formatação condicional\n"); }
        if a.condicional_menu_aberto { s.push_str("Menu aberto: Formatação condicional\n"); }
        if a.graficos.seletor_aberto { s.push_str("Seletor aberto: Tipo de gráfico\n"); }
        if a.graficos.aberto { s.push_str("Painel aberto: Gráfico\n"); }
        if a.graficos.contexto.is_some() { s.push_str("Menu aberto: botão direito no gráfico\n"); }
        if a.dinamica.is_some() { s.push_str("Painel aberto: Tabela dinâmica\n"); }
        if a.formatar_imagem.aberto { s.push_str("Painel aberto: Formatar imagem\n"); }
        if a.efeitos_imagem.aberto { s.push_str("Painel aberto: Efeitos da imagem\n"); }
        if a.imagem_contexto.is_some() { s.push_str("Menu aberto: botão direito na imagem\n"); }
        if a.contexto.is_some() { s.push_str("Menu aberto: botão direito na grade\n"); }
        if let Some(m) = a.edicao_menu { s.push_str(&format!("Menu aberto: {m} ▾ da faixa\n")); }
        if a.em_tela_inicio { s.push_str("Na tela inicial (sem planilha aberta)\n"); }
        let janelas: Vec<&str> = AUXILIARES.iter().filter(|(t, _)| a.janelas.aberta(*t)).map(|(_, n)| *n).collect();
        if !janelas.is_empty() { s.push_str(&format!("Janelas abertas: {}\n", janelas.join(", "))); }
        if let Some(f) = &a.formulario_macro {
            use feathersheet_macros::formularios::TipoControle;
            let campos: Vec<String> = f.form.controles.iter().filter(|c| c.tipo.tem_valor()).map(|c| {
                let mut t = format!("{}={}", c.nome, f.valores.get(&c.nome).map_or(String::new(), |v| v.as_str().map_or(v.to_string(), str::to_owned)));
                if matches!(c.tipo, TipoControle::Lista | TipoControle::Listagem) { t.push_str(&format!(" ({} itens)", c.opcoes.len())); }
                if c.desativado { t.push_str(" [desativado]"); }
                if c.oculto { t.push_str(" [oculto]"); }
                t
            }).collect();
            s.push_str(&format!("Formulário aberto: {} · {}\n", f.form.nome, campos.join(" · ")));
        }
        let setas = a.setas_visiveis().len();
        if setas > 0 { s.push_str(&format!("Setas de rastreio: {setas}\n")); }
        if !a.inspecoes.is_empty() { s.push_str(&format!("Inspeções: {}\n", a.inspecoes.len())); }
        if a.mostrar_formulas { s.push_str("Mostrar fórmulas: ligado\n"); }
        if a.config.calculo_manual {
            s.push_str(&format!("Cálculo manual{}\n", if a.workbook.calculo_pendente_manual() { " · aviso Calcular no rodapé" } else { "" }));
        }
        s.push_str(&format!("Status: {}\n", a.status));
        Some(s)
    }

    fn dinamica(&mut self, args: &Json) -> Result<String, String> {
        let origem = match args.get("origem").and_then(Json::as_str) {
            Some(_) => Some(feathersheet_mcp::texto::intervalo(self, args, "origem")?),
            None => None,
        };
        self.app.dinamica_pelo_mcp(args, origem)
    }

    fn macros_liberadas(&mut self) -> bool {
        self.app.macros_liberadas()
    }
    fn macros_mudaram(&mut self) {
        // O MCP grava as macros na própria pasta (`Workbook::macros`).
        self.app.recarregar_macros();
        self.app.alterado = true;
    }
    fn formulario(&mut self, fechar: bool, abrir: Option<(String, Json)>, campos: Vec<(String, String, Json)>) -> bool {
        if fechar { self.app.formulario_macro = None; }
        if let Some((nome, dados)) = abrir { self.app.abrir_formulario_macro(&nome, &dados); }
        self.app.aplicar_campos_formulario(campos);
        self.app.formulario_macro.is_some()
    }
}

impl App {
    pub(super) fn atender_mcp(&mut self, pedido: PedidoMcp) -> Task<Msg> {
        let msg = pedido.msg().clone();
        let params = msg.get("params").cloned().unwrap_or(Json::Null);
        if msg.get("method").and_then(Json::as_str) == Some("tools/call") && params["name"] == "sheet_ver" {
            let janela = params["arguments"]["janela"].as_str().unwrap_or("principal");
            let alvo = if janela == "principal" { self.janelas.principal }
                else { auxiliar_por_nome(janela).and_then(|a| self.janelas.id(a)) };
            let Some(id) = alvo else {
                pedido.responder_ferramenta(json!([{ "type": "text", "text": format!("A janela \"{janela}\" não está aberta.") }]), true);
                return Task::none();
            };
            return Task::done(Msg::McpImagem(pedido, self.fotografar(id)));
        }
        // `sheet_mouse`: o pelicano anda com o cursor desenhado "Claude" e clica de verdade
        // na janela, passo a passo (o roteiro roda depois desta resposta).
        if msg.get("method").and_then(Json::as_str) == Some("tools/call") && params["name"] == "sheet_mouse" {
            let args = &params["arguments"];
            let janela = args["janela"].as_str().unwrap_or("principal");
            let alvo = if janela == "principal" { self.janelas.principal }
                else { auxiliar_por_nome(janela).and_then(|a| self.janelas.id(a)) };
            let roteiro = args["roteiro"].as_str().unwrap_or("");
            let erro = match alvo {
                None => Some(format!("A janela \"{janela}\" não está aberta.")),
                Some(_) if roteiro.trim().is_empty() => Some("Falta o roteiro.".to_string()),
                Some(_) => pelicano::validar_roteiro(roteiro).err().map(|e| format!("Roteiro inválido: {e}")),
            };
            if let Some(e) = erro {
                pedido.responder_ferramenta(json!([{ "type": "text", "text": e }]), true);
                return Task::none();
            }
            pedido.responder_ferramenta(json!([{ "type": "text", "text":
                "Roteiro enviado; o cursor roda na tela agora. Confira o resultado com sheet_tela ou sheet_ver depois de esperar o tempo dos passos." }]), false);
            return Task::window_of(alvo.unwrap(), pelicano::WindowCmd::Roteiro(roteiro.to_string()));
        }
        // `sheet_macro` com acao "executar" roda a macro dentro do
        // feathersheet-mcp, sem passar pelo `rodar_macro`: mede aqui e anota
        // no histórico de macros pela resposta.
        let macro_mcp = (msg.get("method").and_then(Json::as_str) == Some("tools/call")
            && params["name"] == "sheet_macro" && params["arguments"]["acao"] == "executar")
            .then(|| (std::time::Instant::now(), self.alterado));
        let mut h = HospedeApp { app: self, tarefas: Vec::new() };
        let resposta = rpc::atender(&mut h, &msg);
        let tarefas = std::mem::take(&mut h.tarefas);
        if let (Some((inicio, alterado_antes)), Some(r)) = (macro_mcp, &resposta) {
            self.anotar_macro_mcp(&params["arguments"], r, inicio.elapsed().as_millis(), alterado_antes);
        }
        if let Some(r) = resposta { pedido.responder(r); }
        Task::batch(tarefas)
    }

    /// A execução de macro pedida pelo MCP, no histórico: erro quando a
    /// ferramenta falhou (`isError`) ou a saída traz a linha "Erro: …".
    fn anotar_macro_mcp(&mut self, args: &Json, resposta: &Json, ms: u128, alterado_antes: bool) {
        use historico_macros::{Execucao, GatilhoMacro, ResultadoExecucao};
        let resultado = &resposta["result"];
        let texto: String = resultado["content"].as_array().into_iter().flatten()
            .filter_map(|c| c["text"].as_str()).collect::<Vec<_>>().join("\n");
        let erro = texto.lines().find_map(|l| l.strip_prefix("Erro: ")).map(str::to_owned)
            .or_else(|| (resultado["isError"] == true || resposta.get("error").is_some()).then(|| {
                if texto.is_empty() { resposta["error"]["message"].as_str().unwrap_or("falhou").to_owned() } else { texto.clone() }
            }));
        let nome = args["nome"].as_str().map(str::to_owned)
            .unwrap_or_else(|| "(código avulso do MCP)".into());
        self.historico_macros.registrar(Execucao {
            id: 0, hora: String::new(), macro_: nome, gatilho: GatilhoMacro::Mcp,
            resultado: erro.map_or(ResultadoExecucao::Ok, ResultadoExecucao::Erro), ms,
            alterou: !alterado_antes && self.alterado, respostas: Vec::new(), arquivos: Vec::new(),
        });
    }

    pub(super) fn responder_imagem(&mut self, pedido: PedidoMcp, tela: Foto) -> Task<Msg> {
        use base64::Engine;
        // A foto do pelicano já vem em PNG.
        let png: Result<&[u8], String> = if tela.png.is_empty() { Err("imagem inválida".into()) } else { Ok(&tela.png) };
        match png {
            Ok(png) => pedido.responder_ferramenta(json!([
                { "type": "image", "mimeType": "image/png", "data": base64::engine::general_purpose::STANDARD.encode(png) },
                { "type": "text", "text": format!("{}×{} px", tela.largura, tela.altura) },
            ]), false),
            Err(e) => pedido.responder_ferramenta(json!([{ "type": "text", "text": format!("Não consegui fotografar: {e}") }]), true),
        }
        Task::none()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn chamar(app: &mut App, nome: &str, args: Json) -> (bool, String) {
        let mut h = HospedeApp { app, tarefas: Vec::new() };
        let r = rpc::chamar_ferramenta(&mut h, &json!({ "name": nome, "arguments": args }));
        (r["isError"] == true, r["content"][0]["text"].as_str().unwrap_or("").to_string())
    }

    /// #127: com o arquivo em "Ativar macros?", o MCP não roda macro e o aviso volta.
    #[test]
    fn macro_pelo_mcp_respeita_o_bloqueio() {
        let mut app = App::default();
        app.janelas.principal = Some(window::novo_id());
        app.macros_bloqueadas = true;
        let (erro, t) = chamar(&mut app, "sheet_macro", json!({ "acao": "executar", "codigo": "sheet.set('A1', 7)" }));
        assert!(erro && t.contains("desativadas"), "{t}");
        assert!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0).is_empty());
        assert!(app.aviso_macros);
        app.macros_bloqueadas = false;
        let (erro, t) = chamar(&mut app, "sheet_macro", json!({ "acao": "executar", "codigo": "sheet.set('A1', 7)" }));
        assert!(!erro, "{t}");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "7");
    }

    #[test]
    fn atalhos_viram_teclas() {
        let (k, m) = tecla("Ctrl+Shift+F9").unwrap();
        assert_eq!(k, keyboard::Key::Named(keyboard::key::Named::F9));
        assert!(m.control() && m.shift());
        assert_eq!(tecla("Ctrl+/").unwrap().0, keyboard::Key::Character("/".into()));
        assert!(tecla("Hiper+X").is_none());
    }

    #[test]
    fn ferramentas_agem_na_pasta_da_tela() {
        let mut app = App::default();
        app.janelas.principal = Some(window::novo_id());
        let (erro, t) = chamar(&mut app, "sheet_escrever", json!({ "celula": "A1", "valores": [[2], ["=A1*10"]] }));
        assert!(!erro, "{t}");
        assert!(app.alterado);
        let (_, t) = chamar(&mut app, "sheet_selecionar", json!({ "intervalo": "A2" }));
        assert!(t.contains("A2"), "{t}");
        assert_eq!((app.linha, app.coluna), (1, 0));
        let (erro, t) = chamar(&mut app, "sheet_comando", json!({ "comando": "rastrear_precedentes" }));
        assert!(!erro && t.contains("seta"), "{t}");
        assert_eq!(app.setas_visiveis().len(), 1);
        let (_, t) = chamar(&mut app, "sheet_tela", json!({}));
        assert!(t.contains("Setas de rastreio: 1"), "{t}");
        let (erro, _) = chamar(&mut app, "sheet_comando", json!({ "comando": "tecla", "args": { "combo": "Ctrl+Z" } }));
        assert!(!erro);
        assert!(app.workbook.aba(0).unwrap().sheet.get_raw(1, 0).is_empty(), "Ctrl+Z desfez a escrita da IA inteira");
        let (erro, t) = chamar(&mut app, "sheet_comando", json!({ "comando": "faixa", "args": { "aba": "Fórmulas" } }));
        assert!(!erro, "{t}");
        assert_eq!(app.faixa_ativa, AbaFaixa::Formulas);
        let (_, t) = chamar(&mut app, "sheet_comando", json!({ "comando": "listar" }));
        assert!(t.contains("rastrear_dependentes"));
    }

    /// sheet_filtrar ao vivo: liga o AutoFiltro, esconde as linhas como as
    /// setas do cabeçalho, lista, limpa e remove.
    #[test]
    fn regra_condicional_pela_ia_nao_abre_janela() {
        // #11: a regra entra na aba sem abrir a janela de formatação condicional.
        let mut app = App::default();
        app.janelas.principal = Some(window::novo_id());
        let (erro, t) = chamar(&mut app, "sheet_condicional", json!({ "acao": "criar", "intervalo": "A1:A3", "tipo": "maior", "valores": [5], "fundo": "#ff0000" }));
        assert!(!erro, "{t}");
        assert!(!app.condicional_aberto && !app.condicional_menu_aberto);
        assert!(!app.janelas.aberta(Auxiliar::Condicional));
        assert!(app.workbook.aba(0).unwrap().sheet.r.is_some());
    }

    #[test]
    fn tela_diz_os_paineis_e_seletores_abertos() {
        // #12: o seletor de gráfico e o diálogo de formatação condicional não apareciam.
        let mut app = App::default();
        app.janelas.principal = Some(window::novo_id());
        app.condicional_aberto = true;
        let _ = chamar(&mut app, "sheet_escrever", json!({ "celula": "A1", "valores": [["Região", "Total"], ["Sul", 3]] }));
        let _ = chamar(&mut app, "sheet_selecionar", json!({ "intervalo": "A1:B2" }));
        let _ = chamar(&mut app, "sheet_comando", json!({ "comando": "menu", "args": { "texto": "Gráfico…" } }));
        let (_, t) = chamar(&mut app, "sheet_tela", json!({}));
        assert!(t.contains("Formatação condicional"), "{t}");
        assert!(t.contains("Tipo de gráfico") || t.contains("Painel aberto: Gráfico"), "{t}");
    }

    #[test]
    fn menus_lista_os_rotulos_com_caminho_e_menu_aciona_um_deles() {
        // #13: "Congelar linha superior" não existia e não havia como descobrir "1 linha".
        let mut app = App::default();
        app.janelas.principal = Some(window::novo_id());
        let (erro, t) = chamar(&mut app, "sheet_comando", json!({ "comando": "menus", "args": { "filtro": "congelar" } }));
        assert!(!erro && t.contains("› 1 linha"), "{t}");
        let (_, t) = chamar(&mut app, "sheet_comando", json!({ "comando": "menu", "args": { "texto": "Congelar linha superior" } }));
        assert!(t.contains("comando=\"menus\""), "o erro aponta o caminho: {t}");
    }

    #[test]
    fn congelar_pelo_mcp() {
        let mut app = App::default();
        let (erro, t) = chamar(&mut app, "sheet_comando", json!({ "comando": "congelar", "args": { "linhas": 2, "colunas": 1 } }));
        assert!(!erro && app.congelamento_atual() == (2, 1), "{t}");
        chamar(&mut app, "sheet_comando", json!({ "comando": "congelar", "args": { "linhas": 0, "colunas": 0 } }));
        assert_eq!(app.congelamento_atual(), (0, 0));
    }

    #[test]
    fn filtrar_pelo_mcp_usa_o_autofiltro_da_tela() {
        let mut app = App::default();
        let (erro, t) = chamar(&mut app, "sheet_escrever", json!({ "celula": "A1", "valores": [
            ["Nome", "UF"], ["Bia", "SP"], ["Ana", "RJ"], ["Caio", "SP"], ["Davi", "MG"]] }));
        assert!(!erro, "{t}");
        let oculta = |app: &App, r: u32| app.workbook.aba(0).unwrap().sheet.row_heights.get(&r) == Some(&0);
        let (erro, t) = chamar(&mut app, "sheet_filtrar", json!({ "intervalo": "A1:B5", "coluna": "UF", "criterio": ["SP"] }));
        assert!(!erro && t.contains("2 linha(s) escondida(s)"), "{t}");
        assert!(!oculta(&app, 1) && oculta(&app, 2) && !oculta(&app, 3) && oculta(&app, 4));
        let (_, t) = chamar(&mut app, "sheet_filtrar", json!({ "intervalo": "A1", "acao": "listar" }));
        assert!(t.contains("AutoFiltro em A1:B5") && t.contains("coluna B"), "{t}");
        let (erro, t) = chamar(&mut app, "sheet_filtrar", json!({ "intervalo": "A1", "acao": "limpar" }));
        assert!(!erro && !oculta(&app, 2), "{t}");
        let (erro, t) = chamar(&mut app, "sheet_filtrar", json!({ "intervalo": "A1", "acao": "remover" }));
        assert!(!erro, "{t}");
        let (_, t) = chamar(&mut app, "sheet_filtrar", json!({ "intervalo": "A1", "acao": "listar" }));
        assert!(t.starts_with("Sem AutoFiltro"), "{t}");
    }

    #[test]
    fn macro_rodada_pelo_mcp_entra_no_historico() {
        let mut app = App::default();
        let pedir = |app: &mut App, codigo: &str| {
            let msg = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": { "name": "sheet_macro", "arguments": { "acao": "executar", "codigo": codigo } } });
            let _ = app.atender_mcp(PedidoMcp(Arc::new((msg, Mutex::new(None)))));
        };
        pedir(&mut app, "sheet.set('A1', 3);");
        pedir(&mut app, "throw new Error('falhou na IA');");
        let linhas: Vec<_> = app.historico_macros.filtradas().collect();
        assert_eq!(linhas.len(), 2);
        assert!(linhas.iter().all(|e| e.gatilho == historico_macros::GatilhoMacro::Mcp));
        assert!(matches!(&linhas[0].resultado, historico_macros::ResultadoExecucao::Erro(e) if e.contains("falhou na IA")),
            "{:?}", linhas[0].resultado);
        assert_eq!(linhas[1].resultado, historico_macros::ResultadoExecucao::Ok);
        assert!(linhas[1].alterou);
    }

    #[test]
    fn editar_passa_pela_barra_fx() {
        let mut app = App::default();
        let (erro, t) = chamar(&mut app, "sheet_comando", json!({ "comando": "editar", "args": { "texto": "=SOMA(1;2)" } }));
        assert!(!erro, "{t}");
        assert_eq!(app.workbook.valor(0, 0, 0), feathersheet_core::celulas::values::Value::Number(3.));
    }
}
