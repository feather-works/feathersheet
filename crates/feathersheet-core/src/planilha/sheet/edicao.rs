//! A edição da planilha: foto da célula, aplicar, desfazer, estilo, limpar e
//! colar.
//!
//! Tradução do miolo de escrita do `sheet.ts` (`snapshot`, `apply`,
//! `transact`, `setCell`, `setStyle`, `clear`, `paste`, `undo`, `redo`). Até
//! aqui o Rust só sabia gravar um texto numa célula; a interface precisa de
//! tudo isto para deixar de guardar a planilha do lado do JavaScript.
//!
//! **Portar é traduzir, não melhorar** (regra 12 do `CLAUDE.md`). Três
//! comportamentos daqui parecem estranhos e são de propósito, porque é o que
//! o TypeScript faz e há planilha que depende:
//!
//! - o `setStyle` de área pequena grava na célula o estilo **já mesclado**
//!   com o da coluna, da linha e da aba (é o `getStyle` do TypeScript, que
//!   devolve o mesclado);
//! - o `styleFor` de quem digita faz o mesmo com o formato implícito;
//! - o colar sem desfazer (a importação de CSV) decide número pelo primeiro
//!   caractere e **não guarda o texto digitado**: "007" vira 7.
//!
//! O módulo é filho de `sheet` para enxergar os campos privados da `Sheet`
//! sem abri-los para o resto do crate.

use super::*;
use crate::celulas::estilo_parcial::{formato, mesclar, tem_estilo, Campo};
use crate::celulas::number::to_js_string;
use std::io::{self, Read, Write, Seek, SeekFrom};
use std::sync::{Arc, Mutex, atomic::{AtomicU64, Ordering}};


/// O que uma célula diz: o texto digitado e o estilo direto. É o
/// `CellSnapshot` do TypeScript, e é a unidade do desfazer.
#[derive(Debug, Clone, PartialEq)]
pub struct Snap {
    pub raw: String,
    pub style: Option<EstiloParcial>,
}

impl Snap {
    pub fn texto(raw: impl Into<String>) -> Self {
        Snap { raw: raw.into(), style: None }
    }
}

#[derive(Debug, Clone)]
struct Change {
    k: CellKey,
    antes: Option<Snap>,
    depois: Option<Snap>,
}

/// Referência a um texto em buffer compacto. `inicio == u32::MAX`
/// representa snapshot inexistente; `estilo == 0` representa ausência.
#[derive(Debug, Clone, Copy)]
struct FotoCompacta {
    inicio: u32,
    tamanho: u32,
    estilo: u32,
}
#[derive(Debug, Clone)]
struct EdicaoCompacta {
    k: CellKey,
    antes: FotoCompacta,
    depois: FotoCompacta,
}
/// Uma única alocação para strings e um registro fixo pequeno por célula.
/// Quando há estilos, são internados por bloco, preservando a edição exata.
#[derive(Debug, Clone)]
struct BlocoCompacto {
    alteracoes: Vec<EdicaoCompacta>,
    textos: Vec<u8>,
    estilos: Vec<EstiloParcial>,
    area: [u32; 4],
}
impl BlocoCompacto {
    fn novo() -> Self {
        Self {
            alteracoes: Vec::new(),
            textos: Vec::new(),
            estilos: Vec::new(),
            area: [u32::MAX, u32::MAX, 0, 0],
        }
    }
    fn guardar(&mut self, foto: Option<Snap>) -> FotoCompacta {
        let Some(foto) = foto else {
            return FotoCompacta { inicio: u32::MAX, tamanho: 0, estilo: 0 };
        };
        let inicio = self.textos.len();
        self.textos.extend_from_slice(foto.raw.as_bytes());
        let estilo = if let Some(estilo) = foto.style {
            match self.estilos.iter().position(|s| s == &estilo) {
                Some(i) => (i + 1) as u32,
                None => { self.estilos.push(estilo); self.estilos.len() as u32 },
            }
        } else { 0 };
        FotoCompacta {
            inicio: inicio as u32,
            tamanho: foto.raw.len() as u32,
            estilo,
        }
    }
    fn adicionar(&mut self, c: Change) {
        let (row, col) = (key_row(c.k), key_col(c.k));
        self.area[0] = self.area[0].min(row);
        self.area[1] = self.area[1].min(col);
        self.area[2] = self.area[2].max(row);
        self.area[3] = self.area[3].max(col);
        let antes = self.guardar(c.antes);
        let depois = self.guardar(c.depois);
        self.alteracoes.push(EdicaoCompacta { k: c.k, antes, depois });
    }
    fn reconstruir(&self, foto: FotoCompacta) -> Option<Snap> {
        if foto.inicio == u32::MAX { return None; }
        let inicio = foto.inicio as usize;
        let fim = inicio + foto.tamanho as usize;
        let raw = String::from_utf8(self.textos[inicio..fim].to_vec())
            .expect("texto do histórico foi guardado como UTF-8");
        let style = if foto.estilo == 0 { None }
            else { self.estilos.get((foto.estilo - 1) as usize).cloned() };
        Some(Snap { raw, style })
    }
}

/// Limite aproximado da RAM dos snapshots de uma transação de conversão.
/// Os blocos mais antigos são movidos para um arquivo temporário após 24 MiB.
const LIMITE_HISTORIA_DIVISAO_RAM: usize = 24 * 1024 * 1024;
static SEQUENCIA_HISTORIA: AtomicU64 = AtomicU64::new(1);

/// Dados de undo grandes são mantidos em disco, não num Vec de milhões
/// de fotografias. Um Arc compartilhado entre undo e redo mantém o arquivo
/// vivo. O Drop remove o arquivo somente após o último bloco desaparecer.
#[derive(Debug)]
struct ArquivoHistorico {
    caminho: std::path::PathBuf,
    arquivo: Mutex<std::fs::File>,
}
impl ArquivoHistorico {
    fn criar() -> io::Result<Arc<Self>> {
        let pasta = Self::pasta();
        let _ = std::fs::create_dir_all(&pasta);
        Self::limpar_orfaos(&pasta);
        for _ in 0..8 {
            let contador = SEQUENCIA_HISTORIA.fetch_add(1, Ordering::Relaxed);
            let caminho = pasta.join(format!(
                "feathersheet-undo-{}-{contador}.bin", std::process::id()
            ));
            let mut opcoes = std::fs::OpenOptions::new();
            opcoes.create_new(true).read(true).write(true);
            // O histórico pode conter dados sensíveis da planilha.
            // Em Unix o arquivo temporário deve ser privado (0600).
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opcoes.mode(0o600);
            }
            match opcoes.open(&caminho) {
                Ok(arquivo) => return Ok(Arc::new(Self {
                    caminho, arquivo: Mutex::new(arquivo),
                })),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::new(io::ErrorKind::AlreadyExists,
            "não foi possível criar arquivo exclusivo de undo"))
    }
    /// Disco de verdade: em muitos Linux o `/tmp` é tmpfs, ou seja, RAM —
    /// "mandar para o disco" lá não soltava memória nenhuma (13/10/2026:
    /// 11 GB de histórico esquecido no `/tmp`).
    fn pasta() -> std::path::PathBuf {
        if let Some(p) = std::env::var_os("FEATHERSHEET_UNDO_DIR") {
            return p.into();
        }
        let cache = std::env::var_os("XDG_CACHE_HOME").map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".cache")))
            .or_else(|| std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from));
        match cache {
            Some(c) => c.join("feathersheet").join("desfazer"),
            None => std::env::temp_dir(),
        }
    }

    /// Apaga histórico de processos que já morreram (app fechado à força
    /// não roda o `Drop`). Só no Linux dá para saber pelo `/proc`.
    fn limpar_orfaos(pasta: &std::path::Path) {
        static FEITO: std::sync::Once = std::sync::Once::new();
        FEITO.call_once(|| {
            if !std::path::Path::new("/proc/self").exists() { return; }
            let Ok(itens) = std::fs::read_dir(pasta) else { return };
            for item in itens.flatten() {
                let nome = item.file_name();
                let Some(pid) = nome.to_str()
                    .and_then(|n| n.strip_prefix("feathersheet-undo-"))
                    .and_then(|n| n.split('-').next())
                    .and_then(|p| p.parse::<u32>().ok()) else { continue };
                if pid != std::process::id() && !std::path::Path::new(&format!("/proc/{pid}")).exists() {
                    let _ = std::fs::remove_file(item.path());
                }
            }
        });
    }

    fn gravar(self: &Arc<Self>, bloco: &BlocoCompacto) -> io::Result<BlocoNoDisco> {
        let mut registro = Vec::with_capacity(
            8 + bloco.alteracoes.len() * 32 + bloco.textos.len()
        );
        registro.extend_from_slice(&(bloco.alteracoes.len() as u32).to_le_bytes());
        registro.extend_from_slice(&(bloco.textos.len() as u32).to_le_bytes());
        for c in &bloco.alteracoes {
            registro.extend_from_slice(&c.k.to_le_bytes());
            for foto in [c.antes, c.depois] {
                registro.extend_from_slice(&foto.inicio.to_le_bytes());
                registro.extend_from_slice(&foto.tamanho.to_le_bytes());
                registro.extend_from_slice(&foto.estilo.to_le_bytes());
            }
        }
        registro.extend_from_slice(&bloco.textos);
        let mut arquivo = self.arquivo.lock().map_err(|_| io::Error::other(
            "arquivo de undo indisponível"
        ))?;
        let offset = arquivo.seek(SeekFrom::End(0))?;
        arquivo.write_all(&registro)?;
        Ok(BlocoNoDisco {
            origem: Arc::clone(self),
            offset, tamanho: registro.len(),
            area: bloco.area, estilos: bloco.estilos.clone(),
        })
    }
}
impl Drop for ArquivoHistorico {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.caminho);
    }
}
#[derive(Debug, Clone)]
struct BlocoNoDisco {
    origem: Arc<ArquivoHistorico>,
    offset: u64,
    tamanho: usize,
    area: [u32; 4],
    estilos: Vec<EstiloParcial>,
}
impl BlocoNoDisco {
    fn carregar(&self) -> io::Result<BlocoCompacto> {
        // Um único lote é carregado por vez. Não reconstruímos 1M de
        // alterações em RAM para desfazer/refazer.
        let mut bytes = vec![0u8; self.tamanho];
        {
            let mut arquivo = self.origem.arquivo.lock().map_err(|_| io::Error::other(
                "arquivo de undo indisponível"
            ))?;
            arquivo.seek(SeekFrom::Start(self.offset))?;
            arquivo.read_exact(&mut bytes)?;
        }
        let mut cursor = io::Cursor::new(bytes);
        fn u32_le(r: &mut impl Read) -> io::Result<u32> {
            let mut v = [0u8; 4];
            r.read_exact(&mut v)?;
            Ok(u32::from_le_bytes(v))
        }
        let quantidade = u32_le(&mut cursor)? as usize;
        let tamanho_textos = u32_le(&mut cursor)? as usize;
        if 8usize.saturating_add(quantidade.saturating_mul(32))
            .saturating_add(tamanho_textos) != self.tamanho
        {
            return Err(io::Error::new(io::ErrorKind::InvalidData,
                "histórico temporário inconsistente"));
        }
        let mut alteracoes = Vec::with_capacity(quantidade);
        for _ in 0..quantidade {
            let mut k = [0u8; 8];
            cursor.read_exact(&mut k)?;
            let mut ler_foto = || -> io::Result<FotoCompacta> {
                Ok(FotoCompacta {
                    inicio: u32_le(&mut cursor)?,
                    tamanho: u32_le(&mut cursor)?,
                    estilo: u32_le(&mut cursor)?,
                })
            };
            alteracoes.push(EdicaoCompacta {
                k: u64::from_le_bytes(k),
                antes: ler_foto()?,
                depois: ler_foto()?,
            });
        }
        let mut textos = vec![0u8; tamanho_textos];
        cursor.read_exact(&mut textos)?;
        Ok(BlocoCompacto {
            alteracoes, textos,
            estilos: self.estilos.clone(), area: self.area,
        })
    }
}

/// Um passo de uma transação do desfazer.
#[derive(Debug, Clone)]
enum Passo {
    Celula(Change),
    /// Pacote de modificações de conversão, independente de quantas linhas.
    Compacto(BlocoCompacto),
    /// Snapshot excedente de conversão, armazenado no disco temporário.
    CompactoNoDisco(BlocoNoDisco),
    /// As linhas `first..first + ordem.len()` × `c0..=c1` foram permutadas —
    /// `ordem[i]` foi para `first + i` (o ordenar). Guardar a permutação, e
    /// não a foto de cada célula, é o que deixa ordenar dois milhões de
    /// linhas caber na memória: eram 22 milhões de fotos (5,5 GB).
    /// `quebradas`: fórmulas que o deslocamento transformou em `#REF!`
    /// (linha de origem, coluna, texto e estilo de antes): a permutação inversa
    /// não as conserta, então o desfazer regrava o texto original.
    Permutacao { first: u32, c0: u32, c1: u32, ordem: Vec<u32>, quebradas: Vec<(u32, u32, String, Option<usize>)> },
    /// Largura/altura são propriedades da grade e precisam do mesmo undo das células.
    Dimensao { coluna: bool, indice: u32, antes: Option<u32>, depois: Option<u32> },
    /// Largura padrão das colunas da aba.
    LarguraPadrao { antes: Option<u32>, depois: Option<u32> },
    /// Linhas (ou colunas) ocultas de um índice até o fim da planilha.
    OcultasDesde { coluna: bool, antes: Option<u32>, depois: Option<u32> },
    /// Níveis da estrutura de tópicos (Agrupar), inteiros: são poucos.
    Topicos { antes: Box<super::topicos::Topicos>, depois: Box<super::topicos::Topicos> },
    /// Regiões mescladas da aba (JSON `mc`): não altera o conteúdo das células.
    Mesclas { area: [u32; 4], antes: Option<String>, depois: Option<String> },
    /// Metadados de estilo/totais das tabelas, sem fotografar nenhuma célula.
    /// Filtros (alturas) e nomes definidos são tratados em etapas próprias.
    Tabelas { area: [u32; 4], antes: Option<String>, depois: Option<String> },
    /// A definição e o mapa de cada tabela dinâmica (JSON dn) seguem
    /// o mesmo Ctrl+Z/Ctrl+Y da gravação das células do resultado.
    Dinamicas { area: [u32; 4], antes: Option<String>, depois: Option<String> },
    /// Objetos de imagem da aba (JSON `im`): mover/redimensionar desfaz num passo só.
    Imagens { antes: Option<String>, depois: Option<String> },
    /// Objetos clicáveis das macros da aba (JSON bt): uma operação por gesto.
    Botoes { antes: Option<String>, depois: Option<String> },
    /// Tinta da aba Desenhar (texto `tn`): cada traço/apagar/forma é um passo.
    Tinta { antes: Option<String>, depois: Option<String> },
    /// Objetos soltos (JSON ob): criar, mover, redimensionar, girar, apagar
    /// ou mudar a aparência é um passo.
    Objetos { antes: Option<String>, depois: Option<String> },
    /// Gráficos da aba (JSON g): cada edição do painel Formatar gráfico,
    /// mover, redimensionar ou excluir é um passo.
    Graficos { antes: Option<String>, depois: Option<String> },
    /// Comentário de uma célula (aba Revisão): o de antes e o de depois.
    Comentario { linha: u32, coluna: u32, antes: Option<Box<super::comentarios::Comentario>>, depois: Option<Box<super::comentarios::Comentario>> },
    /// Regras de validação (JSON va): sem snapshot de células, com undo.
    Validacoes { area: [u32; 4], antes: Option<String>, depois: Option<String> },
    /// Formatação condicional (JSON r) no histórico de transações.
    Condicionais { area: [u32; 4], antes: Option<String>, depois: Option<String> },
    /// Texto para Colunas em lote: guarda só a foto do que JÁ EXISTIA na
    /// área (passos `Compacto`, sem o "depois") e a receita para refazer.
    /// Desfazer = limpar a área e devolver a foto; refazer = aplicar a
    /// receita de novo — o estado depois do desfazer é o de antes, então dá
    /// o mesmo resultado. Eram 87 milhões de fotos no CSV do Spotify (31 s).
    Divisao { area: [u32; 4], antes: Vec<Passo>, refazer: Refazer },
}

/// A receita do refazer de um [`Passo::Divisao`].
#[derive(Clone)]
pub struct Refazer(pub Arc<dyn Fn(&mut Sheet) + Send + Sync>);
impl std::fmt::Debug for Refazer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Refazer")
    }
}

/// Um campo do Texto para Colunas já analisado — pode ser feito fora da
/// thread da janela, em paralelo, porque não lê a planilha.
#[derive(Debug, Clone)]
pub struct CampoDivisao {
    /// O texto aparado, como o `aplicar_divisao` grava.
    pub raw: String,
    /// `None`: fórmula, erro ou vazio — vai pelo caminho completo.
    pub rapido: Option<CampoRapido>,
}

#[derive(Debug, Clone)]
pub struct CampoRapido {
    pub valor: Value,
    pub formato: Option<&'static str>,
    /// O texto digitado precisa ser guardado ao lado do valor.
    pub preserva: bool,
}

impl CampoDivisao {
    /// A mesma decisão do [`Sheet::set_cell_divisao`] para uma célula sem
    /// estilo nenhum por perto (nem dela, nem da linha, coluna ou aba).
    pub fn novo(raw: &str) -> Self {
        let rapido = (!raw.is_empty() && !raw.starts_with('=')).then(|| {
            let analisado = crate::celulas::input::parse_literal(raw);
            if matches!(analisado.value, Value::Error(_)) { return None; }
            let formato = formato_do_literal(analisado.format);
            let estilo = formato.map(|f| {
                let mut patch = EstiloParcial::new();
                patch.insert("format".into(), Campo::Texto(f.into()));
                patch
            });
            let valor = analisado.value;
            let preserva = !round_trips(raw, &valor, estilo.as_ref()) || match &valor {
                Value::Text(texto) => texto != raw,
                Value::Number(numero) => to_js_string(*numero) != raw,
                Value::Bool(true) => raw != "VERDADEIRO",
                Value::Bool(false) => raw != "FALSO",
                _ => true,
            };
            Some(CampoRapido { valor, formato, preserva })
        }).flatten();
        CampoDivisao { raw: raw.to_owned(), rapido }
    }
}

/// Estado de uma divisão em andamento numa aba: o que já se sabe da área
/// para não perguntar de novo a cada uma das milhões de células.
#[derive(Debug)]
pub struct GravadorDivisao {
    /// Células da área com fórmula avulsa, estilo direto ou função externa:
    /// vão pelo caminho completo.
    especiais: HashSet<CellKey>,
    /// Há estilo de aba, linha ou coluna alcançando a área: tudo pelo
    /// caminho completo (o formato herdado muda a decisão do literal).
    estilo_herdado: bool,
    formulas_leem: bool,
    /// Por coluna da área (a partir de `c0`): com certeza vazia antes da
    /// divisão — dispensa perguntar a cada célula se há algo a desligar.
    coluna_limpa: Vec<bool>,
    c0: u32,
    estilo_do_formato: std::collections::HashMap<&'static str, Option<usize>>,
    tipos: Vec<crate::operacoes::ops::TipoColunaDivisao>,
}

/// As pilhas do desfazer e a transação aberta.
#[derive(Debug, Default)]
pub struct Historico {
    desfazer: Vec<Vec<Passo>>,
    refazer: Vec<Vec<Passo>>,
    pendente: Option<Vec<Passo>>,
    profundidade: u32,
    memoria_divisao: usize,
    arquivo_divisao: Option<Arc<ArquivoHistorico>>,
    /// Transações que o teto tirou do fundo da pilha (o `historicoDescartado`
    /// do TypeScript).
    descartadas: usize,
}

/// O TypeScript guarda as últimas 200 transações.
const LIMITE_DO_DESFAZER: usize = 200;

/// Até quantas células o `setStyle`/`clear` percorre o retângulo inteiro.
/// Acima disso só mexe no que já existe. Igual ao `sheet.ts`.
const AREA_PEQUENA: u64 = 500;

/// `displayRaw` do `sheet.ts`: o texto que se reconstrói do valor.
fn display_raw(valor: &Value, estilo: Option<&EstiloParcial>) -> Option<String> {
    match valor {
        Value::Number(n) => {
            let fmt = formato(estilo);
            match fmt.as_deref() {
                Some(f) if f != "auto" && f != "text" => {
                    let s = crate::celulas::estilo_parcial::para_style(estilo.unwrap());
                    Some(crate::celulas::style::format_number(*n, Some(&s)))
                }
                _ => Some(to_js_string(*n)),
            }
        }
        Value::Text(t) => Some(t.clone()),
        Value::Bool(b) => Some(if *b { "VERDADEIRO" } else { "FALSO" }.to_string()),
        _ => None,
    }
}

/// `roundTrips` do `sheet.ts`.
pub(crate) fn round_trips(raw: &str, valor: &Value, estilo: Option<&EstiloParcial>) -> bool {
    if let Value::Bool(_) = valor {
        return matches!(raw, "VERDADEIRO" | "FALSO" | "TRUE" | "FALSE");
    }
    display_raw(valor, estilo).as_deref() == Some(raw)
}

/// Formato implicito delegado ao mesmo parser de entrada do motor Rust.
pub fn implied_format(raw: &str) -> Option<&'static str> {
    formato_do_literal(crate::celulas::input::implied_format(raw))
}

/// O nome do formato que um literal já analisado sugere — a metade do
/// [`implied_format`] que não é fórmula. Serve quem já tem o
/// `parse_literal` em mãos e não quer analisar o texto duas vezes.
pub fn formato_do_literal(formato: Option<crate::celulas::input::NumberFormat>) -> Option<&'static str> {
    use crate::celulas::input::NumberFormat as F;
    match formato? {
        F::General => None,
        F::Number => Some("number"),
        F::Currency => Some("currency"),
        F::Percent => Some("percent"),
        F::Date => Some("date"),
        F::Time => Some("time"),
        F::Datetime => Some("datetime"),
        F::Text => Some("text"),
    }
}

/// O valor do colar rápido (a importação): o `paste` sem desfazer do
/// TypeScript decide pelo primeiro caractere e não guarda o texto.
fn valor_rapido(raw: &str) -> Value {
    let primeiro = raw.as_bytes()[0];
    if primeiro.is_ascii_digit() || primeiro == b'-' || primeiro == b'+' {
        return match crate::celulas::input::parse_literal(raw).value {
            Value::Number(n) => Value::Number(n),
            _ => Value::Text(raw.to_string()),
        };
    }
    match raw {
        "VERDADEIRO" | "TRUE" => Value::Bool(true),
        "FALSO" | "FALSE" => Value::Bool(false),
        _ => Value::Text(raw.to_string()),
    }
}

impl Sheet {
    // ------------------------------------------------------------- leitura

    fn estilo_direto(&self, k: CellKey) -> Option<&EstiloParcial> {
        let idx = match self.cells.get(&k) {
            Some(c) => c.style_idx,
            None => self.styles.get(k),
        }?;
        self.style_parts.get(idx)
    }

    /// A foto da célula: o texto digitado e o estilo direto. `None` quando
    /// não há nada.
    pub fn snapshot(&self, row: u32, col: u32) -> Option<Snap> {
        let k = key(row, col);
        let style = self.estilo_direto(k).cloned();
        if let Some(cell) = self.cells.get(&k) {
            return Some(Snap { raw: cell.raw.clone(), style });
        }
        if self.data.has_cell(row, col) {
            let raw = match self.textos.get(&k) {
                Some(t) => t.to_owned(),
                None => {
                    let v = self.data.get_cell_value(row, col, &self.pool);
                    display_raw(&v, style.as_ref()).unwrap_or_default()
                }
            };
            if !raw.is_empty() {
                return Some(Snap { raw, style });
            }
        }
        if let Some(raw) = self.formulas.raw_at(row, col) {
            // A fórmula empacotada também pode ter estilo (moeda, %, data).
            return Some(Snap { raw, style });
        }
        // Célula vazia que só tem estilo — um fundo pintado.
        style.map(|s| Snap { raw: String::new(), style: Some(s) })
    }

    /// A cor de fundo (`bg`) ou da fonte (`color`) que vale na célula, sem
    /// montar o estilo inteiro: a camada de cima que tiver o campo ganha
    /// (célula, linha, coluna, aba — a ordem de `estilo_mesclado`).
    pub fn cor_celula(&self, row: u32, col: u32, fonte: bool) -> Option<String> {
        let campo = if fonte { "color" } else { "bg" };
        let camadas = [self.estilo_direto(key(row, col)), self.row_styles.get(&row),
            self.col_styles.get(&col), self.sheet_style.as_ref()];
        for camada in camadas.into_iter().flatten() {
            if let Some(v) = camada.get(campo) {
                return match v {
                    crate::celulas::estilo_parcial::Campo::Texto(t) => Some(t.clone()),
                    _ => None,
                };
            }
        }
        None
    }

    /// O estilo que vale na célula: a aba, a coluna, a linha e a célula, uma
    /// por cima da outra. É o `getStyle` do TypeScript.
    pub fn estilo_mesclado(&self, row: u32, col: u32) -> Option<EstiloParcial> {
        let direto = self.estilo_direto(key(row, col));
        let coluna = self.col_styles.get(&col);
        let linha = self.row_styles.get(&row);
        if direto.is_none() && coluna.is_none() && linha.is_none() && self.sheet_style.is_none() {
            return None;
        }
        let mut r = self.sheet_style.clone();
        for camada in [coluna, linha, direto].into_iter().flatten() {
            r = Some(mesclar(r.as_ref(), camada));
        }
        Some(r.unwrap_or_default())
    }

    /// O estilo mesclado, resolvido para formatar.
    pub fn estilo_resolvido(&self, row: u32, col: u32) -> Option<crate::celulas::style::Style> {
        self.estilo_mesclado(row, col).map(|e| crate::celulas::estilo_parcial::para_style(&e))
    }

    /// Se a célula tem conteúdo — sem calcular nada. `hasContent` do
    /// TypeScript (a navegação por Ctrl+Seta pergunta isto linha a linha).
    pub fn has_content(&self, row: u32, col: u32) -> bool {
        let k = key(row, col);
        if let Some(c) = self.cells.get(&k) {
            return !c.raw.is_empty();
        }
        self.data.has_cell(row, col) || self.formulas.has(row, col)
    }

    /// As sequências `(primeira, última)` de linhas seguidas com conteúdo na
    /// coluna, de 0 até `ate` — o mesmo que [`Self::has_content`] linha a
    /// linha, mas lendo as máscaras de presença bloco a bloco (8 linhas por
    /// byte). É o que deixa a Região atual e o Ctrl+T rápidos em 13 milhões
    /// de linhas: perguntar célula a célula travava a janela (17/10/2026).
    pub fn sequencias_com_conteudo(&self, col: u32, ate: u32) -> Vec<(u32, u32)> {
        use crate::planilha::column_store::{CHUNK_SHIFT, CHUNK_SIZE};
        // Células soltas mandam na linha delas, como no `has_content`.
        let mut soltas: Vec<(u32, bool)> = self.cells.iter()
            .filter(|(k, _)| key_col(**k) == col && key_row(**k) <= ate)
            .map(|(k, c)| (key_row(*k), !c.raw.is_empty()))
            .collect();
        soltas.sort_unstable_by_key(|s| s.0);
        let mut soltas = soltas.into_iter().peekable();
        let mut sequencias: Vec<(u32, u32)> = Vec::new();
        let mut marca = |r0: u32, r1: u32| match sequencias.last_mut() {
            Some(ultima) if ultima.1 + 1 == r0 => ultima.1 = r1,
            _ => sequencias.push((r0, r1)),
        };
        let mut mascara = vec![0u8; CHUNK_SIZE >> 3];
        for bloco in 0..=(ate >> CHUNK_SHIFT) as usize {
            let base = (bloco as u32) << CHUNK_SHIFT;
            let partes = self.data.mascaras_do_bloco(col, bloco);
            let formulas = self.formulas.mascara_do_bloco(col, bloco);
            let com_solta = soltas.peek().is_some_and(|s| s.0 >> CHUNK_SHIFT == bloco as u32);
            if partes.iter().all(Option::is_none) && formulas.is_none() && !com_solta {
                continue;
            }
            mascara.fill(0);
            for m in partes.into_iter().chain([formulas]).flatten() {
                for (destino, bits) in mascara.iter_mut().zip(m) { *destino |= bits; }
            }
            while let Some(&(r, cheia)) = soltas.peek() {
                if r >> CHUNK_SHIFT != bloco as u32 { break; }
                let o = (r - base) as usize;
                if cheia { mascara[o >> 3] |= 1 << (o & 7); } else { mascara[o >> 3] &= !(1 << (o & 7)); }
                soltas.next();
            }
            let ultimo_byte = ((ate - base).min(CHUNK_SIZE as u32 - 1) >> 3) as usize;
            for (i, &byte) in mascara[..=ultimo_byte].iter().enumerate() {
                let linha = base + ((i as u32) << 3);
                match byte {
                    0 => {}
                    0xff => marca(linha, linha + 7),
                    _ => for b in 0..8 {
                        if byte & (1 << b) != 0 { marca(linha + b, linha + b); }
                    },
                }
            }
        }
        // O último byte pode passar de `ate`.
        while let Some(ultima) = sequencias.last_mut() {
            if ultima.0 > ate { sequencias.pop(); continue; }
            ultima.1 = ultima.1.min(ate);
            break;
        }
        sequencias
    }

    /// A região contígua em volta da célula — o que o Ctrl+T do Excel pega, e
    /// a origem da tabela dinâmica e do filtro. Tradução do `regiaoAtual` do
    /// `table.ts` (sem diagonal; célula vazia devolve só ela). Pela ponte ele
    /// perguntava "tem conteúdo?" célula a célula: num CSV de 1,3 milhão de
    /// linhas, dezenas de milhões de idas antes de a dinâmica começar.
    pub fn regiao_da_tabela(&mut self, row: u32, col: u32) -> Rect {
        let (max_r, max_c) = (self.max_row_ts(), self.max_col_ts());
        if !self.has_content(row, col) {
            return Rect::new(row, col, row, col);
        }
        let (mut r0, mut r1, mut c0, mut c1) = (row, row, col, col);
        // Pelas sequências de linhas cheias de cada coluna (lidas das
        // máscaras, bloco a bloco), não célula a célula: o gráfico a partir
        // de uma célula de 13 milhões de linhas travava como o Ctrl+T.
        let mut colunas = crate::operacoes::ops::regiao::Colunas::novas(self, max_r);
        let mut cresceu = true;
        while cresceu {
            cresceu = false;
            if let Some(novo) = r0.checked_sub(1).and_then(|r| colunas.bloco_de_linhas(r, c0, c1, false)) {
                r0 = novo;
                cresceu = true;
            }
            if let Some(novo) = (r1 < max_r).then(|| colunas.bloco_de_linhas(r1 + 1, c0, c1, true)).flatten() {
                r1 = novo.min(max_r);
                cresceu = true;
            }
            while c0 > 0 && colunas.tem_na_faixa(c0 - 1, r0, r1) {
                c0 -= 1;
                cresceu = true;
            }
            while c1 < max_c && colunas.tem_na_faixa(c1 + 1, r0, r1) {
                c1 += 1;
                cresceu = true;
            }
        }
        Rect::new(r0, c0, r1, c1)
    }

    /// Nada: nem célula, nem célula só de estilo.
    pub fn is_empty_de_tudo(&self) -> bool {
        self.is_empty() && self.styles.is_empty()
    }

    /// Se a célula guarda uma fórmula.
    pub fn is_formula(&self, row: u32, col: u32) -> bool {
        let k = key(row, col);
        match self.cells.get(&k) {
            Some(c) => c.raw.starts_with('=') && c.raw.len() > 1,
            None => self.formulas.has(row, col),
        }
    }

    /// As chaves de tudo que existe dentro do retângulo, nas quatro moradas.
    pub fn chaves_no_retangulo(&self, r: &Rect) -> Vec<CellKey> {
        let dentro = |row: u32, col: u32| row >= r.r0 && row <= r.r1 && col >= r.c0 && col <= r.c1;
        let mut out: HashSet<CellKey> = HashSet::new();
        for &k in self.cells.keys() {
            if dentro(key_row(k), key_col(k)) {
                out.insert(k);
            }
        }
        // Estilo direto por faixa: só as colunas do retângulo são visitadas.
        out.extend(self.styles.chaves_no_retangulo(r.r0, r.c0, r.r1, r.c1));
        self.data.for_each_in_range(r, &self.pool, &mut |_, row, col| {
            out.insert(key(row, col));
        });
        self.formulas.for_each_in_range(r.r0, r.r1, r.c0, r.c1, |row, col| {
            out.insert(key(row, col));
        });
        let mut v: Vec<CellKey> = out.into_iter().collect();
        v.sort_unstable();
        v
    }

    /// Passeia pelas células ocupadas com o texto e o estilo direto de cada
    /// uma — o `forEachCell` do TypeScript, que o `ops.ts` usa para mover
    /// linha e coluna. A célula só de estilo vem com texto vazio.
    pub fn for_each_snapshot(&self, visit: &mut dyn FnMut(u32, u32, &Snap)) {
        let mut vistas: HashSet<CellKey> = HashSet::new();
        for &k in self.cells.keys() {
            if let Some(s) = self.snapshot(key_row(k), key_col(k)) {
                visit(key_row(k), key_col(k), &s);
            }
            vistas.insert(k);
        }
        let mut literais = Vec::new();
        self.data.for_each(&self.pool, &mut |row, col, _| literais.push((row, col)));
        for (row, col) in literais {
            let k = key(row, col);
            if vistas.insert(k) {
                if let Some(s) = self.snapshot(row, col) {
                    visit(row, col, &s);
                }
            }
        }
        for e in self.formulas.entries() {
            if vistas.insert(key(e.row, e.col)) {
                visit(e.row, e.col, &Snap::texto(e.raw.clone()));
            }
        }
        for k in self.styles.keys() {
            if vistas.insert(k) {
                if let Some(s) = self.snapshot(key_row(k), key_col(k)) {
                    visit(key_row(k), key_col(k), &s);
                }
            }
        }
    }

    // ------------------------------------------------------------- limites

    /// A maior linha ocupada, como o `maxRow` do TypeScript (zero numa aba
    /// vazia). Recalcula se algo saiu da borda.
    /// Ctrl+seta: até onde o salto vai — o `edgeJump` do `ui/grade/grid.ts`,
    /// traduzido linha por linha. Mora aqui porque, pela ponte, perguntar
    /// "tem conteúdo?" célula a célula eram dezenas de milhares de idas numa
    /// planilha de dois milhões de linhas, e a janela congelava (bug de
    /// 23/09/2026). `limite_r`/`limite_c` são os da grade.
    pub fn borda_dos_dados(&mut self, row: u32, col: u32, dr: i32, dc: i32, limite_r: u32, limite_c: u32) -> (u32, u32) {
        let (max_row, max_col) = (self.max_row_ts() as i64, self.max_col_ts() as i64);
        let (lim_r, lim_c) = (limite_r as i64, limite_c as i64);
        let (dr, dc) = (dr as i64, dc as i64);
        let dentro = |r: i64, c: i64| r >= 0 && r <= lim_r && c >= 0 && c <= lim_c;
        let cheia = |s: &Self, r: i64, c: i64| s.has_content(r as u32, c as u32);
        let (mut r, mut c) = (row as i64, col as i64);
        if !dentro(r + dr, c + dc) {
            return (row, col);
        }
        let comeca_cheia = cheia(self, r + dr, c + dc);
        r += dr;
        c += dc;

        // No vazio, salta direto o vácuo de milhões de linhas.
        if !comeca_cheia {
            if dr > 0 && r > max_row {
                return (lim_r as u32, c as u32);
            }
            if dr < 0 && r > max_row {
                r = max_row.min(lim_r);
                if cheia(self, r, c) {
                    return (r as u32, c as u32);
                }
            }
            if dc > 0 && c > max_col {
                return (r as u32, lim_c as u32);
            }
            if dc < 0 && c > max_col {
                c = max_col.min(lim_c);
                if cheia(self, r, c) {
                    return (r as u32, c as u32);
                }
            }
        }

        while dentro(r + dr, c + dc) && cheia(self, r + dr, c + dc) == comeca_cheia {
            r += dr;
            c += dc;
        }
        if !comeca_cheia && dentro(r + dr, c + dc) {
            r += dr;
            c += dc;
        }
        (r as u32, c as u32)
    }

    pub fn max_row_ts(&mut self) -> u32 {
        self.acertar_limites();
        if self.is_empty() && self.styles.is_empty() { 0 } else { self.max_row }
    }

    pub fn max_col_ts(&mut self) -> u32 {
        self.acertar_limites();
        if self.is_empty() && self.styles.is_empty() { 0 } else { self.max_col }
    }

    /// `recomputeBounds`: as quatro moradas, mais a célula só de estilo.
    pub fn acertar_limites(&mut self) {
        if !self.bounds_dirty {
            return;
        }
        let (mut rmax, mut cmax) = (0u32, 0u32);
        for &k in self.cells.keys() {
            rmax = rmax.max(key_row(k));
            cmax = cmax.max(key_col(k));
        }
        if let Some((r, c)) = self.styles.limites() {
            rmax = rmax.max(r);
            cmax = cmax.max(c);
        }
        if self.data.size() > 0 {
            rmax = rmax.max(self.data.max_row());
            cmax = cmax.max(self.data.max_col());
        }
        if self.formulas.size() > 0 {
            let b = self.formulas.bounds();
            rmax = rmax.max(b.max_row.max(0) as u32);
            cmax = cmax.max(b.max_col.max(0) as u32);
        }
        self.max_row = rmax;
        self.max_col = cmax;
        // O mínimo deixa de ser exato: zero é um retângulo que contém tudo,
        // e é o que o `usedRect` do TypeScript sempre devolveu.
        self.min_row = 0;
        self.min_col = 0;
        self.bounds_dirty = false;
    }

    pub(crate) fn cresce_limites(&mut self, row: u32, col: u32) {
        self.min_row = self.min_row.min(row);
        self.min_col = self.min_col.min(col);
        self.max_row = self.max_row.max(row);
        self.max_col = self.max_col.max(col);
    }

    // ------------------------------------------------------------- escrita

    /// O `apply` do TypeScript: põe `next` na célula, guardando o antes para
    /// o desfazer quando `record` e há transação aberta. Devolve se mudou.
    pub fn apply(&mut self, row: u32, col: u32, next: Option<Snap>, record: bool) -> bool {
        self.tocar();
        let k = key(row, col);
        let antes = self.snapshot(row, col);
        if antes.is_none() && next.is_none() {
            return false;
        }
        if let (Some(a), Some(n)) = (&antes, &next) {
            let mesmo_estilo = (!tem_estilo(a.style.as_ref()) && !tem_estilo(n.style.as_ref())) || a.style == n.style;
            if a.raw == n.raw && mesmo_estilo {
                return false;
            }
        }
        if record {
            if let Some(p) = self.hist.pendente.as_mut() {
                p.push(Passo::Celula(Change { k, antes, depois: next.clone() }));
            }
        }

        // Limpa as quatro moradas; o ramo abaixo põe de volta onde couber.
        self.set_cell_raw(row, col, String::new(), None);
        if row == self.max_row || col == self.max_col {
            self.bounds_dirty = true;
        }

        let Some(next) = next else {
            return true;
        };
        let idx = next.style.clone().and_then(|s| self.registrar_parcial(s));
        if next.raw.is_empty() {
            // Célula vazia com só estilo.
            if let Some(i) = idx {
                self.styles.insert(k, i);
                self.cresce_limites(row, col);
            }
            return true;
        }
        if next.raw.starts_with('=') {
            self.set_cell_raw(row, col, next.raw, idx);
            return true;
        }
        let valor = crate::celulas::input::parse_literal(&next.raw).value;
        if let Value::Error(_) = valor {
            // Erro digitado ("#N/A"): não cabe na coluna de dado.
            self.cells.insert(k, CellData { raw: next.raw, formula: None, value: valor, style_idx: idx });
            self.cresce_limites(row, col);
            self.mark_dirty_cascade(k);
            return true;
        }
        let volta = round_trips(&next.raw, &valor, next.style.as_ref());
        let raw = next.raw.clone();
        self.set_cell_raw(row, col, next.raw, idx);
        // Guarda o texto digitado quando a leitura não o reconstruiria. Não
        // basta o `round_trips`: ele formata com o estilo (a data volta
        // "23/09/2026"), mas o `get_raw` da coluna de dados lê sem estilo e
        // devolveria o número de série — foi o 46288 no lugar da data (23/09/2026).
        if self.data.has_cell(row, col) && (!volta || self.data.get_raw(row, col, &self.pool) != raw) {
            self.textos.insert(k, &raw);
        }
        true
    }

    /// `styleFor`: o estilo com que um texto digitado entra — o atual, com o
    /// formato implícito ("10%" pede porcentagem) quando ainda não há um.
    pub fn style_for(&self, row: u32, col: u32, raw: &str) -> Option<EstiloParcial> {
        let atual = self.estilo_mesclado(row, col);
        if let Some(f) = formato(atual.as_ref()) {
            if f != "auto" {
                return atual;
            }
        }
        match implied_format(raw) {
            Some(f) => {
                let mut patch = EstiloParcial::new();
                patch.insert("format".into(), Campo::Texto(f.into()));
                Some(mesclar(atual.as_ref(), &patch))
            }
            None => match raw.strip_prefix('=').and_then(|corpo| self.formato_herdado(corpo)) {
                Some(patch) => Some(mesclar(atual.as_ref(), &patch)),
                None => atual,
            },
        }
    }

    /// Como o Excel: a fórmula sem formato próprio leva o formato de número
    /// da primeira célula que cita (`=A1+2` com A1 data dá data). Só em
    /// conta simples e em SOMA/MÍNIMO/MÁXIMO/MÉDIA: `=ANO(A1)` é número, e
    /// referência a outra aba não conta.
    fn formato_herdado(&self, corpo: &str) -> Option<EstiloParcial> {
        use crate::formula::tokenizer::{tokenize, TokenType};
        const MANTEM: [&str; 10] = ["SOMA", "SUM", "MÍNIMO", "MINIMO", "MIN", "MÁXIMO", "MAXIMO", "MAX", "MÉDIA", "AVERAGE"];
        let tokens = tokenize(corpo).ok()?;
        if tokens.iter().any(|t| t.kind == TokenType::Ident && !MANTEM.contains(&t.text.to_uppercase().as_str())) {
            return None;
        }
        let primeira = tokens.iter().find(|t| t.kind == TokenType::Ref)?;
        if primeira.text.contains('!') { return None; }
        let (linha, coluna) = crate::celulas::refs::parse_cell_ref(&primeira.text)?;
        let origem = self.estilo_mesclado(linha, coluna)?;
        let nome = formato(Some(&origem)).filter(|f| f != "auto" && f != "general" && f != "text")?;
        let mut patch = EstiloParcial::new();
        patch.insert("format".into(), Campo::Texto(nome));
        for chave in ["decimals", "numFmt"] {
            if let Some(v) = origem.get(chave) { patch.insert(chave.into(), v.clone()); }
        }
        Some(patch)
    }

    /// Delete do Excel: apaga só o conteúdo (valor/fórmula) e mantém o
    /// estilo da célula. Validação e comentário moram fora da célula.
    pub fn limpar_conteudo(&mut self, row: u32, col: u32) {
        self.abrir_transacao();
        let style = self.estilo_direto(key(row, col)).cloned().filter(|s| tem_estilo(Some(s)));
        let next = style.map(|style| Snap { raw: String::new(), style: Some(style) });
        self.apply(row, col, next, true);
        self.fechar_transacao();
    }

    /// `setCell`: o que a pessoa digitou.
    pub fn set_cell(&mut self, row: u32, col: u32, raw: &str) {
        self.abrir_transacao();
        let next = if raw.is_empty() {
            None
        } else {
            Some(Snap { raw: raw.to_string(), style: self.style_for(row, col, raw) })
        };
        self.apply(row, col, next, true);
        self.fechar_transacao();
    }

    /// Reutiliza o formato já detectado no `parse_literal` de uma
    /// conversão em massa. `style_for` executa `implied_format(raw)`,
    /// que chamaria `parse_literal` novamente para CADA campo.
    fn style_for_divisao_parseado(&self, row: u32, col: u32,
        detectado: Option<crate::celulas::input::NumberFormat>
    ) -> Option<EstiloParcial> {
        let atual = self.estilo_mesclado(row, col);
        if formato(atual.as_ref()).is_some_and(|f| f != "auto") {
            return atual;
        }
        match formato_do_literal(detectado) {
            Some(f) => {
                let mut patch = EstiloParcial::new();
                patch.insert("format".into(), Campo::Texto(f.into()));
                Some(mesclar(atual.as_ref(), &patch))
            }
            None => atual,
        }
    }

    /// Escrita de um campo resultante de "Texto para Colunas".
    ///
    /// O caminho geral `set_cell -> apply -> set_cell_raw` limpava e
    /// reinterpretava o literal mais de uma vez, alocava cópias temporárias
    /// e sujava o cache de cálculo mesmo numa coluna sem fórmulas.
    /// Aqui parseamos UMA vez, escrevemos direto na DataTable e mantemos
    /// exatamente o mesmo Snap de undo/redo da edição normal.
    /// Exige a transação de lote já aberta pelo chamador.
    pub fn set_cell_divisao(&mut self, row: u32, col: u32, raw: &str) {
        if raw.is_empty() || raw.starts_with('=') {
            self.set_cell(row, col, raw);
            return;
        }
        let analisado = crate::celulas::input::parse_literal(raw);
        if matches!(analisado.value, Value::Error(_)) {
            self.set_cell(row, col, raw);
            return;
        }
        let style = self.style_for_divisao_parseado(row, col, analisado.format);
        let valor = analisado.value;
        let antes = self.snapshot(row, col);
        let depois = Snap { raw: raw.to_owned(), style };
        if antes.as_ref().is_some_and(|a| {
            a.raw == depois.raw &&
                ((!tem_estilo(a.style.as_ref()) && !tem_estilo(depois.style.as_ref()))
                    || a.style == depois.style)
        }) {
            return;
        }
        // Calcular as decisões ANTES de mover o snapshot para o histórico:
        // evita um clone adicional do String e da árvore de estilos por célula.
        let idx = depois.style.clone().and_then(|s| self.registrar_parcial(s));
        let preserva_raw = !round_trips(raw, &valor, depois.style.as_ref()) ||
            match &valor {
                Value::Text(texto) => texto != raw,
                Value::Number(numero) => to_js_string(*numero) != raw,
                Value::Bool(true) => raw != "VERDADEIRO",
                Value::Bool(false) => raw != "FALSO",
                _ => true,
            };
        let k = key(row, col);
        if let Some(pendente) = self.hist.pendente.as_mut() {
            pendente.push(Passo::Celula(Change {
                k, antes, depois: Some(depois),
            }));
        }
        self.soltar_para_literal(row, col);
        self.data.set_cell_value(row, col, &valor, &mut self.pool);
        self.cresce_limites(row, col);
        if let Some(i) = idx {
            self.styles.insert(k, i);
        }
        if preserva_raw {
            self.textos.insert(k, raw);
        }
        // Há dependências? Só então propagar a cascata. A escrita de um
        // milhão de literais sem fórmulas não precisa de dirty HashSet.
        if self.alguma_formula_le() {
            self.mark_dirty_cascade(k);
        }
    }

    /// As três perguntas do `setStyle`/`clear`: é a aba toda, uma coluna
    /// inteira ou uma linha inteira?
    fn forma_da_area(&mut self, r: &Rect) -> (bool, bool, bool) {
        let max_row = self.max_row_ts();
        let max_col = self.max_col_ts();
        let (todas_linhas, todas_colunas) = (r.r1 >= MAX_ROWS - 1, r.c1 >= MAX_COLS - 1);
        // "Cobre a área usada" é o Ctrl+A, e só ele: uma coluna ou linha
        // inteira também cobre a área quando os dados moram nela, e virava a
        // aba inteira (bug de 23/09/2026 — o TypeScript tem o mesmo).
        let aba = r.r0 == 0
            && r.c0 == 0
            && ((todas_linhas && todas_colunas)
                || (!todas_linhas
                    && !todas_colunas
                    && r.r1 >= max_row
                    && r.c1 >= max_col
                    && (r.r1 >= 50 || r.c1 >= 20)));
        let coluna = r.r0 == 0 && (r.r1 >= MAX_ROWS - 1 || (r.r1 >= max_row && r.r1 >= 50));
        let linha = r.c0 == 0 && (r.c1 >= MAX_COLS - 1 || (r.c1 >= max_col && r.c1 >= 20));
        (aba, coluna, linha)
    }

    fn area(r: &Rect) -> u64 {
        (r.r1 - r.r0 + 1) as u64 * (r.c1 - r.c0 + 1) as u64
    }

    /// `setStyle`: pinta a área. Aba, coluna e linha inteiras viram estilo
    /// de aba/coluna/linha (sem desfazer, como no TypeScript); o resto vai
    /// célula a célula.
    pub fn set_style(&mut self, area: &Rect, patch: &EstiloParcial) {
        let r = area.normalized();
        let (aba, coluna, linha) = self.forma_da_area(&r);
        if aba || coluna || linha {
            self.rastro.borrow_mut().desconhecer();
        }
        self.abrir_transacao();
        if aba {
            self.sheet_style = Some(mesclar(self.sheet_style.as_ref(), patch));
            self.col_styles.clear();
            self.row_styles.clear();
        } else if coluna {
            for c in r.c0..=r.c1 {
                let atual = self.col_styles.get(&c).or(self.sheet_style.as_ref());
                let novo = mesclar(atual, patch);
                if novo.is_empty() {
                    self.col_styles.remove(&c);
                } else {
                    self.col_styles.insert(c, novo);
                }
            }
        } else if linha {
            for l in r.r0..=r.r1 {
                let atual = self.row_styles.get(&l).or(self.sheet_style.as_ref());
                let novo = mesclar(atual, patch);
                if novo.is_empty() {
                    self.row_styles.remove(&l);
                } else {
                    self.row_styles.insert(l, novo);
                }
            }
        } else {
            let chaves: Vec<CellKey> = if Self::area(&r) <= AREA_PEQUENA {
                let mut v = Vec::new();
                for row in r.r0..=r.r1 {
                    for col in r.c0..=r.c1 {
                        v.push(key(row, col));
                    }
                }
                v
            } else {
                self.chaves_no_retangulo(&r)
            };
            for k in chaves {
                let (row, col) = (key_row(k), key_col(k));
                let raw = self.get_raw(row, col);
                let style = mesclar(self.estilo_mesclado(row, col).as_ref(), patch);
                self.apply(row, col, Some(Snap { raw, style: Some(style) }), true);
            }
        }
        self.fechar_transacao();
        // A cor mudou: SOMAR.POR.COR e CONT.SE.COR (voláteis) recalculam.
        self.sujar_volateis();
    }

    /// `clear`: apaga conteúdo e estilo da área.
    pub fn clear(&mut self, area: &Rect) {
        let r = area.normalized();
        let (aba, coluna, linha) = self.forma_da_area(&r);
        if aba || coluna || linha {
            self.rastro.borrow_mut().desconhecer();
        }
        self.abrir_transacao();
        if aba {
            self.sheet_style = None;
            self.col_styles.clear();
            self.row_styles.clear();
        } else if coluna {
            for c in r.c0..=r.c1 {
                self.col_styles.remove(&c);
            }
        } else if linha {
            for l in r.r0..=r.r1 {
                self.row_styles.remove(&l);
            }
        }
        let pequena = !aba && !coluna && !linha && Self::area(&r) <= AREA_PEQUENA;
        let chaves: Vec<CellKey> = if pequena {
            let mut v = Vec::new();
            for row in r.r0..=r.r1 {
                for col in r.c0..=r.c1 {
                    let k = key(row, col);
                    if self.cells.contains_key(&k)
                        || self.data.has_cell(row, col)
                        || self.formulas.has(row, col)
                        || self.styles.contains_key(&k)
                    {
                        v.push(k);
                    }
                }
            }
            v
        } else {
            self.chaves_no_retangulo(&r)
        };
        for k in chaves {
            self.apply(key_row(k), key_col(k), None, true);
        }
        self.fechar_transacao();
    }

    /// O que o `set_cell_raw(.., "")` faz antes de um literal entrar pelo
    /// colar rápido — tirar a célula das outras moradas e do cálculo —, sem a
    /// cascata de "sujo", que quem chama decide.
    fn soltar_para_literal(&mut self, row: u32, col: u32) {
        let k = key(row, col);
        self.rastro.borrow_mut().anotar(k);
        self.deps.unlink(k, &mut |_, _| {});
        self.formulas.clear_cell(row, col);
        self.textos.remove(&k);
        self.externas.remove(&k);
        self.cells.remove(&k);
        self.styles.remove(&k);
        let mut calc = self.calc.borrow_mut();
        calc.cache.remove(&k);
        calc.dirty.remove(&k);
    }

    /// Se existe fórmula nesta aba que leia outra célula: o grafo (fórmula
    /// avulsa, por célula ou intervalo) ou a coluna de fórmulas empacotadas.
    fn alguma_formula_le(&self) -> bool {
        !self.deps.dependents.is_empty() || !self.deps.range_deps.is_empty() || self.formulas.size() > 0
    }

    /// `paste`: um bloco de textos a partir de (`row`, `col`). Sem desfazer é
    /// o caminho da importação — ver a nota do topo sobre o "007".
    pub fn paste(&mut self, row: u32, col: u32, linhas: &[Vec<String>], record: bool) {
        let max_r = (linhas.len() as u32).min(MAX_ROWS - row);
        if record {
            self.abrir_transacao();
        }
        for r in 0..max_r {
            let linha = &linhas[r as usize];
            let max_c = (linha.len() as u32).min(MAX_COLS - col);
            for c in 0..max_c {
                let raw = &linha[c as usize];
                let (rr, cc) = (row + r, col + c);
                if raw.is_empty() {
                    self.apply(rr, cc, None, record);
                } else if record {
                    let style = self.style_for(rr, cc, raw);
                    self.apply(rr, cc, Some(Snap { raw: raw.clone(), style }), true);
                } else if raw.starts_with('=') && raw.len() > 1 {
                    self.apply(rr, cc, Some(Snap::texto(raw.clone())), false);
                } else if implied_format(raw).is_some() {
                    // Data, hora, porcentagem, moeda: o caminho completo, que
                    // guarda o formato e o texto digitado. Pelo atalho abaixo a
                    // data de um CSV virava o número de série sem formato
                    // (46288) — bug do TypeScript também, consertado nos dois
                    // (23/09/2026). Número puro continua rápido.
                    let style = self.style_for(rr, cc, raw);
                    self.apply(rr, cc, Some(Snap { raw: raw.clone(), style }), false);
                } else {
                    self.soltar_para_literal(rr, cc);
                    let v = valor_rapido(raw);
                    self.data.set_cell_value(rr, cc, &v, &mut self.pool);
                    self.cresce_limites(rr, cc);
                    // Só suja quando há fórmula que possa ler a célula. O
                    // literal em si não recalcula, e sujá-lo punha cada
                    // célula de um CSV no conjunto `dirty` — 26 milhões de
                    // chaves e ~600 ns por célula (23/09/2026).
                    if self.alguma_formula_le() {
                        self.mark_dirty_cascade(key(rr, cc));
                    }
                }
            }
        }
        if record {
            self.fechar_transacao();
        }
    }

    // -------------------------------------------------------- dimensões

    fn aplicar_ocultas_desde(&mut self, coluna: bool, valor: Option<u32>) {
        if coluna { self.colunas_ocultas_desde = valor } else { self.linhas_ocultas_desde = valor }
    }

    /// Grava "ocultas a partir de `valor`" com um passo de desfazer.
    fn definir_ocultas_desde(&mut self, coluna: bool, valor: Option<u32>) -> bool {
        let antes = if coluna { self.colunas_ocultas_desde } else { self.linhas_ocultas_desde };
        if antes == valor { return false; }
        self.abrir_transacao();
        if let Some(pendente) = self.hist.pendente.as_mut() {
            pendente.push(Passo::OcultasDesde { coluna, antes, depois: valor });
        }
        self.aplicar_ocultas_desde(coluna, valor);
        self.fechar_transacao();
        true
    }

    fn aplicar_dimensao(&mut self, coluna: bool, indice: u32, valor: Option<u32>) {
        let mapa = if coluna { &mut self.col_widths } else { &mut self.row_heights };
        if let Some(valor) = valor { mapa.insert(indice, valor); }
        else { mapa.remove(&indice); }
    }

    /// Redimensiona uma linha ou coluna, registrando um passo de histórico.
    /// Retorna false quando o índice é inválido ou não houve mudança.
    pub fn definir_dimensao(&mut self, coluna: bool, indice: u32, valor: u32) -> bool {
        if indice >= if coluna { MAX_COLS } else { MAX_ROWS } { return false; }
        let valor = valor.clamp(1, 10_000);
        let antes = if coluna { self.col_widths.get(&indice) } else { self.row_heights.get(&indice) }.copied();
        if antes == Some(valor) { return false; }
        self.abrir_transacao();
        if let Some(pendente) = self.hist.pendente.as_mut() {
            pendente.push(Passo::Dimensao { coluna, indice, antes, depois: Some(valor) });
        }
        self.aplicar_dimensao(coluna, indice, Some(valor));
        self.fechar_transacao();
        true
    }

    /// Muda a largura padrão das colunas da aba (as que não têm largura
    /// própria), num passo de desfazer. Falso quando nada muda.
    pub fn definir_largura_padrao(&mut self, valor: Option<u32>) -> bool {
        let valor = valor.map(|v| v.clamp(1, 2_000));
        if self.largura_padrao == valor { return false; }
        self.abrir_transacao();
        if let Some(pendente) = self.hist.pendente.as_mut() {
            pendente.push(Passo::LarguraPadrao { antes: self.largura_padrao, depois: valor });
        }
        self.largura_padrao = valor;
        self.fechar_transacao();
        true
    }

    /// Muda uma dimensão dentro da transação aberta, com o passo de desfazer.
    /// Público para o AutoFiltro da GUI: as alturas zeradas pelo filtro e o
    /// JSON da tabela entram no mesmo Ctrl+Z ([`Self::alterar_tabelas`]).
    pub fn registrar_dimensao(&mut self, coluna: bool, indice: u32, depois: Option<u32>) {
        let antes = if coluna { self.col_widths.get(&indice) } else { self.row_heights.get(&indice) }.copied();
        if antes == depois { return; }
        if let Some(pendente) = self.hist.pendente.as_mut() {
            pendente.push(Passo::Dimensao { coluna, indice, antes, depois });
        }
        self.aplicar_dimensao(coluna, indice, depois);
    }

    /// Empilha o passo de um comentário na transação aberta (a troca já foi
    /// feita por [`Sheet::registrar_comentario`]).
    pub(crate) fn empilhar_comentario(
        &mut self, linha: u32, coluna: u32,
        antes: Option<super::comentarios::Comentario>, depois: Option<super::comentarios::Comentario>,
    ) {
        if let Some(pendente) = self.hist.pendente.as_mut() {
            pendente.push(Passo::Comentario { linha, coluna, antes: antes.map(Box::new), depois: depois.map(Box::new) });
        }
    }

    /// Troca a estrutura de tópicos dentro da transação aberta.
    pub(crate) fn registrar_topicos(&mut self, depois: super::topicos::Topicos) {
        if self.topicos == depois { return; }
        let antes = std::mem::replace(&mut self.topicos, depois.clone());
        if let Some(pendente) = self.hist.pendente.as_mut() {
            pendente.push(Passo::Topicos { antes: Box::new(antes), depois: Box::new(depois) });
        }
    }

    /// Oculta (altura/largura zero) ou reexibe linhas/colunas, compatível com
    /// `ui/app/estrutura.ts`. Mantém as células intactas e registra uma
    /// transação de dimensões, desfazível com um único Ctrl+Z.
    ///
    /// Ao reexibir, inclui faixas ocultas contíguas à seleção e remove suas
    /// entradas do mapa: a dimensão volta ao padrão da planilha, como no TS.
    pub fn alterar_visibilidade_dimensoes(
        &mut self, coluna: bool, ocultar: bool, a: u32, b: u32,
    ) -> Result<usize, String> {
        // Uma transação só: reexibir a faixa do fim e as ocultas avulsas
        // encostadas desfaz com um Ctrl+Z.
        self.abrir_transacao();
        let r = self.visibilidade_dimensoes(coluna, ocultar, a, b);
        self.fechar_transacao();
        r
    }

    fn visibilidade_dimensoes(
        &mut self, coluna: bool, ocultar: bool, a: u32, b: u32,
    ) -> Result<usize, String> {
        let limite_eixo = if coluna { MAX_COLS } else { MAX_ROWS };
        let inicio = a.min(b);
        let fim = a.max(b);
        if fim >= limite_eixo {
            return Err("O intervalo ultrapassa os limites da planilha".into());
        }
        // Até o fim da planilha (ex.: da linha 30 até a 100.000.000): um
        // número só, sem laço. Excel também deixa ocultar tudo abaixo.
        let desde_atual = if coluna { self.colunas_ocultas_desde } else { self.linhas_ocultas_desde };
        if ocultar && fim == limite_eixo - 1 {
            if inicio == 0 {
                return Err("Mantenha ao menos uma linha ou coluna visível".into());
            }
            let novo = desde_atual.map_or(inicio, |n| n.min(inicio));
            if desde_atual == Some(novo) { return Ok(0); }
            self.definir_ocultas_desde(coluna, Some(novo));
            return Ok((limite_eixo - novo) as usize);
        }
        // Reexibir encostado na faixa oculta do fim (selecionar a linha 29 e
        // a de baixo, ou "tudo"): a faixa inteira volta.
        let mut reexibidas_do_fim = 0usize;
        if !ocultar {
            if let Some(n) = desde_atual.filter(|&n| u64::from(fim) + 1 >= u64::from(n)) {
                self.definir_ocultas_desde(coluna, None);
                reexibidas_do_fim = (limite_eixo - n) as usize;
            }
        }
        // Cada dimensão oculta é uma entrada no mapa e um passo de desfazer.
        // 200 mil não deixava ocultar metade de uma planilha de 1 milhão de
        // linhas (16/10); 2,1 milhões cobre a planilha inteira do Excel.
        const MAX_OPERACOES: u64 = 2_100_000;
        let mapa = if coluna { &self.col_widths } else { &self.row_heights };
        let indices: Vec<u32> = if ocultar {
            if u64::from(fim) - u64::from(inicio) + 1 > MAX_OPERACOES {
                return Err("Seleção grande demais para ocultação individual; reduza o intervalo".into());
            }
            (inicio..=fim).filter(|i| mapa.get(i).copied() != Some(0)).collect()
        } else {
            // Estender até os zeros encostados à seleção, sem percorrer
            // milhões de linhas vazias quando o usuário selecionou um eixo.
            let mut de = inicio;
            while de > 0 && mapa.get(&(de - 1)) == Some(&0) { de -= 1; }
            let mut ate = fim;
            while ate < limite_eixo - 1 && mapa.get(&(ate + 1)) == Some(&0) { ate += 1; }
            let mut ocultos: Vec<_> = mapa.iter()
                .filter_map(|(&i, &v)| (v == 0 && i >= de && i <= ate).then_some(i))
                .collect();
            if ocultos.len() as u64 > MAX_OPERACOES {
                return Err("Muitas dimensões ocultas para reexibir numa operação".into());
            }
            ocultos.sort_unstable();
            ocultos
        };

        if indices.is_empty() { return Ok(reexibidas_do_fim); }
        if ocultar {
            let restantes_ocultos = mapa.values().filter(|&&px| px == 0).count() as u64;
            if restantes_ocultos + indices.len() as u64 >= limite_eixo as u64 {
                return Err("Mantenha ao menos uma linha ou coluna visível".into());
            }
        }
        self.abrir_transacao();
        for indice in &indices {
            let antes = if coluna { self.col_widths.get(indice) }
                else { self.row_heights.get(indice) }.copied();
            let depois = if ocultar { Some(0) } else { None };
            if let Some(pendente) = self.hist.pendente.as_mut() {
                pendente.push(Passo::Dimensao {
                    coluna, indice: *indice, antes, depois,
                });
            }
            self.aplicar_dimensao(coluna, *indice, depois);
        }
        self.fechar_transacao();
        Ok(indices.len() + reexibidas_do_fim)
    }

    // ------------------------------------------------------------ desfazer

    /// Abre uma transação (ou entra numa já aberta). Tudo que for gravado até
    /// o [`Self::fechar_transacao`] correspondente desfaz de uma vez.
    pub fn abrir_transacao(&mut self) {
        if self.hist.pendente.is_none() {
            self.hist.pendente = Some(Vec::new());
            self.hist.profundidade = 0;
        }
        self.hist.profundidade += 1;
    }

    /// Compacta somente o lote recém-gravado dentro da transação aberta.
    /// Chamado após cada bloco de Texto para Colunas: não é preciso manter
    /// milhões de `Change`/`Snap` na memória até chegar ao commit.
    pub fn compactar_lote_divisao(&mut self) {
        let Some(pendente) = self.hist.pendente.as_mut() else { return };
        let inicio = pendente.iter().rposition(|p| !matches!(p, Passo::Celula(_)))
            .map(|i| i + 1).unwrap_or(0);
        if inicio == pendente.len() { return; }
        let recentes = pendente.split_off(inicio);
        let mut bloco = BlocoCompacto::novo();
        for passo in recentes {
            if let Passo::Celula(c) = passo { bloco.adicionar(c); }
        }
        if bloco.alteracoes.is_empty() { return; }
        let bytes = bloco.textos.len()
            + bloco.alteracoes.len() * std::mem::size_of::<EdicaoCompacta>();
        self.hist.memoria_divisao = self.hist.memoria_divisao.saturating_add(bytes);
        if self.hist.arquivo_divisao.is_none()
            && self.hist.memoria_divisao > LIMITE_HISTORIA_DIVISAO_RAM
        {
            if let Ok(arquivo) = ArquivoHistorico::criar() {
                // Evitar manter em RAM os primeiros 24 MiB também.
                for passo in pendente.iter_mut() {
                    if let Passo::Compacto(antigo) = passo {
                        if let Ok(disco) = arquivo.gravar(antigo) {
                            *passo = Passo::CompactoNoDisco(disco);
                        }
                    }
                }
                self.hist.arquivo_divisao = Some(arquivo);
            }
        }
        if let Some(arquivo) = &self.hist.arquivo_divisao {
            if let Ok(disco) = arquivo.gravar(&bloco) {
                pendente.push(Passo::CompactoNoDisco(disco));
                return;
            }
        }
        // Falha de I/O: preservar undo/redo na RAM, nunca descartar dados.
        pendente.push(Passo::Compacto(bloco));
    }

    /// Fecha a transação; a mais de fora empilha o desfazer.
    pub fn fechar_transacao(&mut self) {
        self.tocar();
        if self.hist.pendente.is_none() {
            return;
        }
        self.hist.profundidade = self.hist.profundidade.saturating_sub(1);
        if self.hist.profundidade > 0 {
            return;
        }
        let feitas = self.hist.pendente.take().unwrap_or_default();
        self.hist.arquivo_divisao = None;
        self.hist.memoria_divisao = 0;
        if !feitas.is_empty() {
            self.hist.desfazer.push(feitas);
            if self.hist.desfazer.len() > LIMITE_DO_DESFAZER {
                self.hist.desfazer.remove(0);
                self.hist.descartadas += 1;
            }
            self.hist.refazer.clear();
        }
    }

    /// Tudo ou nada: volta o que a transação aberta já escreveu e a fecha
    /// sem empilhar. É o que o `transact` do TypeScript faz quando esbarra
    /// numa proteção no meio.
    pub fn reverter_transacao(&mut self) {
        let feitas = self.hist.pendente.take().unwrap_or_default();
        self.hist.arquivo_divisao = None;
        self.hist.memoria_divisao = 0;
        self.hist.profundidade = 0;
        for passo in feitas.into_iter().rev() {
            self.desfazer_passo(&passo);
        }
    }

    /// Atualiza o metadado de mesclagem como uma única etapa no histórico.
    /// O motor não interpreta JSON: mantém a mesma representação do arquivo.
    pub fn alterar_mesclas(&mut self, novo: Option<String>, area: &crate::celulas::refs::Rect) -> bool {
        if self.mc == novo { return false; }
        self.abrir_transacao();
        let antes = std::mem::replace(&mut self.mc, novo.clone());
        if let Some(pendente) = self.hist.pendente.as_mut() {
            pendente.push(Passo::Mesclas {
                area: [area.r0, area.c0, area.r1, area.c1],
                antes, depois: novo,
            });
        }
        self.fechar_transacao();
        true
    }

    /// Altera os metadados de tabelas em UMA etapa de Ctrl+Z/Ctrl+Y.
    /// A transação pode ser agrupada com outras alterações já abertas.
    /// Não faz snapshot de células: retém apenas o JSON `tb`.
    pub fn alterar_tabelas(&mut self, novo: Option<String>, area: &crate::celulas::refs::Rect) -> bool {
        if self.tb == novo { return false; }
        self.abrir_transacao();
        let antes = std::mem::replace(&mut self.tb, novo.clone());
        if let Some(pendente) = self.hist.pendente.as_mut() {
            pendente.push(Passo::Tabelas {
                area: [area.r0, area.c0, area.r1, area.c1],
                antes, depois: novo,
            });
        }
        self.fechar_transacao();
        true
    }

    /// Grava metadados de dinâmica como parte do mesmo histórico das células.
    /// Pode ser chamado com outra transação já aberta, sem criar uma etapa extra.
    pub fn alterar_dinamicas(&mut self, novo: Option<String>, area: &crate::celulas::refs::Rect) -> bool {
        if self.dn == novo { return false; }
        self.abrir_transacao();
        let antes = std::mem::replace(&mut self.dn, novo.clone());
        if let Some(pendente) = self.hist.pendente.as_mut() {
            pendente.push(Passo::Dinamicas {
                area: [area.r0, area.c0, area.r1, area.c1],
                antes, depois: novo,
            });
        }
        self.fechar_transacao();
        true
    }

    /// Altera o JSON de imagens (`im`) em UMA etapa de Ctrl+Z/Ctrl+Y.
    pub fn alterar_imagens(&mut self, novo: Option<String>) -> bool {
        if self.im == novo { return false; }
        self.abrir_transacao();
        let antes = std::mem::replace(&mut self.im, novo.clone());
        if let Some(pendente) = self.hist.pendente.as_mut() {
            pendente.push(Passo::Imagens { antes, depois: novo });
        }
        self.fechar_transacao();
        true
    }

    /// Salva a lista de botões de macro (bt), com undo/redo atômico e sem tocar as células.
    pub fn alterar_botoes(&mut self, novo: Option<String>) -> bool {
        if self.bt == novo { return false; }
        self.abrir_transacao();
        let antes = std::mem::replace(&mut self.bt, novo.clone());
        if let Some(pendente) = self.hist.pendente.as_mut() {
            pendente.push(Passo::Botoes { antes, depois: novo });
        }
        self.fechar_transacao();
        true
    }

    /// Salva a tinta da aba (tn, traços da aba Desenhar) em UMA etapa de
    /// Ctrl+Z/Ctrl+Y, sem tocar nas células. O motor só guarda o texto.
    pub fn alterar_tinta(&mut self, novo: Option<String>) -> bool {
        if self.tn == novo { return false; }
        self.abrir_transacao();
        let antes = std::mem::replace(&mut self.tn, novo.clone());
        if let Some(pendente) = self.hist.pendente.as_mut() {
            pendente.push(Passo::Tinta { antes, depois: novo });
        }
        self.fechar_transacao();
        true
    }

    /// Salva os objetos soltos da aba (ob: formas, ícones, SmartArt) em UMA
    /// etapa de Ctrl+Z/Ctrl+Y, sem tocar nas células.
    pub fn alterar_objetos(&mut self, novo: Option<String>) -> bool {
        if self.ob == novo { return false; }
        self.abrir_transacao();
        let antes = std::mem::replace(&mut self.ob, novo.clone());
        if let Some(pendente) = self.hist.pendente.as_mut() {
            pendente.push(Passo::Objetos { antes, depois: novo });
        }
        self.fechar_transacao();
        true
    }

    /// Salva os gráficos da aba (g) em UMA etapa de Ctrl+Z/Ctrl+Y. Com
    /// `juntar`, se a última etapa da pilha for só uma troca de gráficos (e
    /// nada foi refeito depois), ela é estendida em vez de criar outra: é
    /// como o painel junta as letras digitadas num mesmo campo.
    pub fn alterar_graficos(&mut self, novo: Option<String>, juntar: bool) -> bool {
        if self.g == novo { return false; }
        if juntar && self.hist.pendente.is_none() && self.hist.refazer.is_empty() {
            if let Some([Passo::Graficos { depois, .. }]) = self.hist.desfazer.last_mut().map(Vec::as_mut_slice) {
                *depois = novo.clone();
                self.g = novo;
                self.tocar();
                return true;
            }
        }
        self.abrir_transacao();
        let antes = std::mem::replace(&mut self.g, novo.clone());
        if let Some(pendente) = self.hist.pendente.as_mut() {
            pendente.push(Passo::Graficos { antes, depois: novo });
        }
        self.fechar_transacao();
        true
    }

    /// Atualiza regras de validação com undo/redo e transações aninhadas.
    /// Mantém JSON opaco no núcleo, que não deve recriar regras da UI.
    pub fn alterar_validacoes(&mut self, novo: Option<String>, area: &crate::celulas::refs::Rect) -> bool {
        if self.va == novo { return false; }
        self.abrir_transacao();
        let antes = std::mem::replace(&mut self.va, novo.clone());
        if let Some(pendente) = self.hist.pendente.as_mut() {
            pendente.push(Passo::Validacoes {
                area: [area.r0,area.c0,area.r1,area.c1],
                antes, depois: novo,
            });
        }
        self.fechar_transacao();
        true
    }

    /// Grava regras condicionais como metadados com undo/redo sem criar células.
    pub fn alterar_condicionais(&mut self, novo: Option<String>, area: &crate::celulas::refs::Rect) -> bool {
        if self.r == novo {return false;}
        self.abrir_transacao();
        let antes=std::mem::replace(&mut self.r,novo.clone());
        if let Some(pendente)=self.hist.pendente.as_mut() {
            pendente.push(Passo::Condicionais{
                area:[area.r0,area.c0,area.r1,area.c1],antes,depois:novo,
            });
        }
        self.fechar_transacao();
        true
    }

    /// Evita clonar o JSON de tabelas ao desfazer/refazer apenas celulas.
    pub fn proximo_historico_altera_tabelas(&self, refazer: bool) -> bool {
        let historico = if refazer { &self.hist.refazer } else { &self.hist.desfazer };
        historico.last().is_some_and(|transacao|
            transacao.iter().any(|p| matches!(p, Passo::Tabelas { .. })))
    }

    pub fn can_undo(&self) -> bool {
        !self.hist.desfazer.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.hist.refazer.is_empty()
    }

    /// As áreas `[r0, c0, r1, c1]` que o próximo desfazer (ou refazer) vai
    /// mexer — quem confere proteção antes de desfazer precisa delas. Uma
    /// célula é a área dela; uma permutação é o retângulo inteiro.
    pub fn celulas_do_proximo(&self, refazer: bool) -> Vec<[u32; 4]> {
        let pilha = if refazer { &self.hist.refazer } else { &self.hist.desfazer };
        pilha
            .last()
            .map(|t| {
                t.iter()
                    .map(|p| match p {
                        Passo::Celula(c) => [key_row(c.k), key_col(c.k), key_row(c.k), key_col(c.k)],
                        Passo::Compacto(bloco) => bloco.area,
                        Passo::CompactoNoDisco(bloco) => bloco.area,
                        Passo::Permutacao { first, c0, c1, ordem, .. } => [*first, *c0, first + ordem.len() as u32 - 1, *c1],
                        Passo::Dimensao { coluna: true, indice, .. } => [0, *indice, 0, *indice],
                        Passo::Dimensao { coluna: false, indice, .. } => [*indice, 0, *indice, 0],
                        Passo::Mesclas { area, .. } => *area,
                        Passo::Tabelas { area, .. } => *area,
                        Passo::Dinamicas { area, .. } => *area,
                        Passo::Validacoes { area, .. } => *area,
                        Passo::Condicionais { area, .. } => *area,
                        Passo::Comentario { linha, coluna, .. } => [*linha, *coluna, *linha, *coluna],
                        Passo::Divisao { area, .. } => *area,
                        Passo::Imagens { .. } | Passo::Botoes { .. } | Passo::Tinta { .. } | Passo::Objetos { .. } | Passo::Graficos { .. }
                        | Passo::LarguraPadrao { .. }
                        | Passo::Topicos { .. } => [0, 0, 0, 0],
                        Passo::OcultasDesde { .. } => [0, 0, 0, 0],
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn desfazer_passo(&mut self, passo: &Passo) {
        match passo {
            Passo::Celula(ch) => {
                self.apply(key_row(ch.k), key_col(ch.k), ch.antes.clone(), false);
            }
            Passo::Compacto(bloco) => {
                for ch in bloco.alteracoes.iter().rev() {
                    self.apply(key_row(ch.k), key_col(ch.k),
                        bloco.reconstruir(ch.antes), false);
                }
            }
            Passo::CompactoNoDisco(bloco) => {
                // Nunca pular um bloco silenciosamente se o arquivo falhar:
                // isso produziria uma restauração incompleta e incorreta.
                let dados = bloco.carregar().expect("histórico de undo indisponível no disco");
                for ch in dados.alteracoes.iter().rev() {
                    self.apply(key_row(ch.k), key_col(ch.k),
                        dados.reconstruir(ch.antes), false);
                }
            }
            Passo::Permutacao { first, c0, c1, ordem, quebradas } => {
                // A inversa: quem foi de `ordem[i]` para `first + i` volta.
                let mut volta = vec![0u32; ordem.len()];
                for (i, &origem) in ordem.iter().enumerate() {
                    volta[(origem - first) as usize] = first + i as u32;
                }
                self.permutar_sem_registro(*first, *c0, *c1, &volta);
                for (row, col, raw, estilo) in quebradas {
                    self.set_cell_raw(*row, *col, raw.clone(), *estilo);
                }
            }
            Passo::Dimensao { coluna, indice, antes, .. } =>
                self.aplicar_dimensao(*coluna, *indice, *antes),
            Passo::Mesclas { antes, .. } => self.mc = antes.clone(),
            Passo::Tabelas { antes, .. } => self.tb = antes.clone(),
            Passo::Dinamicas { antes, .. } => self.dn = antes.clone(),
            Passo::Validacoes { antes, .. } => self.va = antes.clone(),
            Passo::Condicionais { antes, .. } => self.r = antes.clone(),
            Passo::Imagens { antes, .. } => self.im = antes.clone(),
            Passo::LarguraPadrao { antes, .. } => self.largura_padrao = *antes,
            Passo::Topicos { antes, .. } => self.topicos = (**antes).clone(),
            Passo::OcultasDesde { coluna, antes, .. } => self.aplicar_ocultas_desde(*coluna, *antes),
            Passo::Botoes { antes, .. } => self.bt = antes.clone(),
            Passo::Tinta { antes, .. } => self.tn = antes.clone(),
            Passo::Objetos { antes, .. } => self.ob = antes.clone(),
            Passo::Graficos { antes, .. } => self.g = antes.clone(),
            Passo::Comentario { linha, coluna, antes, .. } =>
                self.aplicar_comentario(*linha, *coluna, antes.as_deref().cloned()),
            Passo::Divisao { area, antes, .. } => {
                let t = std::time::Instant::now();
                self.limpar_area_sem_registro(*area);
                for p in antes {
                    self.desfazer_passo(p);
                }
                if self.alguma_formula_le() { self.invalidar_todos_calculos(); }
                crate::anotar!("[desfazer] texto para colunas · {:.2} s", t.elapsed().as_secs_f64());
            }
        }
    }

    fn refazer_passo(&mut self, passo: &Passo) {
        match passo {
            Passo::Celula(ch) => {
                self.apply(key_row(ch.k), key_col(ch.k), ch.depois.clone(), false);
            }
            Passo::Compacto(bloco) => {
                for ch in &bloco.alteracoes {
                    self.apply(key_row(ch.k), key_col(ch.k),
                        bloco.reconstruir(ch.depois), false);
                }
            }
            Passo::CompactoNoDisco(bloco) => {
                let dados = bloco.carregar().expect("histórico de redo indisponível no disco");
                for ch in &dados.alteracoes {
                    self.apply(key_row(ch.k), key_col(ch.k),
                        dados.reconstruir(ch.depois), false);
                }
            }
            Passo::Permutacao { first, c0, c1, ordem, .. } => { self.permutar_sem_registro(*first, *c0, *c1, ordem); }
            Passo::Dimensao { coluna, indice, depois, .. } =>
                self.aplicar_dimensao(*coluna, *indice, *depois),
            Passo::Mesclas { depois, .. } => self.mc = depois.clone(),
            Passo::Tabelas { depois, .. } => self.tb = depois.clone(),
            Passo::Dinamicas { depois, .. } => self.dn = depois.clone(),
            Passo::Validacoes { depois, .. } => self.va = depois.clone(),
            Passo::Condicionais { depois, .. } => self.r = depois.clone(),
            Passo::Imagens { depois, .. } => self.im = depois.clone(),
            Passo::LarguraPadrao { depois, .. } => self.largura_padrao = *depois,
            Passo::Topicos { depois, .. } => self.topicos = (**depois).clone(),
            Passo::OcultasDesde { coluna, depois, .. } => self.aplicar_ocultas_desde(*coluna, *depois),
            Passo::Botoes { depois, .. } => self.bt = depois.clone(),
            Passo::Tinta { depois, .. } => self.tn = depois.clone(),
            Passo::Objetos { depois, .. } => self.ob = depois.clone(),
            Passo::Graficos { depois, .. } => self.g = depois.clone(),
            Passo::Comentario { linha, coluna, depois, .. } =>
                self.aplicar_comentario(*linha, *coluna, depois.as_deref().cloned()),
            Passo::Divisao { refazer, .. } => {
                let t = std::time::Instant::now();
                (refazer.0)(self);
                crate::anotar!("[refazer] texto para colunas · {:.2} s", t.elapsed().as_secs_f64());
            }
        }
    }

    // --------------------------------------------- texto para colunas em lote

    /// Começa uma divisão na `area` `[r0, c0, r1, c1]`. Com `refazer`, exige
    /// transação aberta e empilha nela o [`Passo::Divisao`] com a foto do que
    /// existe na área agora; sem (`None`, o próprio refazer), só prepara.
    pub fn iniciar_divisao(
        &mut self, area: [u32; 4], tipos: Vec<crate::operacoes::ops::TipoColunaDivisao>,
        refazer: Option<Refazer>,
    ) -> GravadorDivisao {
        let [r0, c0, r1, c1] = area;
        let dentro = |k: &CellKey| {
            let (r, c) = (key_row(*k), key_col(*k));
            r >= r0 && r <= r1 && c >= c0 && c <= c1
        };
        if let Some(refazer) = refazer {
            let rect = Rect::new(r0, c0, r1, c1);
            let mut antes = Vec::new();
            let mut bloco = BlocoCompacto::novo();
            let mut arquivo: Option<Arc<ArquivoHistorico>> = None;
            let mut bytes = 0usize;
            for k in self.chaves_no_retangulo(&rect) {
                let foto = self.snapshot(key_row(k), key_col(k));
                bloco.adicionar(Change { k, antes: foto, depois: None });
                if bloco.alteracoes.len() == 65_536 {
                    bytes += bloco.textos.len() + bloco.alteracoes.len() * 32;
                    let cheio = std::mem::replace(&mut bloco, BlocoCompacto::novo());
                    antes.push(Self::guardar_bloco(cheio, bytes, &mut arquivo));
                }
            }
            if !bloco.alteracoes.is_empty() {
                bytes += bloco.textos.len() + bloco.alteracoes.len() * 32;
                antes.push(Self::guardar_bloco(bloco, bytes, &mut arquivo));
            }
            if let Some(pendente) = self.hist.pendente.as_mut() {
                pendente.push(Passo::Divisao { area, antes, refazer });
            }
        }
        // A grade redesenha tudo: anotar milhões de chaves não serve a ninguém.
        self.rastro.borrow_mut().desconhecer();
        let especiais = self.cells.keys().chain(self.externas.iter())
            .filter(|k| dentro(k)).copied()
            .chain(self.styles.chaves_no_retangulo(r0, c0, r1, c1)).collect();
        let mut coluna_limpa: Vec<bool> = (c0..=c1)
            .map(|c| !self.data.pode_ter_na_faixa(c, r0, r1)).collect();
        for (k, _) in self.textos.iter() {
            if dentro(k) { coluna_limpa[(key_col(*k) - c0) as usize] = false; }
        }
        self.formulas.for_each_in_range(r0, r1, c0, c1, |_, c| coluna_limpa[(c - c0) as usize] = false);
        let estilo_herdado = self.sheet_style.is_some()
            || self.col_styles.keys().any(|c| *c >= c0 && *c <= c1)
            || self.row_styles.keys().any(|r| *r >= r0 && *r <= r1);
        GravadorDivisao {
            especiais, estilo_herdado,
            formulas_leem: self.alguma_formula_le(),
            coluna_limpa, c0,
            estilo_do_formato: Default::default(),
            tipos,
        }
    }

    /// Bloco de foto na RAM até 24 MiB somados; depois disso, no disco.
    fn guardar_bloco(bloco: BlocoCompacto, bytes: usize,
        arquivo: &mut Option<Arc<ArquivoHistorico>>) -> Passo {
        if bytes > LIMITE_HISTORIA_DIVISAO_RAM && arquivo.is_none() {
            *arquivo = ArquivoHistorico::criar().ok();
        }
        if let Some(a) = arquivo {
            if let Ok(disco) = a.gravar(&bloco) {
                return Passo::CompactoNoDisco(disco);
            }
        }
        Passo::Compacto(bloco)
    }

    /// Grava linhas já divididas e analisadas a partir de `dst_col`, sem
    /// fotografar nada (o [`Passo::Divisao`] cuida do desfazer).
    pub fn gravar_divisao(&mut self, g: &mut GravadorDivisao, dst_col: u32,
        linhas: &[(u32, Vec<CampoDivisao>)]) {
        use crate::operacoes::ops::TipoColunaDivisao as Tipo;
        // O caminho completo empilharia fotos: aqui elas vão para um rascunho
        // descartado no fim.
        let salvo = (self.hist.pendente.replace(Vec::new()), self.hist.profundidade);
        self.hist.profundidade = 1;
        let largura = linhas.iter().map(|(_, c)| c.len()).max().unwrap_or(0);
        let sem_especiais = !g.estilo_herdado && g.especiais.is_empty();
        let mut max_col = None;
        // Coluna por coluna: a mesma coluna da DataTable fica quente na cache.
        for i in 0..largura {
            let col = dst_col + i as u32;
            if col >= MAX_COLS { break; }
            let tipo = g.tipos.get(i).cloned().unwrap_or(Tipo::Geral);
            if matches!(tipo, Tipo::Pular) { continue; }
            let limpa = col.checked_sub(g.c0)
                .and_then(|j| g.coluna_limpa.get(j as usize).copied()).unwrap_or(false);
            for (row, campos) in linhas {
                let Some(campo) = campos.get(i) else { continue };
                let row = *row;
                let k = key(row, col);
                let especial = !sem_especiais && (g.estilo_herdado || g.especiais.contains(&k));
                if !matches!(tipo, Tipo::Geral) {
                    crate::operacoes::ops::gravar_campo_tipado(self, row, col, &campo.raw, &tipo);
                } else if let (Some(r), false) = (&campo.rapido, especial) {
                    if !limpa && (self.data.has_cell(row, col) || self.textos.contains_key(&k)
                        || self.formulas.has(row, col)) {
                        self.soltar_para_literal(row, col);
                    }
                    self.data.set_cell_value(row, col, &r.valor, &mut self.pool);
                    if let Some(f) = r.formato {
                        let idx = *g.estilo_do_formato.entry(f).or_insert_with(|| {
                            let mut patch = EstiloParcial::new();
                            patch.insert("format".into(), Campo::Texto(f.into()));
                            self.registrar_parcial(patch)
                        });
                        if let Some(i) = idx { self.styles.insert(k, i); }
                    }
                    if r.preserva { self.textos.insert(k, &campo.raw); }
                    if g.formulas_leem { self.mark_dirty_cascade(k); }
                } else if campo.raw.is_empty() && !especial
                    && (limpa || (!self.data.has_cell(row, col) && !self.formulas.has(row, col)
                        && !self.textos.contains_key(&k))) {
                    // Campo vazio sobre célula vazia: nada a fazer.
                    continue;
                } else {
                    self.set_cell_divisao(row, col, &campo.raw);
                }
                max_col = Some(col);
            }
        }
        if let (Some(c), Some(primeira), Some(ultima)) =
            (max_col, linhas.first(), linhas.last()) {
            self.cresce_limites(primeira.0, dst_col);
            self.cresce_limites(ultima.0, c);
        }
        self.hist.pendente = salvo.0;
        self.hist.profundidade = salvo.1;
    }

    /// Esvazia a área sem histórico (o desfazer do [`Passo::Divisao`]).
    /// `r0..=r1` da coluna `col` sem nada: nem dado, nem fórmula, nem
    /// célula especial. É a condição do [`Self::preencher_faixa_vazia`].
    pub fn faixa_vazia(&self, col: u32, r0: u32, r1: u32) -> bool {
        let dentro = |k: &CellKey| key_col(*k) == col && (r0..=r1).contains(&key_row(*k));
        if self.data.pode_ter_na_faixa(col, r0, r1)
            || self.cells.keys().chain(self.externas.iter()).chain(self.textos.iter().map(|(k, _)| k)).any(dentro) {
            return false;
        }
        let mut formula = false;
        self.formulas.for_each_in_range(r0, r1, col, col, |_, _| formula = true);
        !formula
    }

    /// O valor que [`Self::preencher_faixa_vazia`] grava para `raw` com o
    /// estilo `estilo`, ou `None` quando ele não vai por lá (fórmula, erro,
    /// vazio ou texto que não volta igual com esse estilo: "16/10/2026" sem
    /// formato de data precisaria guardar o texto em cada célula).
    pub fn valor_de_faixa(raw: &str, estilo: Option<&EstiloParcial>) -> Option<Value> {
        if raw.is_empty() || raw.starts_with('=') { return None; }
        let analisado = crate::celulas::input::parse_literal(raw);
        let simples = !matches!(analisado.value, Value::Error(_) | Value::Empty)
            && round_trips(raw, &analisado.value, estilo);
        simples.then_some(analisado.value)
    }

    /// Cola o valor `raw` (não fórmula), com o estilo `estilo`, em
    /// `r0..=r1` da coluna `col`, que tem de estar vazia
    /// ([`Self::faixa_vazia`]), bloco a bloco: colar A1 em A2:A100000000
    /// eram 100 milhões de `set_cell` (17/10/2026). Um passo de Ctrl+Z (o do
    /// Texto para Colunas: desfazer limpa a faixa). `false` quando o valor
    /// não vai por aqui ([`Self::valor_de_faixa`], [`Self::pintar_faixa_vazia`]):
    /// use o caminho de sempre.
    pub fn preencher_faixa_vazia(&mut self, col: u32, r0: u32, r1: u32, raw: &str,
        estilo: Option<&EstiloParcial>) -> bool {
        if r0 > r1 { return false; }
        let Some(valor) = Self::valor_de_faixa(raw, estilo) else { return false };
        self.abrir_transacao();
        if estilo.is_some_and(|e| !self.pintar_faixa_vazia(col, r0, r1, e)) {
            self.fechar_transacao();
            return false;
        }
        let refazer = Refazer(Arc::new(move |s: &mut Sheet| {
            if s.data.preencher_faixa(col, r0, r1, &valor, &mut s.pool) {
                s.cresce_limites(r0, col);
                s.cresce_limites(r1, col);
            }
            s.rastro.borrow_mut().desconhecer();
            if s.alguma_formula_le() { s.invalidar_todos_calculos(); }
        }));
        if let Some(pendente) = self.hist.pendente.as_mut() {
            pendente.push(Passo::Divisao { area: [r0, col, r1, col], antes: Vec::new(), refazer: refazer.clone() });
        }
        (refazer.0)(self);
        self.fechar_transacao();
        true
    }

    /// Dá o estilo `estilo` a `r0..=r1` da coluna `col` como uma faixa só
    /// (data ou moeda colada até o fim: um estilo por célula seriam milhões
    /// de entradas). Um passo de Ctrl+Z, que limpa a faixa. `false` se ali
    /// já há estilo de célula ou de linha: o desfazer apagaria o de antes,
    /// e o estilo de linha muda a mescla de linha a linha.
    pub fn pintar_faixa_vazia(&mut self, col: u32, r0: u32, r1: u32, estilo: &EstiloParcial) -> bool {
        if !tem_estilo(Some(estilo)) { return true; }
        if r0 > r1 || !self.row_styles.is_empty()
            || self.styles.alguma_no_retangulo(r0, col, r1, col, |_, _, _| true) {
            return false;
        }
        let Some(idx) = self.registrar_parcial(estilo.clone()) else { return true };
        let refazer = Refazer(Arc::new(move |s: &mut Sheet| {
            s.styles.inserir_faixa(col, r0, r1, idx);
            s.rastro.borrow_mut().desconhecer();
        }));
        self.abrir_transacao();
        if let Some(pendente) = self.hist.pendente.as_mut() {
            pendente.push(Passo::Divisao { area: [r0, col, r1, col], antes: Vec::new(), refazer: refazer.clone() });
        }
        (refazer.0)(self);
        self.fechar_transacao();
        true
    }

    /// Se a fórmula `raw`, escrita em (`row`, `col`), pode ir em bloco pela
    /// coluna empacotada ([`Self::preencher_formula_faixa_vazia`]).
    pub fn formula_cabe_em_bloco(&self, col: u32, row: u32, raw: &str) -> bool {
        use crate::formula::formula_pattern::pattern_for;
        let raw = raw.trim();
        raw.starts_with('=') && pattern_for(raw, row as i32, col as i32).is_some_and(|padrao|
            crate::formula::formula_table::FormulaTable::aceitaria(&padrao, raw, super::pattern_check)
                && self.formulas.caberia_na_coluna(&padrao, col))
    }

    /// A fórmula `raw` — o texto dela na linha `r0` — levada para `r0..=r1`
    /// da coluna `col`, que tem de estar vazia ([`Self::faixa_vazia`]), como
    /// o Ctrl+D a levaria: referências relativas andam. Um padrão para a
    /// faixa toda, em blocos na coluna empacotada; um passo de Ctrl+Z.
    /// `MÍNIMOSES(AO:AO;A:A;A2;…)` em 13,7 milhões de linhas travou o app
    /// pelo caminho de uma fórmula por célula (17/10/2026). `false` quando a
    /// fórmula não cabe na coluna empacotada (volátil, matriz, nome, lê a
    /// própria coluna…): use o caminho de sempre.
    pub fn preencher_formula_faixa_vazia(&mut self, col: u32, r0: u32, r1: u32, raw: &str) -> bool {
        let raw = raw.trim();
        if r0 > r1 || !self.formula_cabe_em_bloco(col, r0, raw) { return false; }
        let raw = raw.to_string();
        let refazer = Refazer(Arc::new(move |s: &mut Sheet| {
            if !s.formulas.preencher_padrao(col, r0, r1, &raw, super::pattern_check) { return; }
            s.cresce_limites(r0, col);
            s.cresce_limites(r1, col);
            s.rastro.borrow_mut().desconhecer();
            // Quem lê estas linhas (inclusive por intervalo) recalcula, por
            // coluna: célula a célula seriam milhões de entradas.
            let usos = s.formulas.columns_in_use();
            s.invalidar_colunas_e_dependentes(std::iter::once(col).collect(), &usos);
        }));
        self.abrir_transacao();
        if let Some(pendente) = self.hist.pendente.as_mut() {
            pendente.push(Passo::Divisao { area: [r0, col, r1, col], antes: Vec::new(), refazer: refazer.clone() });
        }
        (refazer.0)(self);
        self.fechar_transacao();
        true
    }

    fn limpar_area_sem_registro(&mut self, area: [u32; 4]) {
        let [r0, c0, r1, c1] = area;
        let dentro = |k: &CellKey| {
            let (r, c) = (key_row(*k), key_col(*k));
            r >= r0 && r <= r1 && c >= c0 && c <= c1
        };
        let avulsas: Vec<CellKey> = self.cells.keys()
            .chain(self.externas.iter()).chain(self.textos.iter().map(|(k, _)| k))
            .filter(|k| dentro(k)).copied().collect();
        for k in avulsas {
            self.soltar_para_literal(key_row(k), key_col(k));
        }
        // Estilo por faixa, sem abrir célula a célula (`pintar_faixa_vazia`).
        for c in c0..=c1 {
            self.styles.remover_faixa(c, r0, r1);
        }
        // Fórmula empacotada: em blocos, como entrou (`preencher_formula_faixa_vazia`).
        for c in c0..=c1 {
            self.formulas.limpar_faixa(c, r0, r1);
        }
        {
            let calc = self.calc.get_mut();
            calc.cache.retain(|k, _| !dentro(k));
            calc.dirty.retain(|k| !dentro(k));
        }
        for c in c0..=c1 {
            self.data.limpar_faixa(c, r0, r1);
        }
        // A última linha usada pode ter subido: desfazer o colar de 100
        // milhões de linhas deixava `max_row` lá, e o próximo colar "até a
        // última linha com dados" ia até o fim (17/10/2026).
        self.bounds_dirty = true;
        self.acertar_limites();
        self.rastro.borrow_mut().desconhecer();
    }

    /// As células que a última transação mexeu (a que o Ctrl+Z desfaria),
    /// num retângulo: o `Target` do `Worksheet_Change`. `None` se ela não
    /// mexeu em célula (largura, mescla, tabela…).
    pub fn area_da_ultima_transacao(&self) -> Option<Rect> {
        let mut area: Option<[u32; 4]> = None;
        let mut juntar = |a: [u32; 4]| {
            area = Some(match area {
                None => a,
                Some(b) => [b[0].min(a[0]), b[1].min(a[1]), b[2].max(a[2]), b[3].max(a[3])],
            });
        };
        for passo in self.hist.desfazer.last()? {
            match passo {
                Passo::Celula(c) => {
                    let (r, k) = (crate::celulas::refs::key_row(c.k), crate::celulas::refs::key_col(c.k));
                    juntar([r, k, r, k]);
                }
                Passo::Compacto(b) => if b.area[0] <= b.area[2] { juntar(b.area) },
                Passo::CompactoNoDisco(b) => if b.area[0] <= b.area[2] { juntar(b.area) },
                _ => {}
            }
        }
        area.map(|a| Rect::new(a[0], a[1], a[2], a[3]))
    }

    pub fn undo(&mut self) -> bool {
        self.tocar();
        let Some(t) = self.hist.desfazer.pop() else {
            return false;
        };
        // Só células (nada muda de lugar no meio): uma cascata de "sujo" no
        // fim, como na escrita em lote ([`Sheet::adiar_cascata`]).
        let em_lote = Self::so_celulas(&t);
        if em_lote { self.adiar_cascata(); }
        for passo in t.iter().rev() {
            self.desfazer_passo(passo);
        }
        if em_lote { self.soltar_cascata(); }
        self.hist.refazer.push(t);
        true
    }

    fn so_celulas(t: &[Passo]) -> bool {
        t.len() > 1 && t.iter().all(|p| matches!(p, Passo::Celula(_) | Passo::Compacto(_) | Passo::CompactoNoDisco(_)))
    }

    pub fn redo(&mut self) -> bool {
        self.tocar();
        let Some(t) = self.hist.refazer.pop() else {
            return false;
        };
        let em_lote = Self::so_celulas(&t);
        if em_lote { self.adiar_cascata(); }
        for passo in &t {
            self.refazer_passo(passo);
        }
        if em_lote { self.soltar_cascata(); }
        self.hist.desfazer.push(t);
        true
    }

    /// Permuta as linhas `first..first + ordem.len()` × `c0..=c1`: a linha
    /// `ordem[i]` vai para `first + i`, com o estilo direto de cada célula, e
    /// a fórmula anda junto (como o `shiftFormula` do ordenar). Um passo só
    /// no desfazer.
    ///
    /// Direto no armazenamento, coluna a coluna — 23/09/2026: ordenar os 2
    /// milhões de linhas do usuário regravando célula a célula levava 69 s e
    /// 5,5 GB (cada célula guardava a foto de antes e de depois).
    ///
    /// Diferença assumida para o `sortRange` do TypeScript, que apagava a área
    /// e regravava cada célula com o estilo que ela MOSTRAVA: lá, o estilo de
    /// coluna e de linha virava estilo direto de cada célula movida. Aqui só
    /// o estilo direto anda; o de coluna continua sendo da coluna (dá o mesmo
    /// na tela) e o de linha inteira fica na linha.
    pub fn permutar_linhas(&mut self, first: u32, c0: u32, c1: u32, ordem: &[u32]) {
        if ordem.is_empty() {
            return;
        }
        self.abrir_transacao();
        let quebradas = self.permutar_sem_registro(first, c0, c1, ordem);
        if let Some(p) = self.hist.pendente.as_mut() {
            p.push(Passo::Permutacao { first, c0, c1, ordem: ordem.to_vec(), quebradas });
        }
        self.fechar_transacao();
    }

    /// Devolve as fórmulas que o deslocamento levou a `#REF!` (ver [`Passo::Permutacao`]).
    fn permutar_sem_registro(&mut self, first: u32, c0: u32, c1: u32, ordem: &[u32]) -> Vec<(u32, u32, String, Option<usize>)> {
        self.rastro.borrow_mut().desconhecer();
        let last = first + ordem.len() as u32 - 1;
        let mut destino_de = vec![0u32; ordem.len()];
        for (i, &origem) in ordem.iter().enumerate() {
            destino_de[(origem - first) as usize] = first + i as u32;
        }
        let dentro = |k: CellKey| {
            let (r, c) = (key_row(k), key_col(k));
            r >= first && r <= last && c >= c0 && c <= c1
        };
        let novo = |k: CellKey| key(destino_de[(key_row(k) - first) as usize], key_col(k));

        // 1) As células do `Map` (fórmula com intervalo, com estilo, erro
        //    digitado): poucas, e cada uma tem o seu lugar no grafo. Saem
        //    agora e voltam no fim, já no lugar novo.
        let do_map: Vec<CellKey> = self.cells.keys().copied().filter(|&k| dentro(k)).collect();
        let mut voltam = Vec::with_capacity(do_map.len());
        for k in do_map {
            let cell = self.cells.remove(&k).expect("listada acima");
            self.deps.unlink(k, &mut |_, _| {});
            self.deps.volatiles.remove(&k);
            self.externas.remove(&k);
            voltam.push((novo(k), key_row(k), cell));
        }

        // 2) As colunas: o literal, a fórmula por padrão, e o que é por chave
        //    (o estilo direto e o texto digitado que não volta do valor).
        let mut regravar = Vec::new();
        for col in c0..=c1 {
            self.data.permutar_linhas(col, first, ordem);
            for (destino, origem, raw) in self.formulas.permutar_linhas(col, first, ordem, c0, c1) {
                regravar.push((destino, origem, col, raw));
            }
        }
        let estilos: Vec<(CellKey, usize)> = self.styles.chaves_no_retangulo(first, c0, last, c1).into_iter()
            .filter_map(|k| self.styles.get(k).map(|v| (k, v))).collect();
        for (k, _) in &estilos {
            self.styles.remove(k);
        }
        for (k, v) in estilos {
            self.styles.insert(novo(k), v);
        }
        let textos: Vec<(CellKey, String)> = self.textos.iter().filter(|(&k, _)| dentro(k)).map(|(&k, v)| (k, v.to_owned())).collect();
        for (k, _) in &textos {
            self.textos.remove(k);
        }
        for (k, v) in textos {
            self.textos.insert(novo(k), &v);
        }
        self.permutar_comentarios(first, c0, c1, &destino_de);

        // 3) As que voltam: a fórmula com o texto deslocado para a linha nova.
        let mut quebradas = Vec::new();
        let quebrou = |antes: &str, depois: &str| depois.matches("#REF!").count() > antes.matches("#REF!").count();
        for (k, origem, cell) in voltam {
            let (row, col) = (key_row(k), key_col(k));
            if cell.raw.starts_with('=') {
                let raw = crate::operacoes::fill::shift_formula(&cell.raw, row as i32 - origem as i32, 0);
                if quebrou(&cell.raw, &raw) {
                    quebradas.push((origem, col, cell.raw.to_string(), cell.style_idx));
                }
                self.set_cell_raw(row, col, raw, cell.style_idx);
            } else {
                self.cells.insert(k, cell);
            }
        }
        for (destino, origem, col, raw) in regravar {
            let novo = crate::operacoes::fill::shift_formula(&raw, destino as i32 - origem as i32, 0);
            if quebrou(&raw, &novo) {
                quebradas.push((origem, col, raw.to_string(), None));
            }
            self.set_cell_raw(destino, col, novo, None);
        }

        // 4) Quem lia o que andou recalcula: o cálculo é preguiçoso, só o que
        //    aparecer paga.
        self.formulas.sujar_fora_de(first, last, c0, c1);
        {
            let mut calc = self.calc.borrow_mut();
            calc.cache.clear();
            calc.dirty.clear();
            calc.evaluating.clear();
        }
        self.bounds_dirty = true;
        quebradas
    }

    /// Depois de inserir/excluir linhas ou colunas: o deslocamento não entra
    /// no desfazer, então nenhum passo anterior (nem os da transação aberta,
    /// de uma macro) pode ser repetido nas coordenadas velhas.
    pub fn esquecer_historico(&mut self) {
        self.clear_history();
        if let Some(pendente) = self.hist.pendente.as_mut() {
            pendente.clear();
        }
    }

    pub fn clear_history(&mut self) {
        self.hist.desfazer.clear();
        self.hist.refazer.clear();
        self.hist.arquivo_divisao = None;
        self.hist.memoria_divisao = 0;
        self.hist.descartadas = 0;
    }

    /// Quantas transações há para desfazer, contando as descartadas pelo
    /// teto: uma marca de tempo (`profundidadeDoHistorico` do TypeScript).
    pub fn profundidade_do_historico(&self) -> usize {
        self.hist.descartadas + self.hist.desfazer.len()
    }

    /// O que as transações empilhadas desde `desde` (uma
    /// [`Self::profundidade_do_historico`] guardada antes da ação) fizeram e a
    /// proteção da aba não deixa. `None`: aba sem proteção ou tudo permitido.
    pub fn examinar_protecao(&self, desde: usize) -> Option<super::protecao::Recusa> {
        let prot = self.protecao.as_ref()?;
        let inicio = desde.saturating_sub(self.hist.descartadas);
        self.hist.desfazer.iter().skip(inicio).flatten()
            .find_map(|passo| self.recusa_do_passo(passo, prot))
    }

    fn recusa_do_passo(&self, passo: &Passo, prot: &super::protecao::ProtecaoPlanilha) -> Option<super::protecao::Recusa> {
        use super::protecao::{Permissao as P, Recusa};
        let pede = |p: P| (!prot.permite(p)).then(|| Recusa::sem_permissao(p));
        let area = |a: &[u32; 4]| self.area_bloqueada(&Rect::new(a[0], a[1], a[2], a[3])).then(Recusa::celula_bloqueada);
        match passo {
            Passo::Celula(ch) => {
                let raw = |s: &Option<Snap>| s.as_ref().map_or("", |s| s.raw.as_str()).to_string();
                let estilo = |s: &Option<Snap>| s.as_ref().and_then(|s| s.style.clone());
                if raw(&ch.antes) != raw(&ch.depois) && self.celula_marcada_bloqueada(key_row(ch.k), key_col(ch.k)) {
                    return Some(Recusa::celula_bloqueada());
                }
                (estilo(&ch.antes) != estilo(&ch.depois)).then(|| pede(P::FormatarCelulas)).flatten()
            }
            Passo::Compacto(bloco) => bloco.alteracoes.iter().find_map(|ch| {
                let (a, d) = (bloco.reconstruir(ch.antes), bloco.reconstruir(ch.depois));
                let raw = |s: &Option<Snap>| s.as_ref().map_or(String::new(), |s| s.raw.clone());
                if raw(&a) != raw(&d) && self.celula_marcada_bloqueada(key_row(ch.k), key_col(ch.k)) {
                    return Some(Recusa::celula_bloqueada());
                }
                (a.and_then(|s| s.style) != d.and_then(|s| s.style)).then(|| pede(P::FormatarCelulas)).flatten()
            }),
            Passo::CompactoNoDisco(bloco) => area(&bloco.area),
            Passo::Divisao { area: a, .. } => area(a),
            Passo::Permutacao { first, c0, c1, ordem, .. } => pede(P::Classificar)
                .or_else(|| area(&[*first, *c0, first + ordem.len() as u32 - 1, *c1])),
            Passo::Dimensao { coluna: true, .. } | Passo::LarguraPadrao { .. }
            | Passo::OcultasDesde { coluna: true, .. } => pede(P::FormatarColunas),
            Passo::OcultasDesde { coluna: false, .. } => pede(P::FormatarLinhas),
            // O filtro também esconde linhas.
            Passo::Dimensao { coluna: false, .. } => (!prot.permite(P::AutoFiltro)).then(|| pede(P::FormatarLinhas)).flatten(),
            Passo::Mesclas { .. } | Passo::Condicionais { .. } => pede(P::FormatarCelulas),
            Passo::Tabelas { .. } => (!prot.permite(P::AutoFiltro)).then(Recusa::planilha_protegida),
            Passo::Dinamicas { .. } => pede(P::TabelaDinamica),
            Passo::Imagens { .. } | Passo::Botoes { .. } | Passo::Tinta { .. } | Passo::Objetos { .. } | Passo::Graficos { .. }
            | Passo::Comentario { .. } => pede(P::EditarObjetos),
            Passo::Validacoes { .. } | Passo::Topicos { .. } => Some(Recusa::planilha_protegida()),
        }
    }

    /// Desfaz até a pilha voltar a `desde` e tira essas transações do
    /// refazer: a ação recusada some como se não tivesse acontecido.
    pub fn desfazer_ate(&mut self, desde: usize) {
        let mut desfeitas = 0;
        while self.profundidade_do_historico() > desde && self.undo() { desfeitas += 1; }
        let fica = self.hist.refazer.len().saturating_sub(desfeitas);
        self.hist.refazer.truncate(fica);
    }

    pub fn profundidade_do_refazer(&self) -> usize {
        self.hist.refazer.len()
    }

    /// Joga fora o que foi empilhado depois da marca (`truncarHistorico`).
    pub fn truncar_historico(&mut self, profundidade: usize) {
        let manter = profundidade.saturating_sub(self.hist.descartadas);
        self.hist.desfazer.truncate(manter);
    }

    /// Desloca linhas (`coluna == false`) ou colunas inteiras: o
    /// armazenamento, o texto digitado guardado à parte e o estilo de linha
    /// ou coluna inteira, que anda junto. O texto das fórmulas é da
    /// [`crate::planilha::workbook::Workbook::mover_estrutura`], que enxerga as outras
    /// abas.
    pub fn deslocar(&mut self, coluna: bool, at: u32, count: u32, removing: bool) {
        self.tocar();
        self.rastro.borrow_mut().desconhecer();
        if count == 0 {
            return;
        }
        match (coluna, removing) {
            (true, true) => self.delete_cols(at, count),
            (true, false) => self.insert_cols(at, count),
            (false, true) => self.delete_rows(at, count),
            (false, false) => self.insert_rows(at, count),
        }
        let destino = |p: u32| -> Option<u32> {
            if !removing {
                return Some(if p >= at { p + count } else { p });
            }
            if p >= at && p < at + count {
                return None;
            }
            Some(if p >= at + count { p - count } else { p })
        };
        let textos = std::mem::take(&mut self.textos);
        for (&k, t) in textos.iter() {
            let (r, c) = (key_row(k), key_col(k));
            let novo = if coluna { destino(c).map(|c| key(r, c)) } else { destino(r).map(|r| key(r, c)) };
            if let Some(n) = novo {
                self.textos.insert(n, t);
            }
        }
        self.topicos.deslocar(coluna, at, count, removing);
        self.deslocar_comentarios(coluna, at, count, removing);
        let mapa = if coluna { &mut self.col_styles } else { &mut self.row_styles };
        let velhos = std::mem::take(mapa);
        for (p, e) in velhos {
            if let Some(n) = destino(p) {
                mapa.insert(n, e);
            }
        }
        self.bounds_dirty = true;
    }

    /// Suja todas as fórmulas que leem de fora da aba. A `Workbook` chama
    /// depois de qualquer mudança em outra aba.
    pub(crate) fn tem_formulas_externas(&self) -> bool {
        !self.externas.is_empty() || self.formulas.tem_externas()
    }

    /// Invalidação conservadora por coluna, preservando as colunas que não
    /// dependem das consultas externas. Mudar Produtos não deve obrigar a
    /// calcular novamente uma corrente de 1 milhão que não lê PROCX.
    pub(super) fn invalidar_externas_compactas(&self) {
        let usos = self.formulas.columns_in_use();
        let mut colunas: std::collections::HashSet<u32> = self.externas.iter()
            .map(|&k| key_col(k)).collect();
        for uso in &usos {
            if uso.usos.iter().any(|p| p.refs.iter().any(|r| r.sheet.is_some())) {
                colunas.insert(uso.col);
            }
        }
        self.invalidar_colunas_e_dependentes(colunas, &usos);
    }

    /// Suja as colunas dadas e, por coluna, tudo o que depende delas nesta aba.
    pub(super) fn invalidar_colunas_e_dependentes(
        &self,
        mut colunas: std::collections::HashSet<u32>,
        usos: &[crate::formula::formula_table::ColumnUse],
    ) {
        loop {
            let antes = colunas.len();
            for uso in usos {
                if uso.usos.iter().any(|p| p.refs.iter().any(|r| {
                    let local = r.sheet.as_deref().is_none_or(|s| super::mesmo_nome(s, &self.name));
                    let c = if r.col_lock { r.col } else { uso.col as i32 + r.col };
                    local && c >= 0 && colunas.contains(&(c as u32))
                }) || p.faixas.iter().any(|f| f.colunas_em(uso.col)
                    .is_some_and(|(c0, c1)| colunas.iter().any(|c| *c >= c0 && *c <= c1)))) {
                    colunas.insert(uso.col);
                }
            }
            for (&k, refs) in &self.deps.precedents {
                if refs.iter().any(|&r| colunas.contains(&key_col(r))) {
                    colunas.insert(key_col(k));
                }
            }
            for (&k, faixas) in &self.deps.range_deps {
                if faixas.iter().any(|r| colunas.iter().any(|c| *c >= r.c0 && *c <= r.c1)) {
                    colunas.insert(key_col(k));
                }
            }
            if antes == colunas.len() { break; }
        }
        self.formulas.marcar_colunas_sujas(&colunas);
        let mut calc = self.calc.borrow_mut();
        calc.cache.retain(|k, _| !colunas.contains(&key_col(*k)));
        calc.dirty.retain(|k| !colunas.contains(&key_col(*k)));
    }

    /// Suja só as fórmulas que leem a aba `aba` em alguma coluna de `rects`
    /// (e quem depende delas). Devolve se sujou alguma.
    pub fn sujar_externas_de(&mut self, aba: &str, rects: &[Rect]) -> bool {
        let toca = |c0: u32, c1: u32| rects.iter().any(|r| r.c0 <= c1 && c0 <= r.c1);
        // Avulsas: as que citam a aba (ou um nome, que pode apontar para ela).
        let avulsas: Vec<CellKey> = self.externas.iter().copied().filter(|k| {
            self.cells.get(k).and_then(|c| c.formula.as_ref()).is_none_or(|n| {
                let mut le = false;
                n.walk(&mut |no| match no {
                    Node::Name(_) => le = true,
                    Node::Ref { sheet: Some(s), col, .. } if super::mesmo_nome(s, aba) => {
                        le |= toca(*col, *col);
                    }
                    Node::Range { sheet: Some(s), c0, c1, .. } if super::mesmo_nome(s, aba) => {
                        le |= toca(*c0.min(c1), *c0.max(c1));
                    }
                    _ => {}
                });
                le
            })
        }).collect();
        // Empacotadas: por coluna, pelas pontas das referências à aba.
        let usos = self.formulas.columns_in_use();
        let mut colunas = std::collections::HashSet::new();
        if self.formulas.tem_externas() {
            for uso in &usos {
                let le = uso.usos.iter().any(|p| {
                    let pontas: Vec<u32> = p.refs.iter()
                        .filter(|r| r.sheet.as_deref().is_some_and(|s| super::mesmo_nome(s, aba)))
                        .map(|r| if r.col_lock { r.col } else { uso.col as i32 + r.col }.max(0) as u32)
                        .collect();
                    match (pontas.iter().min(), pontas.iter().max()) {
                        (Some(&c0), Some(&c1)) => toca(c0, c1),
                        _ => false,
                    }
                });
                if le {
                    colunas.insert(uso.col);
                }
            }
        }
        if avulsas.is_empty() && colunas.is_empty() {
            return false;
        }
        self.tocar();
        self.rastro.borrow_mut().desconhecer();
        if !colunas.is_empty() {
            self.invalidar_colunas_e_dependentes(colunas, &usos);
        }
        for k in avulsas {
            self.mark_dirty_cascade(k);
        }
        true
    }

    pub fn sujar_externas(&mut self) {
        self.tocar();
        // O que a fórmula de fora vai dar só se sabe calculando.
        self.rastro.borrow_mut().desconhecer();
        if self.formulas.tem_externas() {
            self.invalidar_externas_compactas();
            return;
        }
        let externas: Vec<CellKey> = self.externas.iter().copied().collect();
        for k in externas {
            self.mark_dirty_cascade(k);
        }
    }
}

#[cfg(test)]
mod testes;
