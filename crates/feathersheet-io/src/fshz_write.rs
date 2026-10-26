//! Grava o `.fshz` binário (FSH3) inteiro em Rust.
//!
//! Tradução — e correção de rumo — de `src/io/fshz-arrow.ts::montarV2` +
//! `src/io/fshz.ts::encodeBookArrow`. Até 21/09/2026 o TypeScript montava as
//! colunas Arrow, deduplicava estilo, decidia o que vira célula avulsa e só
//! então mandava os bytes prontos para o Rust gravar — ou seja, o "motor"
//! processava e o Rust só escrevia em disco, o oposto do que
//! `docs/ARQUITETURA.md` define. Aqui é o contrário: o TypeScript manda os dados
//! **crus** por coluna (nem Arrow, nem gzip, nem dedup de estilo) e é este
//! módulo que faz o trabalho pesado — monta o Arrow, deduplica, decide
//! avulsa-ou-coluna, comprime e grava.
//!
//! A gravação atual usa FSH3: assinatura + gzip de `[u32 tamanho dos
//! metadados MessagePack][metadados binários][blocos Arrow IPC]`. Textos,
//! células avulsas, estilos e atributos são MessagePack, sem JSON no disco.
//! FSH2 continua somente como formato de leitura e fixture de compatibilidade.
//! O nome `gerar_fshz2` é mantido como API de transição para os consumidores.

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;

use arrow::array::{ArrayRef, Float64Array, Int32Array, RecordBatch};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::ipc::writer::FileWriter;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

/// Abaixo desta ocupação a coluna vira célula avulsa em vez de virar nulos.
/// Mesmo valor de `fshz-arrow.ts::DENSIDADE_MINIMA`.
const DENSIDADE_MINIMA: f64 = 0.05;
/// Coluna menor que isto não compensa o cabeçalho do Arrow. Mesmo valor de
/// `fshz-arrow.ts::LINHAS_MINIMAS`.
const LINHAS_MINIMAS: u32 = 256;

const MAGIC_ARROW: &[u8; 4] = b"FSH3";

// False é omitido do MessagePack: arquivos antigos continuam compactos.
fn valor_falso(valor: &bool) -> bool { !*valor }

// --------------------------------------------------------- o que chega do TS

/// Uma célula avulsa (fórmula sem padrão, literal cujo texto não
/// reconstrói, aba pequena demais para valer coluna). O TypeScript já manda
/// o texto pronto — `v` é exatamente o que a pessoa digitou.
#[derive(Deserialize)]
pub struct CelulaAvulsaBruta {
    pub k: u64,
    pub v: String,
    /// O estilo desta célula, se houver — objeto de estilo cru, ainda não
    /// deduplicado (isso é trabalho deste módulo).
    #[serde(default)]
    pub estilo: Option<Json>,
}

/// Uma coluna numérica crua: os valores e a máscara de presença exatamente
/// como `ChunkedFloatColumn::flatten` devolve (ver `sheet-core::column_store`).
#[derive(Deserialize)]
pub struct ColunaNumBruta {
    pub col: u32,
    pub values: Vec<f64>,
    /// 1 bit por linha, menos significativo primeiro — a mesma convenção do
    /// Arrow, por isso não precisa converter nada para montar o vetor.
    pub bitmap: Vec<u8>,
}

/// Uma coluna de texto/booleano crua: índice no dicionário local da coluna +
/// o próprio dicionário — o mesmo formato que `DataTable::saveText`
/// produziria (`crates/sheet-core`).
#[derive(Deserialize)]
pub struct ColunaTxtBruta {
    pub col: u32,
    pub indices: Vec<i32>,
    pub bitmap: Vec<u8>,
    pub dict: Vec<String>,
}

#[derive(Deserialize)]
pub struct ErroBruto {
    pub code: String,
    #[serde(default)]
    pub detail: Option<String>,
}

/// Uma coluna de fórmula crua: `slots` é o padrão (24 bits altos) + tipo do
/// valor (8 baixos) por linha, `payload` é o valor já calculado.
#[derive(Deserialize)]
pub struct ColunaFormulaBruta {
    pub col: u32,
    pub slots: Vec<i32>,
    pub payload: Vec<f64>,
    pub bitmap: Vec<u8>,
    #[serde(default)]
    pub dict: Vec<String>,
    #[serde(default)]
    pub erros: Vec<ErroBruto>,
}

#[derive(Deserialize)]
pub struct AbaBruta {
    pub n: String,
    #[serde(default)]
    pub tc: Option<String>,
    /// Compatibilidade: aceita metadados de ocultação da UI antiga.
    #[serde(default, alias = "oculta", alias = "hidden")]
    pub oc: bool,
    #[serde(default)]
    pub wp: Option<u32>,
    /// Estrutura de tópicos (Agrupar): `{l, c, rl, rc}`.
    #[serde(default)]
    pub tp: Option<Json>,
    /// Proteger planilha: `{senha: {alg, hash, sal, giros} | null, p: [permissões]}`.
    #[serde(default)]
    pub pt: Option<Json>,
    /// Comentários das células: `[{r, c, a (autor), t (texto), v (visível)}]`.
    #[serde(default)]
    pub cm: Option<Json>,
    /// Linhas ocultas da linha `lo` até o fim da aba (ocultar até o fim).
    #[serde(default)]
    pub lo: Option<u32>,
    /// Colunas ocultas da coluna `co` até o fim da aba.
    #[serde(default)]
    pub co: Option<u32>,
    #[serde(default)]
    pub w: Vec<(u32, u32)>,
    #[serde(default)]
    pub h: Vec<(u32, u32)>,
    #[serde(default)]
    pub g: Option<Json>,
    #[serde(default)]
    pub pr: Option<Json>,
    #[serde(default)]
    pub r: Option<Json>,
    #[serde(default)]
    pub tb: Option<Json>,
    #[serde(default)]
    pub oq: Option<Json>,
    #[serde(default)]
    pub dn: Option<Json>,
    #[serde(default)]
    pub mc: Option<Json>,
    #[serde(default)]
    pub im: Option<Json>,
    /// As respostas guardadas das tarefas assíncronas (`IMPORTARDADOS`,
    /// função de macro): o `pending` do TypeScript. Ver [`crate::pendentes`].
    #[serde(default)]
    pub pd: Option<Json>,
    #[serde(default)]
    pub va: Option<Json>,
    /// Botões de gatilho de macro (`modelo/botao-macro.ts` do TypeScript).
    #[serde(default)]
    pub bt: Option<Json>,
    /// Tinta da aba Desenhar (âncoras + traços `FTINTA`).
    #[serde(default)]
    pub tn: Option<Json>,
    /// Objetos soltos sobre a grade (formas; ver `objetos_aba`).
    #[serde(default)]
    pub ob: Option<Json>,
    #[serde(default)]
    pub ce: Option<Json>,
    #[serde(default)]
    pub avulsas: Vec<CelulaAvulsaBruta>,
    /// `styleEntries()`: estilo direto de literal colunar, por chave de
    /// célula — separado das avulsas porque no TypeScript também é.
    #[serde(default)]
    pub estilos_literais: Vec<(u64, Json)>,
    /// Estilo direto por **faixa**: `(coluna, linha inicial, linha final,
    /// estilo)`. É como a aba viva guarda (`Sheet::styles`, Fase 2 do plano
    /// de 400 MB) e como o arquivo grava desde 16/10/2026 (`sf` no
    /// `AbaMeta`): a Planilha Pesada tinha 6 milhões de entradas em
    /// `estilos_literais`, um JSON clonado por célula — era o Ctrl+S de
    /// 12 GB.
    #[serde(default)]
    pub estilos_faixa: Vec<(u32, u32, u32, Json)>,
    /// `sheet.colStyles`: estilo de coluna inteira.
    ///
    /// ISTO NÃO ERA GRAVADO POR GRAVADOR NENHUM até 22/09/2026, e o efeito era
    /// perda silenciosa: formatar uma coluna, salvar e reabrir devolvia a
    /// coluna sem formato. Passou a doer de verdade quando o enxugador
    /// (`crate::enxugar`) começou a colapsar coluna de células fantasma em
    /// `colStyles` ao abrir — aí abrir e salvar apagava a formatação de um
    /// arquivo que antes a mantinha (pelo caminho caro de 10 milhões de
    /// células). Ver `cs` no `AbaMeta`.
    #[serde(default)]
    pub estilos_coluna: Vec<(u32, Json)>,
    /// `sheet.rowStyles`: estilo de linha inteira. Mesma história.
    #[serde(default)]
    pub estilos_linha: Vec<(u32, Json)>,
    /// `sheet.sheetStyle`: o estilo da aba inteira. Mesma história.
    #[serde(default)]
    pub estilo_aba: Option<Json>,
    /// `sheet.maxRow + 1`. `0` numa aba vazia.
    pub linhas: u32,
    #[serde(default)]
    pub colunas_num: Vec<ColunaNumBruta>,
    #[serde(default)]
    pub colunas_txt: Vec<ColunaTxtBruta>,
    #[serde(default)]
    pub colunas_formula: Vec<ColunaFormulaBruta>,
    #[serde(default)]
    pub padroes_formula: Vec<String>,
}

#[derive(Deserialize)]
pub struct PastaBruta {
    pub a: u32,
    #[serde(default)]
    pub names: Option<Json>,
    /// Estilos de célula da pessoa (Novo estilo de célula).
    #[serde(default)]
    pub es: Option<Json>,
    /// Propriedades do arquivo (autor, modificadoPor, organizacao).
    #[serde(default)]
    pub pr: Option<Json>,
    /// Catálogo de macros da pasta.
    #[serde(default)]
    pub mc: Option<Json>,
    /// Valores guardados das pastas externas citadas pelas fórmulas
    /// (`=[Vendas.fshz]Dados!A1`): ver [`crate::externos_guardados`].
    #[serde(default)]
    pub ex: Option<Json>,
    /// Formulários das macros (`sheet.showComponent`).
    #[serde(default)]
    pub fm: Option<Json>,
    pub sheets: Vec<AbaBruta>,
}

// -------------------------------------------------------- o que vai no JSON

#[derive(Serialize)]
struct ColunaMeta<'a> {
    f: String,
    c: u32,
    k: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    d: Option<Vec<&'a str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    e: Option<Vec<ErroMeta>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fv: Option<String>,
}

#[derive(Serialize)]
struct ErroMeta {
    code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

#[derive(Serialize)]
struct CelulaAvulsaMeta<'a> {
    k: u64,
    v: Cow<'a, str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    s: Option<usize>,
}

#[derive(Serialize)]
struct AbaMeta<'a> {
    n: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tc: Option<String>,
    #[serde(skip_serializing_if = "valor_falso")]
    oc: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    wp: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tp: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pt: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cm: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lo: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    co: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    w: Vec<(u32, u32)>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    h: Vec<(u32, u32)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    g: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pr: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    r: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tb: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    oq: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dn: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mc: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    im: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pd: Option<Json>,
    va: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bt: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tn: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ob: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ce: Option<Json>,
    c: Vec<CelulaAvulsaMeta<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    st: Vec<(u64, usize)>,
    /// Estilo direto por faixa: `[coluna, linha inicial, linha final,
    /// índice na tabela de estilos]`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    sf: Vec<(u32, u32, u32, usize)>,
    /// Estilo de coluna inteira: `[coluna, índice na tabela de estilos]`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cs: Vec<(u32, usize)>,
    /// Estilo de linha inteira.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    rs: Vec<(u32, usize)>,
    /// Estilo da aba inteira.
    #[serde(skip_serializing_if = "Option::is_none")]
    ss: Option<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pat: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    len: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cols: Option<Vec<ColunaMeta<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ar: Option<(usize, usize)>,
}

#[derive(Serialize)]
struct LivroMeta<'a> {
    v: u8,
    a: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    names: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    es: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pr: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mc: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ex: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fm: Option<Json>,
    t: Vec<Json>,
    s: Vec<AbaMeta<'a>>,
}

/// Deduplica estilo por texto canônico — o mesmo que `JSON.stringify` fazia
/// no TypeScript (`indexOf` em `montarV2`). `serde_json::to_string` produz
/// texto estável para o mesmo valor lógico, então serve de chave.
struct TabelaDeEstilos {
    textos: Vec<String>,
    valores: Vec<Json>,
    indice: HashMap<String, usize>,
}

impl TabelaDeEstilos {
    fn new() -> Self {
        Self { textos: Vec::new(), valores: Vec::new(), indice: HashMap::new() }
    }

    fn indexar(&mut self, estilo: &Json) -> usize {
        let texto = serde_json::to_string(estilo).unwrap_or_default();
        if let Some(&at) = self.indice.get(&texto) {
            return at;
        }
        let at = self.valores.len();
        self.valores.push(estilo.clone());
        self.indice.insert(texto.clone(), at);
        self.textos.push(texto);
        at
    }
}

/// Quantos bits ligados há na máscara, nos primeiros `length` índices —
/// tradução de `contarPresentes`.
fn contar_presentes(bitmap: &[u8], length: u32) -> u32 {
    let mut total = 0u32;
    for r in 0..length {
        if bitmap[(r >> 3) as usize] & (1 << (r & 7)) != 0 {
            total += 1;
        }
    }
    total
}

fn denso_o_bastante(bitmap: &[u8], linhas: u32) -> bool {
    linhas >= LINHAS_MINIMAS
        && (contar_presentes(bitmap, linhas) as f64) >= (linhas as f64) * DENSIDADE_MINIMA
}

fn bit_ligado(bitmap: &[u8], r: u32) -> bool {
    bitmap[(r >> 3) as usize] & (1 << (r & 7)) != 0
}

/// Um campo do bloco Arrow de uma aba, ainda sem vetor: aponta para os dados
/// crus e só vira vetor Arrow uma fatia de linhas por vez ([`escrever_bloco`]).
/// Até 17/10/2026 o gravador montava a aba inteira em vetores Arrow e depois
/// os bytes IPC inteiros na memória — três cópias dos dados além da pasta
/// crua: salvar 13,7 milhões de linhas × 46 colunas bateu 20 GB de RAM.
enum Campo<'a> {
    Float { values: &'a [f64], bitmap: &'a [u8] },
    /// `remapa`: índice global → índice no dicionário local da coluna.
    Int { values: &'a [i32], bitmap: &'a [u8], remapa: Option<Vec<i32>> },
}

impl Campo<'_> {
    fn tipo(&self) -> DataType {
        match self {
            Campo::Float { .. } => DataType::Float64,
            Campo::Int { .. } => DataType::Int32,
        }
    }

    fn fatia(&self, r0: u32, r1: u32) -> ArrayRef {
        let presente = |bitmap: &[u8], r: u32| bitmap.get((r >> 3) as usize).is_some_and(|b| b & (1 << (r & 7)) != 0);
        match self {
            Campo::Float { values, bitmap } => Arc::new(Float64Array::from_iter((r0..r1).map(|r| {
                if presente(bitmap, r) { values.get(r as usize).copied() } else { None }
            }))),
            Campo::Int { values, bitmap, remapa } => Arc::new(Int32Array::from_iter((r0..r1).map(|r| {
                if !presente(bitmap, r) { return None; }
                let v = *values.get(r as usize)?;
                Some(match remapa {
                    Some(m) => m.get(v as usize).copied().unwrap_or(v),
                    None => v,
                })
            }))),
        }
    }
}

/// Linhas por `RecordBatch`: ~64 MB de vetores por fatia, nunca menos de
/// 64 mil linhas. Múltiplo de 8 para a máscara não cruzar byte.
fn linhas_por_fatia(campos: usize) -> u32 {
    let por_linha = (campos.max(1) * 8) as u64;
    ((64 << 20) / por_linha).clamp(65_536, 4_194_304) as u32 & !7
}

/// Grava o bloco Arrow IPC de uma aba em `destino`, uma fatia de linhas por
/// `RecordBatch` — o leitor junta os blocos (`coluna_float`/`coluna_int`),
/// inclusive o da v0.1.0. O pico de memória é uma fatia, não a aba.
fn escrever_bloco<W: Write>(destino: W, nomes: &[String], campos: &[Campo], linhas: u32, aba: &str) -> Result<(), String> {
    escrever_em_fatias(destino, nomes, campos, linhas, aba, linhas_por_fatia(campos.len()))
}

fn escrever_em_fatias<W: Write>(destino: W, nomes: &[String], campos: &[Campo], linhas: u32, aba: &str, passo: u32) -> Result<(), String> {
    let fields: Vec<Field> = nomes.iter().zip(campos).map(|(n, c)| Field::new(n, c.tipo(), true)).collect();
    let schema = Arc::new(Schema::new(fields));
    let mut writer = FileWriter::try_new(destino, &schema)
        .map_err(|e| format!("Não consegui abrir o gravador Arrow de \"{aba}\": {e}"))?;
    let mut r0 = 0u32;
    while r0 < linhas {
        let r1 = linhas.min(r0.saturating_add(passo));
        let arrays: Vec<ArrayRef> = campos.iter().map(|c| c.fatia(r0, r1)).collect();
        let batch = RecordBatch::try_new(schema.clone(), arrays)
            .map_err(|e| format!("Não consegui montar a tabela Arrow de \"{aba}\": {e}"))?;
        writer.write(&batch).map_err(|e| format!("Não consegui gravar a tabela Arrow de \"{aba}\": {e}"))?;
        r0 = r1;
    }
    writer.finish().map_err(|e| format!("Não consegui fechar a tabela Arrow de \"{aba}\": {e}"))
}

/// O dicionário local de [`dicionario_local`] sem copiar os índices: devolve
/// o dicionário e a tabela global → local (`None` se já é a identidade, o
/// caso comum — `da_workbook` já manda o dicionário local).
fn remapa_local<'a>(indices: &[i32], bitmap: &[u8], texto: impl Fn(i32) -> &'a str) -> (Vec<&'a str>, Option<Vec<i32>>) {
    let mut tabela: Vec<i32> = Vec::new();
    let mut dict = Vec::new();
    let mut identidade = true;
    for (linha, &indice) in indices.iter().enumerate() {
        if bitmap.get(linha >> 3).copied().unwrap_or(0) & (1 << (linha & 7)) == 0 { continue; }
        let Ok(at) = usize::try_from(indice) else { continue };
        if at >= tabela.len() { tabela.resize(at + 1, -1); }
        if tabela[at] < 0 {
            tabela[at] = dict.len() as i32;
            identidade &= tabela[at] == indice;
            dict.push(texto(indice));
        }
    }
    (dict, (!identidade).then_some(tabela))
}

/// Monta uma aba: decide avulsa-ou-coluna, deduplica estilo, monta os
/// vetores Arrow. Devolve a meta da aba e os campos (nome -> vetor) para
/// entrar num `RecordBatch`, na mesma ordem em que `cols` os lista.
fn montar_aba<'a>(aba: &'a AbaBruta, estilos: &mut TabelaDeEstilos) -> (AbaMeta<'a>, Vec<(String, Campo<'a>)>) {
    let mut avulsas: Vec<CelulaAvulsaMeta> = aba
        .avulsas
        .iter()
        .map(|c| CelulaAvulsaMeta {
            k: c.k,
            v: Cow::Borrowed(&c.v),
            s: c.estilo.as_ref().map(|e| estilos.indexar(e)),
        })
        .collect();

    let mut colunas_meta = Vec::new();
    let mut campos: Vec<(String, Campo<'a>)> = Vec::new();
    let linhas = aba.linhas;

    if linhas >= LINHAS_MINIMAS {
        for col in &aba.colunas_num {
            if !denso_o_bastante(&col.bitmap, linhas) {
                derramar_numerica(col, linhas, &mut avulsas);
                continue;
            }
            let f = format!("n{}", col.col);
            campos.push((f.clone(), Campo::Float { values: &col.values, bitmap: &col.bitmap }));
            colunas_meta.push(ColunaMeta { f, c: col.col, k: "num", d: None, e: None, fv: None });
        }
        for col in &aba.colunas_txt {
            if !denso_o_bastante(&col.bitmap, linhas) {
                derramar_texto(col, linhas, &mut avulsas);
                continue;
            }
            let f = format!("t{}", col.col);
            let (dict, remapa) = remapa_local(&col.indices, &col.bitmap, |id| {
                col.dict.get(id as usize).map(String::as_str).unwrap_or_default()
            });
            campos.push((f.clone(), Campo::Int { values: &col.indices, bitmap: &col.bitmap, remapa }));
            colunas_meta.push(ColunaMeta {
                f,
                c: col.col,
                k: "txt",
                d: Some(dict),
                e: None,
                fv: None,
            });
        }
        // Fórmula: sempre vira coluna Arrow nesta primeira versão — ver o
        // comentário de topo do arquivo, "derrama_formula".
        for col in &aba.colunas_formula {
            let f = format!("f{}", col.col);
            let fv = format!("v{}", col.col);
            campos.push((f.clone(), Campo::Int { values: &col.slots, bitmap: &col.bitmap, remapa: None }));
            campos.push((fv.clone(), Campo::Float { values: &col.payload, bitmap: &col.bitmap }));
            colunas_meta.push(ColunaMeta {
                f,
                c: col.col,
                k: "fx",
                d: if col.dict.is_empty() { None } else { Some(col.dict.iter().map(String::as_str).collect()) },
                e: if col.erros.is_empty() {
                    None
                } else {
                    Some(
                        col.erros
                            .iter()
                            .map(|e| ErroMeta { code: e.code.clone(), detail: e.detail.clone() })
                            .collect(),
                    )
                },
                fv: Some(fv),
            });
        }
    } else {
        // Aba pequena: não compensa o cabeçalho do Arrow, tudo vira avulsa.
        // O TypeScript já mandava assim, mas a pasta que mora no Rust
        // (`da_workbook`) manda as colunas como estão — e até 23/09/2026 este
        // ramo as jogava fora caladas: salvar uma aba de 200 linhas pela
        // ponte a devolvia vazia.
        for col in &aba.colunas_num {
            derramar_numerica(col, linhas, &mut avulsas);
        }
        for col in &aba.colunas_txt {
            derramar_texto(col, linhas, &mut avulsas);
        }
        for col in &aba.colunas_formula {
            derramar_formula(col, &aba.padroes_formula, linhas, &mut avulsas);
        }
    }

    // Inclui as colunas esparsas derramadas acima.
    avulsas.sort_by_key(|c| c.k);
    let st: Vec<(u64, usize)> =
        aba.estilos_literais.iter().map(|(k, estilo)| (*k, estilos.indexar(estilo))).collect();
    let sf: Vec<(u32, u32, u32, usize)> =
        aba.estilos_faixa.iter().map(|(c, r0, r1, estilo)| (*c, *r0, *r1, estilos.indexar(estilo))).collect();
    let cs: Vec<(u32, usize)> =
        aba.estilos_coluna.iter().map(|(c, estilo)| (*c, estilos.indexar(estilo))).collect();
    let rs: Vec<(u32, usize)> =
        aba.estilos_linha.iter().map(|(r, estilo)| (*r, estilos.indexar(estilo))).collect();
    let ss: Option<usize> = aba.estilo_aba.as_ref().map(|e| estilos.indexar(e));

    let meta = AbaMeta {
        n: aba.n.clone(),
        tc: aba.tc.clone(),
        oc: aba.oc,
        wp: aba.wp,
        tp: aba.tp.clone(),
        pt: aba.pt.clone(),
        cm: aba.cm.clone(),
        lo: aba.lo,
        co: aba.co,
        w: aba.w.clone(),
        h: aba.h.clone(),
        g: aba.g.clone(),
        pr: aba.pr.clone(),
        r: aba.r.clone(),
        tb: aba.tb.clone(),
        oq: aba.oq.clone(),
        dn: aba.dn.clone(),
        mc: aba.mc.clone(),
        im: aba.im.clone(),
        pd: aba.pd.clone(),
        va: aba.va.clone(),
        bt: aba.bt.clone(),
        tn: aba.tn.clone(),
        ob: aba.ob.clone(),
        ce: aba.ce.clone(),
        c: avulsas,
        st,
        sf,
        cs,
        rs,
        ss,
        pat: aba.padroes_formula.clone(),
        len: if colunas_meta.is_empty() { None } else { Some(linhas) },
        cols: if colunas_meta.is_empty() { None } else { Some(colunas_meta) },
        ar: None, // preenchido depois de saber o deslocamento dos bytes Arrow
    };
    (meta, campos)
}

/// Remapeia IDs globais para um dicionário contendo somente os textos usados
/// nesta coluna. Nunca replica o pool da aba em todas as colunas do arquivo.
pub(crate) fn dicionario_local<'a>(indices: &mut [i32], bitmap: &[u8], texto: impl Fn(i32) -> &'a str) -> Vec<&'a str> {
    let mut remapeamento = HashMap::new();
    let mut dict = Vec::new();
    for (linha, indice) in indices.iter_mut().enumerate() {
        if bitmap.get(linha >> 3).copied().unwrap_or(0) & (1 << (linha & 7)) == 0 { continue; }
        let novo = *remapeamento.entry(*indice).or_insert_with(|| {
            let id = dict.len() as i32;
            dict.push(texto(*indice));
            id
        });
        *indice = novo;
    }
    dict
}

fn derramar_numerica(col: &ColunaNumBruta, linhas: u32, avulsas: &mut Vec<CelulaAvulsaMeta<'_>>) {
    for r in 0..linhas {
        if !bit_ligado(&col.bitmap, r) {
            continue;
        }
        let k = r as u64 * 16384 + col.col as u64;
        // `to_js_string`: a mesma regra do `Number(n)` do JavaScript. Vale
        // porque só entra na coluna quem passou no `roundTrips()` — o texto
        // digitado é sempre `String(valor)` para essas células.
        avulsas.push(CelulaAvulsaMeta { k, v: Cow::Owned(feathersheet_core::celulas::number::to_js_string(col.values[r as usize])), s: None });
    }
}

fn derramar_texto<'a>(col: &'a ColunaTxtBruta, linhas: u32, avulsas: &mut Vec<CelulaAvulsaMeta<'a>>) {
    for r in 0..linhas {
        if !bit_ligado(&col.bitmap, r) {
            continue;
        }
        let k = r as u64 * 16384 + col.col as u64;
        let idx = col.indices[r as usize];
        let texto = if idx >= 0 { col.dict.get(idx as usize).map(String::as_str).unwrap_or_default() } else { "" };
        avulsas.push(CelulaAvulsaMeta { k, v: Cow::Borrowed(texto), s: None });
    }
}

/// A fórmula volta a ser texto, reconstruído do padrão. O valor calculado
/// não vai junto (a avulsa não tem onde guardá-lo): a aba é pequena, e
/// recalcular ao abrir é o que o TypeScript também fazia neste caso.
fn derramar_formula(col: &ColunaFormulaBruta, padroes: &[String], linhas: u32, avulsas: &mut Vec<CelulaAvulsaMeta<'_>>) {
    for r in 0..linhas.min(col.slots.len() as u32) {
        if !bit_ligado(&col.bitmap, r) {
            continue;
        }
        let id = (col.slots[r as usize] >> feathersheet_core::formula::formula_table::KIND_BITS) as usize;
        let Some(padrao) = padroes.get(id) else { continue };
        let k = r as u64 * 16384 + col.col as u64;
        avulsas.push(CelulaAvulsaMeta {
            k,
            v: Cow::Owned(feathersheet_core::formula::formula_pattern::expand_pattern(padrao, r as i32, col.col as i32)),
            s: None,
        });
    }
}

/// Os blocos Arrow das abas, ainda por gravar: nome da aba, linhas, nomes
/// dos campos e campos.
type Blocos<'a> = Vec<(&'a str, u32, Vec<String>, Vec<Campo<'a>>)>;

/// Monta a meta do livro e o plano dos blocos Arrow. O tamanho de cada bloco
/// (o `ar` da meta, que vai antes dos blocos no arquivo) sai de uma passada
/// que só conta os bytes ([`Contador`]); os bytes de verdade são gerados de
/// novo, fatia a fatia, direto no compressor ([`gravar_blocos`]).
fn montar_partes(pasta: &PastaBruta) -> Result<(LivroMeta<'_>, Blocos<'_>), String> {
    let mut estilos = TabelaDeEstilos::new();
    let mut abas_meta = Vec::with_capacity(pasta.sheets.len());
    let mut blocos: Blocos = Vec::new();
    let mut deslocamento: usize = 0;

    for aba in &pasta.sheets {
        let (mut meta, campos) = montar_aba(aba, &mut estilos);
        if !campos.is_empty() {
            let (nomes, campos): (Vec<String>, Vec<Campo>) = campos.into_iter().unzip();
            let mut contador = Contador::default();
            escrever_bloco(&mut contador, &nomes, &campos, aba.linhas, &aba.n)?;
            let tamanho = usize::try_from(contador.0).map_err(|_| "Bloco Arrow grande demais".to_string())?;
            meta.ar = Some((deslocamento, tamanho));
            deslocamento = deslocamento.checked_add(tamanho).ok_or("Região Arrow grande demais")?;
            blocos.push((&aba.n, aba.linhas, nomes, campos));
        }
        abas_meta.push(meta);
    }

    let meta = LivroMeta {
        v: 3,
        a: pasta.a,
        names: pasta.names.clone(),
        es: pasta.es.clone(),
        pr: pasta.pr.clone(),
        mc: pasta.mc.clone(),
        ex: pasta.ex.clone(),
        fm: pasta.fm.clone(),
        t: estilos.valores,
        s: abas_meta,
    };
    Ok((meta, blocos))
}

fn gravar_blocos<W: Write>(destino: &mut W, blocos: &Blocos) -> Result<(), String> {
    for (aba, linhas, nomes, campos) in blocos {
        escrever_bloco(&mut *destino, nomes, campos, *linhas, aba)?;
    }
    Ok(())
}

/// Conta a serialização sem manter outro buffer de centenas de MB na memória.
#[derive(Default)]
pub(crate) struct Contador(pub(crate) u64);
impl Write for Contador {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self.0.checked_add(bytes.len() as u64)
            .ok_or_else(|| std::io::Error::other("Metadados grandes demais"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

/// Grava metadados binários e Arrow diretamente no compressor, sem corpo
/// descomprimido contíguo nem cópia intermediária dos blocos Arrow.
pub(crate) fn gravar_binario<W: Write, T: Serialize>(destino: W, meta: &T, blocos: &[Vec<u8>]) -> Result<(), String> {
    gravar_binario_com(destino, meta, |enc| {
        for bloco in blocos {
            enc.write_all(bloco).map_err(|e| e.to_string())?;
        }
        Ok(())
    })
}

/// [`gravar_binario`] com os blocos gerados direto no compressor.
fn gravar_binario_com<W: Write, T: Serialize>(
    destino: W,
    meta: &T,
    blocos: impl FnOnce(&mut GzEncoder<W>) -> Result<(), String>,
) -> Result<(), String> {
    let mut contador = Contador::default();
    rmp_serde::encode::write_named(&mut contador, meta).map_err(|e| e.to_string())?;
    let tamanho = u32::try_from(contador.0).map_err(|_| "Metadados FSH3 excedem 4 GiB".to_string())?;
    let mut enc = GzEncoder::new(destino, Compression::default());
    enc.write_all(&tamanho.to_le_bytes()).map_err(|e| e.to_string())?;
    rmp_serde::encode::write_named(&mut enc, meta).map_err(|e| format!("Não consegui gravar os metadados binários: {e}"))?;
    blocos(&mut enc)?;
    enc.finish().map_err(|e| format!("Não consegui comprimir: {e}"))?;
    Ok(())
}

/// Gera FSH3 integralmente binário. Nome mantido para compatibilidade da API;
/// a assinatura gravada é FSH3, e o leitor também aceita os arquivos FSH2 antigos.
pub fn gerar_fshz2(pasta: &PastaBruta) -> Result<Vec<u8>, String> {
    let mut saida = Vec::new();
    gravar_fshz_para(&mut saida, pasta)?;
    Ok(saida)
}

/// Escreve diretamente em um arquivo/stream, sem buffer do arquivo final.
pub fn gravar_fshz_para<W: Write>(mut destino: W, pasta: &PastaBruta) -> Result<(), String> {
    let (meta, blocos) = montar_partes(pasta)?;
    destino.write_all(MAGIC_ARROW).map_err(|e| e.to_string())?;
    gravar_binario_com(destino, &meta, |enc| gravar_blocos(enc, &blocos))
}

/// Fixture FSH2 para testar a leitura dos arquivos já existentes.
#[cfg(test)]
pub(crate) fn gerar_fshz2_legado(pasta: &PastaBruta) -> Result<Vec<u8>, String> {
    let (mut meta, blocos) = montar_partes(pasta)?;
    meta.v = 2;
    let json = serde_json::to_vec(&meta).map_err(|e| e.to_string())?;
    let mut saida = b"FSH2".to_vec();
    let mut enc = GzEncoder::new(&mut saida, Compression::fast());
    enc.write_all(&(json.len() as u32).to_le_bytes()).map_err(|e| e.to_string())?;
    enc.write_all(&json).map_err(|e| e.to_string())?;
    gravar_blocos(&mut enc, &blocos)?;
    enc.finish().map_err(|e| e.to_string())?;
    Ok(saida)
}

#[cfg(test)]
mod testes {
    use super::*;
    use arrow::array::Array as _;
    use arrow::ipc::reader::FileReader;
    use std::io::Cursor;
    use std::io::Read as _;

    #[test]
    fn bloco_em_fatias_volta_igual_ao_inteiro() {
        let linhas = 100u32;
        let values: Vec<f64> = (0..linhas).map(f64::from).collect();
        let mut bitmap = vec![0xFFu8; 13];
        bitmap[2] = 0; // linhas 16–23 vazias
        let indices: Vec<i32> = (0..linhas as i32).map(|r| 7 - r % 3).collect();
        let (dict, remapa) = remapa_local(&indices, &bitmap, |id| ["a", "b", "c", "d", "e", "f", "x", "y"][id as usize]);
        assert_eq!(dict, ["y", "x", "f"]);
        let campos = [
            Campo::Float { values: &values, bitmap: &bitmap },
            Campo::Int { values: &indices, bitmap: &bitmap, remapa },
        ];
        let nomes = ["n0".to_string(), "t1".to_string()];
        let mut bytes = Vec::new();
        escrever_em_fatias(&mut bytes, &nomes, &campos, linhas, "Aba", 24).unwrap();
        let batches: Vec<RecordBatch> = FileReader::try_new(Cursor::new(bytes), None).unwrap().map(Result::unwrap).collect();
        assert_eq!(batches.len(), 5);
        let mut r = 0usize;
        for b in &batches {
            let n = b.column(0).as_any().downcast_ref::<Float64Array>().unwrap();
            let t = b.column(1).as_any().downcast_ref::<Int32Array>().unwrap();
            for i in 0..b.num_rows() {
                if (16..24).contains(&r) {
                    assert!(!n.is_valid(i) && !t.is_valid(i));
                } else {
                    assert_eq!(n.value(i), r as f64);
                    assert_eq!(t.value(i), (r % 3) as i32);
                }
                r += 1;
            }
        }
        assert_eq!(r, linhas as usize);
    }

    #[test]
    fn dicionario_ja_local_nao_remapeia() {
        let indices = [0, 1, 0, 2];
        let (dict, remapa) = remapa_local(&indices, &[0x0F], |id| ["a", "b", "c"][id as usize]);
        assert_eq!(dict, ["a", "b", "c"]);
        assert!(remapa.is_none());
    }

    fn bitmap_de(presentes: &[bool]) -> Vec<u8> {
        let mut bitmap = vec![0u8; (presentes.len() + 7) >> 3];
        for (r, &p) in presentes.iter().enumerate() {
            if p {
                bitmap[r >> 3] |= 1 << (r & 7);
            }
        }
        bitmap
    }

    fn descomprimir(bytes: &[u8]) -> Vec<u8> {
        assert_eq!(&bytes[0..4], MAGIC_ARROW);
        let mut descomprimido = Vec::new();
        flate2::read::GzDecoder::new(&bytes[4..]).read_to_end(&mut descomprimido).unwrap();
        let n = u32::from_le_bytes(descomprimido[..4].try_into().unwrap()) as usize;
        let meta: Json = rmp_serde::from_slice(&descomprimido[4..4+n]).unwrap();
        assert_eq!(meta["v"], 3);
        let json = serde_json::to_vec(&meta).unwrap();
        let mut saida = (json.len() as u32).to_le_bytes().to_vec();
        saida.extend_from_slice(&json);
        saida.extend_from_slice(&descomprimido[4+n..]);
        saida
    }

    #[test]
    fn aba_pequena_so_tem_avulsas() {
        let pasta = PastaBruta {
            a: 0,
            names: None,
            es: None,
            pr: None,
            mc: None,
            ex: None,
            fm: None,
            sheets: vec![AbaBruta {
                n: "Planilha1".into(),
                tc: None,
                oc: false,
                wp: None,
                tp: None,
                pt: None,
                cm: None,
                lo: None,
                co: None,
                w: vec![],
                h: vec![],
                g: None,
                pr: None,
                r: None,
                tb: None,
                oq: None,
                dn: None,
                mc: None,
                im: None,
                pd: None,
                va: None,
                bt: None,
                tn: None,
                ob: None,
                ce: None,
                avulsas: vec![CelulaAvulsaBruta { k: 0, v: "10".into(), estilo: None }],
                estilos_literais: vec![],
                estilos_faixa: vec![],
                estilos_coluna: vec![],
                estilos_linha: vec![],
                estilo_aba: None,
                linhas: 1,
                colunas_num: vec![],
                colunas_txt: vec![],
                colunas_formula: vec![],
                padroes_formula: vec![],
            }],
        };
        let bytes = gerar_fshz2(&pasta).unwrap();
        let corpo = descomprimir(&bytes);
        let meta_len = u32::from_le_bytes(corpo[0..4].try_into().unwrap()) as usize;
        let meta: Json = serde_json::from_slice(&corpo[4..4 + meta_len]).unwrap();
        assert_eq!(meta["s"][0]["c"][0]["v"], "10");
        assert!(meta["s"][0].get("cols").is_none());
    }

    #[test]
    fn coluna_densa_vira_arrow_e_volta_intacta() {
        let linhas = 300u32;
        let mut presentes = vec![false; linhas as usize];
        let mut values = vec![0.0; linhas as usize];
        for r in 0..linhas {
            presentes[r as usize] = true;
            values[r as usize] = r as f64 * 1.5;
        }
        let bitmap = bitmap_de(&presentes);

        let pasta = PastaBruta {
            a: 0,
            names: None,
            es: None,
            pr: None,
            mc: None,
            ex: None,
            fm: None,
            sheets: vec![AbaBruta {
                n: "Dados".into(),
                tc: None,
                oc: false,
                wp: None,
                tp: None,
                pt: None,
                cm: None,
                lo: None,
                co: None,
                w: vec![],
                h: vec![],
                g: None,
                pr: None,
                r: None,
                tb: None,
                oq: None,
                dn: None,
                mc: None,
                im: None,
                pd: None,
                va: None,
                bt: None,
                tn: None,
                ob: None,
                ce: None,
                avulsas: vec![],
                estilos_literais: vec![],
                estilos_faixa: vec![],
                estilos_coluna: vec![],
                estilos_linha: vec![],
                estilo_aba: None,
                linhas,
                colunas_num: vec![ColunaNumBruta { col: 2, values, bitmap }],
                colunas_txt: vec![],
                colunas_formula: vec![],
                padroes_formula: vec![],
            }],
        };
        let bytes = gerar_fshz2(&pasta).unwrap();
        let corpo = descomprimir(&bytes);
        let meta_len = u32::from_le_bytes(corpo[0..4].try_into().unwrap()) as usize;
        let meta: Json = serde_json::from_slice(&corpo[4..4 + meta_len]).unwrap();
        assert_eq!(meta["s"][0]["cols"][0]["f"], "n2");
        assert!(meta["s"][0]["c"].as_array().unwrap().is_empty());

        let ar = meta["s"][0]["ar"].as_array().unwrap();
        let off = ar[0].as_u64().unwrap() as usize;
        let len = ar[1].as_u64().unwrap() as usize;
        let arrow_bytes = &corpo[4 + meta_len + off..4 + meta_len + off + len];
        let leitor = FileReader::try_new(Cursor::new(arrow_bytes), None).unwrap();
        let tabela: Vec<_> = leitor.collect::<Result<_, _>>().unwrap();
        let coluna = tabela[0].column_by_name("n2").unwrap();
        let floats = coluna.as_any().downcast_ref::<Float64Array>().unwrap();
        assert_eq!(floats.value(0), 0.0);
        assert_eq!(floats.value(299), 299.0 * 1.5);
    }

    #[test]
    fn coluna_esparsa_derrama_para_avulsa() {
        let linhas = 300u32;
        // Só uma célula presente, bem abaixo dos 5% mínimos — deve virar avulsa.
        let mut bitmap = vec![0u8; (linhas as usize + 7) >> 3];
        bitmap[0] |= 1;
        let mut values = vec![0.0; linhas as usize];
        values[0] = 42.0;

        let pasta = PastaBruta {
            a: 0,
            names: None,
            es: None,
            pr: None,
            mc: None,
            ex: None,
            fm: None,
            sheets: vec![AbaBruta {
                n: "Dados".into(),
                tc: None,
                oc: false,
                wp: None,
                tp: None,
                pt: None,
                cm: None,
                lo: None,
                co: None,
                w: vec![],
                h: vec![],
                g: None,
                pr: None,
                r: None,
                tb: None,
                oq: None,
                dn: None,
                mc: None,
                im: None,
                pd: None,
                va: None,
                bt: None,
                tn: None,
                ob: None,
                ce: None,
                avulsas: vec![],
                estilos_literais: vec![],
                estilos_faixa: vec![],
                estilos_coluna: vec![],
                estilos_linha: vec![],
                estilo_aba: None,
                linhas,
                colunas_num: vec![ColunaNumBruta { col: 0, values, bitmap }],
                colunas_txt: vec![],
                colunas_formula: vec![],
                padroes_formula: vec![],
            }],
        };
        let bytes = gerar_fshz2(&pasta).unwrap();
        let corpo = descomprimir(&bytes);
        let meta_len = u32::from_le_bytes(corpo[0..4].try_into().unwrap()) as usize;
        let meta: Json = serde_json::from_slice(&corpo[4..4 + meta_len]).unwrap();
        assert!(meta["s"][0].get("cols").is_none());
        let avulsas = meta["s"][0]["c"].as_array().unwrap();
        assert_eq!(avulsas.len(), 1);
        assert_eq!(avulsas[0]["v"], "42");
    }

    #[test]
    fn estilo_repetido_nao_duplica_na_tabela() {
        let estilo = serde_json::json!({"negrito": true});
        let pasta = PastaBruta {
            a: 0,
            names: None,
            es: None,
            pr: None,
            mc: None,
            ex: None,
            fm: None,
            sheets: vec![AbaBruta {
                n: "Planilha1".into(),
                tc: None,
                oc: false,
                wp: None,
                tp: None,
                pt: None,
                cm: None,
                lo: None,
                co: None,
                w: vec![],
                h: vec![],
                g: None,
                pr: None,
                r: None,
                tb: None,
                oq: None,
                dn: None,
                mc: None,
                im: None,
                pd: None,
                va: None,
                bt: None,
                tn: None,
                ob: None,
                ce: None,
                avulsas: vec![
                    CelulaAvulsaBruta { k: 0, v: "a".into(), estilo: Some(estilo.clone()) },
                    CelulaAvulsaBruta { k: 1, v: "b".into(), estilo: Some(estilo.clone()) },
                ],
                estilos_literais: vec![],
                estilos_faixa: vec![],
                estilos_coluna: vec![],
                estilos_linha: vec![],
                estilo_aba: None,
                linhas: 1,
                colunas_num: vec![],
                colunas_txt: vec![],
                colunas_formula: vec![],
                padroes_formula: vec![],
            }],
        };
        let bytes = gerar_fshz2(&pasta).unwrap();
        let corpo = descomprimir(&bytes);
        let meta_len = u32::from_le_bytes(corpo[0..4].try_into().unwrap()) as usize;
        let meta: Json = serde_json::from_slice(&corpo[4..4 + meta_len]).unwrap();
        assert_eq!(meta["t"].as_array().unwrap().len(), 1);
        assert_eq!(meta["s"][0]["c"][0]["s"], 0);
        assert_eq!(meta["s"][0]["c"][1]["s"], 0);
    }

    /// Estilo de coluna, de linha e da aba inteira precisam ir para o arquivo.
    ///
    /// Não iam, e o efeito era perda silenciosa: formatar uma coluna, salvar e
    /// reabrir devolvia a coluna sem formato. Ficou grave quando o enxugador
    /// passou a colapsar coluna de células fantasma em `colStyles` ao abrir —
    /// aí abrir-e-salvar APAGAVA a formatação de um arquivo que antes a
    /// mantinha. Aconteceu com o arquivo do usuário em 22/09/2026.
    #[test]
    fn estilo_de_coluna_linha_e_aba_vai_para_o_arquivo() {
        let moeda = serde_json::json!({ "format": "currency" });
        let negrito = serde_json::json!({ "bold": true });
        let centro = serde_json::json!({ "align": "center" });
        let pasta = PastaBruta {
            a: 0,
            names: None,
            es: None,
            pr: None,
            mc: None,
            ex: None,
            fm: None,
            sheets: vec![AbaBruta {
                n: "Planilha1".into(),
                tc: None,
                oc: false,
                wp: None,
                tp: None,
                pt: None,
                cm: None,
                lo: None,
                co: None,
                w: vec![],
                h: vec![],
                g: None,
                pr: None,
                r: None,
                tb: None,
                oq: None,
                dn: None,
                mc: None,
                im: None,
                pd: None,
                va: None,
                bt: None,
                tn: None,
                ob: None,
                ce: None,
                avulsas: vec![],
                estilos_literais: vec![],
                estilos_faixa: vec![],
                estilos_coluna: vec![(7, moeda.clone()), (2, negrito.clone())],
                estilos_linha: vec![(4, negrito.clone())],
                estilo_aba: Some(centro.clone()),
                linhas: 0,
                colunas_num: vec![],
                colunas_txt: vec![],
                colunas_formula: vec![],
                padroes_formula: vec![],
            }],
        };
        let bytes = gerar_fshz2(&pasta).unwrap();
        let corpo = descomprimir(&bytes);
        let meta_len = u32::from_le_bytes(corpo[0..4].try_into().unwrap()) as usize;
        let meta: Json = serde_json::from_slice(&corpo[4..4 + meta_len]).unwrap();

        let tabela = meta["t"].as_array().unwrap();
        let estilo_de = |i: &Json| tabela[i.as_u64().unwrap() as usize].clone();

        let cs = meta["s"][0]["cs"].as_array().unwrap();
        assert_eq!(cs.len(), 2);
        assert_eq!(cs[0][0], 7);
        assert_eq!(estilo_de(&cs[0][1]), moeda);
        assert_eq!(cs[1][0], 2);
        assert_eq!(estilo_de(&cs[1][1]), negrito);

        let rs = meta["s"][0]["rs"].as_array().unwrap();
        assert_eq!(rs[0][0], 4);
        assert_eq!(estilo_de(&rs[0][1]), negrito);
        // O negrito aparece na coluna 2 e na linha 4: a tabela de estilos
        // deduplica, então é o MESMO índice nos dois lugares.
        assert_eq!(rs[0][1], cs[1][1], "estilo repetido não pode duplicar na tabela");

        assert_eq!(estilo_de(&meta["s"][0]["ss"]), centro);
    }

    /// Aba sem estilo de coluna não ganha campo nenhum — o arquivo não deve
    /// crescer por causa de uma funcionalidade que não está em uso.
    #[test]
    fn sem_estilo_de_coluna_nao_escreve_campo() {
        let pasta = PastaBruta {
            a: 0,
            names: None,
            es: None,
            pr: None,
            mc: None,
            ex: None,
            fm: None,
            sheets: vec![AbaBruta {
                n: "Planilha1".into(),
                tc: None,
                oc: false,
                wp: None,
                tp: None,
                pt: None,
                cm: None,
                lo: None,
                co: None,
                w: vec![],
                h: vec![],
                g: None,
                pr: None,
                r: None,
                tb: None,
                oq: None,
                dn: None,
                mc: None,
                im: None,
                pd: None,
                va: None,
                bt: None,
                tn: None,
                ob: None,
                ce: None,
                avulsas: vec![],
                estilos_literais: vec![],
                estilos_faixa: vec![],
                estilos_coluna: vec![],
                estilos_linha: vec![],
                estilo_aba: None,
                linhas: 0,
                colunas_num: vec![],
                colunas_txt: vec![],
                colunas_formula: vec![],
                padroes_formula: vec![],
            }],
        };
        let bytes = gerar_fshz2(&pasta).unwrap();
        let corpo = descomprimir(&bytes);
        let meta_len = u32::from_le_bytes(corpo[0..4].try_into().unwrap()) as usize;
        let meta: Json = serde_json::from_slice(&corpo[4..4 + meta_len]).unwrap();
        assert!(meta["s"][0].get("cs").is_none());
        assert!(meta["s"][0].get("rs").is_none());
        assert!(meta["s"][0].get("ss").is_none());
    }

}
