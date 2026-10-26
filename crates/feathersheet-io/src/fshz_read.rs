//! Lê o `.fshz` inteiro em Rust.
//!
//! Tradução — no mesmo espírito de correção de rumo de [`crate::fshz_write`]
//! — de `src/io/fshz.ts::decode`/`decodeInto` + `src/io/fshz-arrow.ts::carregarV2`.
//! Até aqui só a escrita (`gravar_fshz2`) tinha saído do TypeScript; a leitura
//! continuava lá, e um arquivo colunar de verdade nem abria direito pelo
//! `ler_fshz` antigo do `src-tauri` (só gunzipava e devolvia texto — quebra
//! em qualquer coisa que não seja JSON puro).
//!
//! Este módulo devolve os dados **crus**, no mesmo espírito de
//! [`crate::fshz_write::PastaBruta`] em espelho: quem entende o que é uma
//! `Sheet` continua sendo o `sheet.rs`/TypeScript do outro lado — aqui só se
//! descomprime, desmonta o envelope, deduplica-e-resolve estilo e devolve as
//! colunas Arrow como `values`/`bitmap`/`dict` crus, exatamente a forma que
//! `dataColumns.loadNumeric`/`loadText` e `formulaColumns.loadColumn` (TypeScript)
//! ou o futuro `sheet.rs` esperam para aplicar.

use std::io::{BufReader, Cursor};
use std::io::Read;

use arrow::array::{Array, Int32Array, RecordBatch};
use arrow::ipc::reader::FileReader;
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

const MAGIC_ARROW: &[u8; 4] = b"FSH2";

// -------------------------------------------------------- o que chega do disco

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ColunaMetaLida {
    f: String,
    c: u32,
    k: String,
    #[serde(default)]
    d: Option<Vec<String>>,
    #[serde(default)]
    e: Option<Vec<ErroMetaLida>>,
    #[serde(default)]
    fv: Option<String>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
struct ErroMetaLida {
    code: String,
    #[serde(default)]
    detail: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CelulaAvulsaMetaLida {
    k: u64,
    v: String,
    #[serde(default)]
    s: Option<usize>,
}

/// Espelha `LivroMetaLida::s` — os campos batem com `CompactSheet` do
/// `fshz.ts`.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AbaMetaLida {
    n: String,
    #[serde(default)]
    tc: Option<String>,
    #[serde(default, alias = "oculta", alias = "hidden")]
    oc: bool,
    /// Largura padrão das colunas da aba, em pixels (Formatar ▾ → Largura padrão).
    #[serde(default)]
    wp: Option<u32>,
    #[serde(default)]
    tp: Option<Json>,
    #[serde(default)]
    pt: Option<Json>,
    #[serde(default)]
    cm: Option<Json>,
    #[serde(default)]
    lo: Option<u32>,
    #[serde(default)]
    co: Option<u32>,
    #[serde(default)]
    w: Vec<(u32, f64)>,
    #[serde(default)]
    h: Vec<(u32, f64)>,
    #[serde(default)]
    g: Option<Json>,
    #[serde(default)]
    pr: Option<Json>,
    #[serde(default)]
    r: Option<Json>,
    #[serde(default)]
    tb: Option<Json>,
    #[serde(default)]
    oq: Option<Json>,
    #[serde(default)]
    dn: Option<Json>,
    #[serde(default)]
    mc: Option<Json>,
    #[serde(default)]
    im: Option<Json>,
    #[serde(default)]
    pd: Option<Json>,
    va: Option<Json>,
    #[serde(default)]
    bt: Option<Json>,
    #[serde(default)]
    tn: Option<Json>,
    #[serde(default)]
    ob: Option<Json>,
    #[serde(default)]
    ce: Option<Json>,
    #[serde(default)]
    c: Vec<CelulaAvulsaMetaLida>,
    #[serde(default)]
    st: Vec<(u64, usize)>,
    /// Estilo direto por faixa (arquivos de 16/10/2026 em diante).
    #[serde(default)]
    sf: Vec<(u32, u32, u32, usize)>,
    #[serde(default)]
    cs: Vec<(u32, usize)>,
    #[serde(default)]
    rs: Vec<(u32, usize)>,
    #[serde(default)]
    ss: Option<usize>,
    #[serde(default)]
    pat: Vec<String>,
    #[serde(default)]
    len: Option<u32>,
    #[serde(default)]
    cols: Option<Vec<ColunaMetaLida>>,
    #[serde(default)]
    ar: Option<(usize, usize)>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LivroMetaLida {
    v: u32,
    a: u32,
    #[serde(default)]
    names: Option<Json>,
    #[serde(default)]
    es: Option<Json>,
    /// Propriedades do arquivo (autor, modificadoPor, organizacao).
    #[serde(default)]
    pr: Option<Json>,
    /// Catálogo de macros da pasta.
    #[serde(default)]
    mc: Option<Json>,
    /// Valores guardados das pastas externas (ver `crate::externos_guardados`).
    #[serde(default)]
    ex: Option<Json>,
    /// Formulários das macros (`sheet.showComponent`).
    #[serde(default)]
    fm: Option<Json>,
    #[serde(default)]
    t: Vec<Json>,
    s: Vec<AbaMetaLida>,
}

// --------------------------------------------------------- o que volta pro TS

#[derive(Serialize)]
pub struct CelulaAvulsaLida {
    pub k: u64,
    pub v: String,
    /// Posição na tabela [`AbaLida::estilos`].
    pub estilo: Option<usize>,
}

#[derive(Serialize)]
pub struct ColunaNumLida {
    pub col: u32,
    pub values: Vec<f64>,
    pub bitmap: Vec<u8>,
}

#[derive(Serialize)]
pub struct ColunaTxtLida {
    pub col: u32,
    pub indices: Vec<i32>,
    pub bitmap: Vec<u8>,
    pub dict: Vec<String>,
}

#[derive(Serialize)]
pub struct ErroLida {
    pub code: String,
    pub detail: Option<String>,
}

#[derive(Serialize)]
pub struct ColunaFormulaLida {
    pub col: u32,
    pub slots: Vec<i32>,
    pub payload: Vec<f64>,
    pub bitmap: Vec<u8>,
    pub dict: Vec<String>,
    pub erros: Vec<ErroLida>,
}

#[derive(Serialize)]
pub struct AbaLida {
    pub n: String,
    pub tc: Option<String>,
    pub oc: bool,
    pub wp: Option<u32>,
    pub tp: Option<Json>,
    pub pt: Option<Json>,
    pub cm: Option<Json>,
    pub lo: Option<u32>,
    pub co: Option<u32>,
    pub avulsas: Vec<CelulaAvulsaLida>,
    /// Os estilos distintos do arquivo, uma vez só (partilhados entre as
    /// abas). Célula guarda a posição aqui, não uma cópia: eram 6 milhões
    /// de objetos JSON clonados — 3,4 GB para abrir um `.fshz` de 43 MB
    /// (13/10/2026).
    #[serde(skip)]
    pub estilos: std::sync::Arc<Vec<Json>>,
    /// Estilo direto de literal colunar (`styleEntries`/`setStyleDirect`) —
    /// separado das avulsas porque no TypeScript também é. Posição em
    /// [`Self::estilos`].
    pub estilos_diretos: Vec<(u64, usize)>,
    /// Estilo direto por faixa: `(coluna, linha inicial, linha final,
    /// posição em [`Self::estilos`])`. Arquivos antigos só têm
    /// [`Self::estilos_diretos`]; os dois convivem na leitura.
    pub estilos_faixa: Vec<(u32, u32, u32, usize)>,
    pub estilos_coluna: Vec<(u32, Json)>,
    pub estilos_linha: Vec<(u32, Json)>,
    pub estilo_aba: Option<Json>,
    pub colunas_num: Vec<ColunaNumLida>,
    pub colunas_txt: Vec<ColunaTxtLida>,
    pub colunas_formula: Vec<ColunaFormulaLida>,
    pub padroes_formula: Vec<String>,
    pub w: Vec<(u32, f64)>,
    pub h: Vec<(u32, f64)>,
    pub g: Option<Json>,
    pub pr: Option<Json>,
    pub r: Option<Json>,
    pub tb: Option<Json>,
    pub oq: Option<Json>,
    pub dn: Option<Json>,
    pub mc: Option<Json>,
    pub im: Option<Json>,
    /// As respostas guardadas das tarefas assíncronas (`IMPORTARDADOS`,
    /// função de macro): o `pending` do TypeScript. Ver [`crate::pendentes`].
    pub pd: Option<Json>,
    pub va: Option<Json>,
    /// Botões de gatilho de macro (`modelo/botao-macro.ts` do TypeScript).
    pub bt: Option<Json>,
    /// Tinta da aba Desenhar (âncoras + traços `FTINTA`), opaca para o leitor.
    pub tn: Option<Json>,
    /// Objetos soltos sobre a grade (formas; ver `objetos_aba`), opacos para o leitor.
    pub ob: Option<Json>,
    pub ce: Option<Json>,
}

#[derive(Serialize)]
pub struct PastaLida {
    /// Versão do formato lido: 2 (legado híbrido) ou 3 (binário).
    pub v: u32,
    pub a: u32,
    pub names: Option<Json>,
    pub es: Option<Json>,
    pub pr: Option<Json>,
    pub mc: Option<Json>,
    /// Valores guardados das pastas externas (ver `crate::externos_guardados`).
    pub ex: Option<Json>,
    /// Formulários das macros (`sheet.showComponent`).
    pub fm: Option<Json>,
    pub sheets: Vec<AbaLida>,
}

// ------------------------------------------------------------------- leitura

fn gunzip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut saida = Vec::new();
    GzDecoder::new(bytes).read_to_end(&mut saida).map_err(|e| e.to_string())?;
    Ok(saida)
}

fn tabela_de(arrow_bytes: &[u8]) -> Result<Vec<RecordBatch>, String> {
    let leitor = FileReader::try_new(Cursor::new(arrow_bytes), None)
        .map_err(|e| format!("Não consegui abrir a tabela Arrow: {e}"))?;
    leitor.collect::<Result<Vec<_>, _>>().map_err(|e| format!("Não consegui ler a tabela Arrow: {e}"))
}

/// As colunas Arrow da aba, já em vetores (ver [`crate::arrow_fluxo`]).
type ColunasArrow = std::collections::HashMap<String, crate::arrow_fluxo::ColunaArrow>;

fn tirar_float(colunas: &mut ColunasArrow, campo: &str) -> Option<(Vec<f64>, Vec<u8>)> {
    match colunas.remove(campo)? {
        crate::arrow_fluxo::ColunaArrow::Float(v, m) => Some((v, m)),
        crate::arrow_fluxo::ColunaArrow::Int(..) => None,
    }
}

fn tirar_int(colunas: &mut ColunasArrow, campo: &str) -> Option<(Vec<i32>, Vec<u8>)> {
    match colunas.remove(campo)? {
        crate::arrow_fluxo::ColunaArrow::Int(v, m) => Some((v, m)),
        crate::arrow_fluxo::ColunaArrow::Float(..) => None,
    }
}

/// As colunas do bloco Arrow da aba, de uma fatia já na memória (FSH2 antigo).
fn colunas_da_fatia(aba: &AbaMetaLida, arrow_bytes: &[u8]) -> Result<ColunasArrow, String> {
    let (Some((off, len)), Some(linhas)) = (aba.ar, aba.len) else { return Ok(Default::default()) };
    let fatia = arrow_bytes
        .get(off..off.checked_add(len).ok_or("Fatia Arrow excede o intervalo")?)
        .ok_or_else(|| format!("Fatia Arrow fora dos limites na aba \"{}\"", aba.n))?;
    crate::arrow_fluxo::ler_colunas(fatia, len, linhas)
}

fn montar_aba_lida(aba: AbaMetaLida, estilos: &std::sync::Arc<Vec<Json>>, mut arrow: ColunasArrow) -> Result<AbaLida, String> {
    let existe = |i: &usize| *i < estilos.len();
    let avulsas = aba
        .c
        .into_iter()
        .map(|c| CelulaAvulsaLida { k: c.k, v: c.v, estilo: c.s.filter(existe) })
        .collect();

    let mut colunas_num = Vec::new();
    let mut colunas_txt = Vec::new();
    let mut colunas_formula = Vec::new();

    if let (Some(cols), Some(_), Some(linhas)) = (aba.cols, aba.ar, aba.len) {
        for col in cols {
            match col.k.as_str() {
                "num" => {
                    if let Some((values, bitmap)) = tirar_float(&mut arrow, &col.f) {
                        colunas_num.push(ColunaNumLida { col: col.c, values, bitmap });
                    }
                }
                "txt" => {
                    if let Some((mut indices, bitmap)) = tirar_int(&mut arrow, &col.f) {
                        let original = col.d.unwrap_or_default();
                        let dict = crate::fshz_write::dicionario_local(&mut indices, &bitmap, |id| original.get(id as usize).map(String::as_str).unwrap_or_default())
                            .into_iter().map(str::to_owned).collect();
                        colunas_txt.push(ColunaTxtLida {
                            col: col.c,
                            indices,
                            bitmap,
                            dict,
                        });
                    }
                }
                "fx" => {
                    if let Some((ids, bitmap)) = tirar_int(&mut arrow, &col.f) {
                        let (slots, payload) = match &col.fv {
                            // Formato de hoje: o campo `f` já traz o slot
                            // empacotado (padrão nos 24 bits altos, tipo do
                            // valor nos 8 baixos); `fv` traz o valor calculado.
                            Some(fv) => {
                                let payload = tirar_float(&mut arrow, fv)
                                    .map(|(v, _)| v)
                                    .unwrap_or_else(|| vec![0.0; linhas as usize]);
                                (ids, payload)
                            }
                            // Formato antigo: `f` só tinha o número do padrão.
                            // Empacota igual ao `empacotarIdsAntigos` do
                            // TypeScript — sem valor calculado, toda célula
                            // nasce suja, que é como o formato antigo se
                            // comportava mesmo.
                            None => {
                                let slots: Vec<i32> = ids.iter().map(|id| id << 8).collect();
                                (slots, vec![0.0; linhas as usize])
                            }
                        };
                        colunas_formula.push(ColunaFormulaLida {
                            col: col.c,
                            slots,
                            payload,
                            bitmap,
                            dict: col.d.unwrap_or_default(),
                            erros: col
                                .e
                                .unwrap_or_default()
                                .into_iter()
                                .map(|e| ErroLida { code: e.code, detail: e.detail })
                                .collect(),
                        });
                    }
                }
                outro => return Err(format!("Tipo de coluna desconhecido \"{outro}\" na aba \"{}\"", aba.n)),
            }
        }
    }

    let estilos_diretos = aba.st.into_iter().filter(|(_, idx)| existe(idx)).collect();
    let estilos_faixa = aba.sf.into_iter().filter(|(_, r0, r1, idx)| existe(idx) && r0 <= r1).collect();
    let estilos_coluna =
        aba.cs.iter().filter_map(|(c, idx)| estilos.get(*idx).cloned().map(|estilo| (*c, estilo))).collect();
    let estilos_linha =
        aba.rs.iter().filter_map(|(r, idx)| estilos.get(*idx).cloned().map(|estilo| (*r, estilo))).collect();
    let estilo_aba = aba.ss.and_then(|idx| estilos.get(idx).cloned());

    Ok(AbaLida {
        n: aba.n,
        tc: aba.tc,
        oc: aba.oc,
        wp: aba.wp,
        tp: aba.tp,
        pt: aba.pt,
        cm: aba.cm,
        lo: aba.lo,
        co: aba.co,
        avulsas,
        estilos: estilos.clone(),
        estilos_diretos,
        estilos_faixa,
        estilos_coluna,
        estilos_linha,
        estilo_aba,
        colunas_num,
        colunas_txt,
        colunas_formula,
        padroes_formula: aba.pat,
        w: aba.w,
        h: aba.h,
        g: aba.g,
        pr: aba.pr,
        r: aba.r,
        tb: aba.tb,
        oq: aba.oq,
        dn: aba.dn,
        mc: aba.mc,
        im: aba.im,
        pd: aba.pd,
        va: aba.va,
        bt: aba.bt,
        tn: aba.tn,
        ob: aba.ob,
        ce: aba.ce,
    })
}

fn montar_pasta_lida(livro: LivroMetaLida, arrow_bytes: &[u8]) -> Result<PastaLida, String> {
    let tabela = std::sync::Arc::new(livro.t);
    let sheets =
        livro.s.into_iter().map(|aba| {
            let colunas = colunas_da_fatia(&aba, arrow_bytes)?;
            montar_aba_lida(aba, &tabela, colunas)
        }).collect::<Result<Vec<_>, _>>()?;
    Ok(PastaLida { v: livro.v, a: livro.a, names: livro.names, es: livro.es, pr: livro.pr, mc: livro.mc, ex: livro.ex, fm: livro.fm, sheets })
}

fn ler_fshz2(resto: &[u8]) -> Result<PastaLida, String> {
    let corpo = gunzip(resto).unwrap_or_else(|_| resto.to_vec());
    if corpo.len() < 4 {
        return Err("Arquivo FSH2 truncado".to_string());
    }
    let meta_len = u32::from_le_bytes(corpo[0..4].try_into().unwrap()) as usize;
    if corpo.len() < 4 + meta_len {
        return Err("Arquivo FSH2 truncado".to_string());
    }
    let livro: LivroMetaLida =
        serde_json::from_slice(&corpo[4..4 + meta_len]).map_err(|e| format!("JSON do FSH2 inválido: {e}"))?;
    montar_pasta_lida(livro, &corpo[4 + meta_len..])
}

/// FSH3 é descomprimido em fluxo, sem string JSON nem corpo contíguo.
/// Cada bloco Arrow é liberado antes de ler a próxima aba.
/// Lê `len` bytes do fluxo descompactado em pedaços de 1 MB.
///
/// `take(len).read_to_end` sobre o `BufReader<GzDecoder>` levava 10,3 s para
/// os 335 MB do Spotify CSV.fshz (550 mil linhas × 83 colunas), contra 0,6 s
/// lendo em pedaços. A reserva inicial tem teto de 64 MB: o tamanho vem do
/// arquivo e não é confiável, então o resto cresce conforme os bytes chegam.
fn ler_bloco<R: Read>(leitor: &mut R, len: usize) -> std::io::Result<Vec<u8>> {
    let mut bloco = Vec::with_capacity(len.min(64 << 20));
    let mut pedaco = vec![0u8; 1 << 20];
    while bloco.len() < len {
        let quer = (len - bloco.len()).min(pedaco.len());
        let n = leitor.read(&mut pedaco[..quer])?;
        if n == 0 { break; }
        bloco.extend_from_slice(&pedaco[..n]);
    }
    Ok(bloco)
}

fn ler_fluxo<R: Read>(origem: R, versao: u32) -> Result<PastaLida, String> {
    let mut leitor = BufReader::new(GzDecoder::new(origem));
    let mut tamanho = [0; 4];
    leitor.read_exact(&mut tamanho).map_err(|e| format!("Cabeçalho FSH3 inválido: {e}"))?;
    let mut metadados = leitor.by_ref().take(u32::from_le_bytes(tamanho) as u64);
    let livro: LivroMetaLida = if versao == 3 {
        rmp_serde::from_read(&mut metadados).map_err(|e| format!("Metadados binários FSH3 inválidos: {e}"))?
    } else {
        serde_json::from_reader(&mut metadados).map_err(|e| format!("Metadados FSH2 inválidos: {e}"))?
    };
    if metadados.limit() != 0 || livro.v != versao {
        return Err("Tamanho ou versão dos metadados FSH3 inválido".into());
    }
    let mut deslocamento = 0usize;
    let tabela = std::sync::Arc::new(livro.t);
    let mut sheets = Vec::with_capacity(livro.s.len());
    for aba in livro.s {
        let mut colunas = ColunasArrow::default();
        if let Some((off, len)) = aba.ar {
            if off != deslocamento { return Err("Blocos Arrow FSH3 fora de ordem".into()); }
            // Direto do gzip para as colunas, sem o bloco inteiro na memória.
            colunas = crate::arrow_fluxo::ler_colunas(&mut leitor, len, aba.len.unwrap_or(0))
                .map_err(|e| format!("Bloco Arrow FSH3 inválido: {e}"))?;
            deslocamento = deslocamento.checked_add(len).ok_or("Blocos Arrow grandes demais")?;
        }
        sheets.push(montar_aba_lida(aba, &tabela, colunas)?);
    }
    // Verifica o checksum gzip inclusive quando não há colunas Arrow.
    let mut extra = [0];
    if leitor.read(&mut extra).map_err(|e| format!("Compressão FSH3 inválida: {e}"))? != 0 {
        return Err("Dados extras no fim do FSH3".into());
    }
    Ok(PastaLida { v: versao, a: livro.a, names: livro.names, es: livro.es, pr: livro.pr, mc: livro.mc, ex: livro.ex, fm: livro.fm, sheets })
}

/// Converte FSH2 em FSH3 sem reconstruir uma Workbook e sem expandir o
/// arquivo inteiro. Útil para medir/migrar documentos grandes. A leitura
/// normal continua aceitando FSH2; salvar pelo app usa o gravador nativo.
/// Compacta dicionários e remapeia seus índices Arrow sem alterar os textos,
/// valores numéricos ou fórmulas.
pub fn converter_fshz2<R: Read, W: std::io::Write>(mut origem: R, mut destino: W) -> Result<(), String> {
    let mut magic = [0; 4];
    origem.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if &magic != b"FSH2" { return Err("A conversão requer um arquivo FSH2".into()); }
    let mut leitor = BufReader::new(GzDecoder::new(origem));
    let mut tamanho = [0; 4];
    leitor.read_exact(&mut tamanho).map_err(|e| e.to_string())?;
    let mut metadados = leitor.by_ref().take(u32::from_le_bytes(tamanho) as u64);
    let mut livro: LivroMetaLida = serde_json::from_reader(&mut metadados).map_err(|e| e.to_string())?;
    if metadados.limit() != 0 || livro.v != 2 { return Err("Metadados FSH2 inválidos".into()); }
    livro.v = 3;
    for aba in &mut livro.s { aba.c.sort_by_key(|c| c.k); }
    let mut blocos = Vec::new();
    let mut entrada_off = 0usize;
    let mut saida_off = 0usize;
    for aba in &mut livro.s {
        let Some((off, len)) = aba.ar else { continue };
        if off != entrada_off { return Err("Blocos FSH2 fora de ordem".into()); }
        let bloco = ler_bloco(&mut leitor, len).map_err(|e| e.to_string())?;
        if bloco.len() != len { return Err("Bloco Arrow FSH2 truncado".into()); }
        entrada_off = entrada_off.checked_add(len).ok_or("Região Arrow grande demais")?;
        let batches = tabela_de(&bloco)?;
        let primeiro = batches.first().ok_or("Bloco Arrow sem registros")?;
        let batch = arrow::compute::concat_batches(&primeiro.schema(), &batches).map_err(|e| e.to_string())?;
        let mut arrays = batch.columns().to_vec();
        for col in aba.cols.iter_mut().flatten() {
            if col.k != "txt" { continue; }
            let at = batch.schema().index_of(&col.f).map_err(|e| e.to_string())?;
            let arr = arrays[at].as_any().downcast_ref::<Int32Array>().ok_or("Índices de texto não são Int32")?;
            let mut indices: Vec<i32> = arr.iter().map(|v| v.unwrap_or(0)).collect();
            let mut bitmap = vec![0u8; arr.len().div_ceil(8)];
            for i in 0..arr.len() { if arr.is_valid(i) { bitmap[i >> 3] |= 1 << (i & 7); } }
            let original = col.d.take().unwrap_or_default();
            col.d = Some(crate::fshz_write::dicionario_local(&mut indices, &bitmap, |id| original.get(id as usize).map(String::as_str).unwrap_or_default())
                .into_iter().map(str::to_owned).collect());
            arrays[at] = std::sync::Arc::new(Int32Array::from_iter(indices.into_iter().enumerate().map(|(i,v)|
                (bitmap[i >> 3] & (1 << (i & 7)) != 0).then_some(v))));
        }
        let novo = RecordBatch::try_new(batch.schema(), arrays).map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        let mut writer = arrow::ipc::writer::FileWriter::try_new(&mut bytes, &novo.schema()).map_err(|e| e.to_string())?;
        writer.write(&novo).map_err(|e| e.to_string())?;
        writer.finish().map_err(|e| e.to_string())?;
        drop(writer);
        aba.ar = Some((saida_off, bytes.len()));
        saida_off = saida_off.checked_add(bytes.len()).ok_or("Região Arrow grande demais")?;
        blocos.push(bytes);
    }
    let mut extra = [0];
    if leitor.read(&mut extra).map_err(|e| e.to_string())? != 0 { return Err("Dados extras FSH2".into()); }
    destino.write_all(b"FSH3").map_err(|e| e.to_string())?;
    crate::fshz_write::gravar_binario(destino, &livro, &blocos)?;
    Ok(())
}

/// Abre FSH3 diretamente do disco sem manter o arquivo compactado na RAM.
pub fn ler_fshz_arquivo(path: &std::path::Path) -> Result<PastaLida, String> {
    let mut arquivo = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut magic = [0; 4];
    arquivo.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if &magic == b"FSH3" { return ler_fluxo(crate::progresso::Contador(arquivo), 3); }
    if &magic == b"FSH2" {
        use std::io::{Seek, SeekFrom};
        let mut gzip = [0; 2];
        arquivo.read_exact(&mut gzip).map_err(|e| e.to_string())?;
        arquivo.seek(SeekFrom::Start(4)).map_err(|e| e.to_string())?;
        if gzip == [0x1f, 0x8b] { return ler_fluxo(arquivo, 2); }
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    ler_fshz(&bytes)
}

/// Lê um `.fshz`. `bytes` é o arquivo inteiro, como veio do disco — só a
/// assinaturas `FSH2` (legado) e `FSH3` são reconhecidas.
pub fn ler_fshz(bytes: &[u8]) -> Result<PastaLida, String> {
    if bytes.len() >= 4 {
        let magic: &[u8; 4] = bytes[0..4].try_into().unwrap();
        if magic == b"FSH3" { return ler_fluxo(&bytes[4..], 3); }
        if magic == MAGIC_ARROW {
            if bytes.get(4..6) == Some(&[0x1f, 0x8b]) { return ler_fluxo(&bytes[4..], 2); }
            return ler_fshz2(&bytes[4..]);
        }
    }
    Err("Arquivo do FeatherSheet (.fshz) inválido".to_string())
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::fshz_write::{
        AbaBruta, CelulaAvulsaBruta, ColunaFormulaBruta, ColunaNumBruta, ColunaTxtBruta, ErroBruto, PastaBruta,
    };

    fn bitmap_de(presentes: &[bool]) -> Vec<u8> {
        let mut bitmap = vec![0u8; (presentes.len() + 7) >> 3];
        for (r, &p) in presentes.iter().enumerate() {
            if p {
                bitmap[r >> 3] |= 1 << (r & 7);
            }
        }
        bitmap
    }

    fn aba_vazia(nome: &str) -> AbaBruta {
        AbaBruta {
            n: nome.into(),
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
        }
    }

    /// A proteção de intervalo (só o hash da senha) atravessa gravar e ler.
    /// Sem o campo `pr` aqui, salvar pelo app instalado apagava a trava.
    #[test]
    fn protecao_de_intervalo_faz_ida_e_volta() {
        let protecoes = serde_json::json!([{
            "id": "p1",
            "area": { "r0": 1, "c0": 1, "r1": 3, "c1": 2 },
            "descricao": "Metas",
            "senha": { "alg": "PBKDF2-SHA256", "iteracoes": 600000, "sal": "c2Fs", "hash": "aGFzaA==" }
        }]);
        let mut aba = aba_vazia("Planilha1");
        aba.pr = Some(protecoes.clone());
        let pasta = PastaBruta { a: 0, names: None, es: None, pr: None, mc: None, ex: None, fm: None, sheets: vec![aba] };
        let bytes = crate::fshz_write::gerar_fshz2(&pasta).unwrap();

        let lida = ler_fshz(&bytes).unwrap();
        assert_eq!(lida.sheets[0].pr, Some(protecoes.clone()));

        // E o enxugador, que recusa campo desconhecido de propósito, deixa passar.
        let legado = crate::fshz_write::gerar_fshz2_legado(&pasta).unwrap();
        assert_eq!(ler_fshz(&legado).unwrap().sheets[0].pr, Some(protecoes.clone()));
        let (enxuto, _) = crate::enxugar::ler_e_enxugar(&legado).unwrap();
        let n = u32::from_le_bytes([enxuto[0], enxuto[1], enxuto[2], enxuto[3]]) as usize;
        let meta: serde_json::Value = serde_json::from_slice(&enxuto[4..4 + n]).unwrap();
        assert_eq!(meta["s"][0]["pr"], protecoes);
    }

    /// O último valor das tarefas assíncronas (`pd`) atravessa gravar, ler e
    /// o enxugador — que recusaria o campo se ele não estivesse lá.
    #[test]
    fn respostas_assincronas_fazem_ida_e_volta() {
        let pd = serde_json::json!({ "web:texto:https://x": { "v": "oi", "at": 1 }, "web:dados:https://y\u{0}": { "m": [["a", 1]], "at": 2 } });
        let mut aba = aba_vazia("Planilha1");
        aba.pd = Some(pd.clone());
        let pasta = PastaBruta { a: 0, names: None, es: None, pr: None, mc: None, ex: None, fm: None, sheets: vec![aba] };
        let bytes = crate::fshz_write::gerar_fshz2(&pasta).unwrap();
        assert_eq!(ler_fshz(&bytes).unwrap().sheets[0].pd, Some(pd.clone()));
        let legado = crate::fshz_write::gerar_fshz2_legado(&pasta).unwrap();
        let (enxuto, _) = crate::enxugar::ler_e_enxugar(&legado).unwrap();
        let n = u32::from_le_bytes([enxuto[0], enxuto[1], enxuto[2], enxuto[3]]) as usize;
        let meta: serde_json::Value = serde_json::from_slice(&enxuto[4..4 + n]).unwrap();
        assert_eq!(meta["s"][0]["pd"], pd);
    }

    #[test]
    fn avulsa_simples_volta_intacta() {
        let mut aba = aba_vazia("Planilha1");
        aba.avulsas = vec![CelulaAvulsaBruta { k: 5, v: "10".into(), estilo: None }];
        aba.linhas = 1;
        let pasta = PastaBruta { a: 0, names: None, es: None, pr: None, mc: None, ex: None, fm: None, sheets: vec![aba] };
        let bytes = crate::fshz_write::gerar_fshz2(&pasta).unwrap();

        let lida = ler_fshz(&bytes).unwrap();
        assert_eq!(lida.v, 3);
        assert_eq!(lida.sheets[0].avulsas.len(), 1);
        assert_eq!(lida.sheets[0].avulsas[0].k, 5);
        assert_eq!(lida.sheets[0].avulsas[0].v, "10");
    }

    #[test]
    fn coluna_numerica_densa_faz_ida_e_volta() {
        let linhas = 300u32;
        let mut presentes = vec![true; linhas as usize];
        presentes[10] = false; // um buraco no meio, para conferir o bitmap
        let mut values = vec![0.0; linhas as usize];
        for r in 0..linhas {
            values[r as usize] = r as f64 * 2.5;
        }
        let bitmap = bitmap_de(&presentes);

        let mut aba = aba_vazia("Dados");
        aba.linhas = linhas;
        aba.colunas_num = vec![ColunaNumBruta { col: 3, values: values.clone(), bitmap: bitmap.clone() }];
        let pasta = PastaBruta { a: 0, names: None, es: None, pr: None, mc: None, ex: None, fm: None, sheets: vec![aba] };
        let bytes = crate::fshz_write::gerar_fshz2(&pasta).unwrap();

        let lida = ler_fshz(&bytes).unwrap();
        let col = &lida.sheets[0].colunas_num[0];
        assert_eq!(col.col, 3);
        assert_eq!(col.bitmap, bitmap);
        for r in 0..linhas {
            if presentes[r as usize] {
                assert_eq!(col.values[r as usize], values[r as usize]);
            }
        }
    }

    #[test]
    fn coluna_texto_densa_faz_ida_e_volta_com_dicionario() {
        let linhas = 260u32;
        let presentes = vec![true; linhas as usize];
        let bitmap = bitmap_de(&presentes);
        let indices: Vec<i32> = (0..linhas as i32).map(|r| r % 3).collect();
        let dict = vec!["a".to_string(), "b".to_string(), "c".to_string()];

        let mut aba = aba_vazia("Texto");
        aba.linhas = linhas;
        aba.colunas_txt = vec![ColunaTxtBruta { col: 1, indices: indices.clone(), bitmap, dict: dict.clone() }];
        let pasta = PastaBruta { a: 0, names: None, es: None, pr: None, mc: None, ex: None, fm: None, sheets: vec![aba] };
        let bytes = crate::fshz_write::gerar_fshz2(&pasta).unwrap();

        let lida = ler_fshz(&bytes).unwrap();
        let col = &lida.sheets[0].colunas_txt[0];
        assert_eq!(col.dict, dict);
        assert_eq!(col.indices, indices);
    }

    #[test]
    fn coluna_formula_com_valor_calculado_faz_ida_e_volta() {
        let linhas = 260u32;
        let presentes = vec![true; linhas as usize];
        let bitmap = bitmap_de(&presentes);
        let slots: Vec<i32> = (0..linhas as i32).map(|_| 1i32).collect(); // padrão 0, tipo "número"
        let payload: Vec<f64> = (0..linhas).map(|r| r as f64).collect();

        let mut aba = aba_vazia("Formulas");
        aba.linhas = linhas;
        aba.padroes_formula = vec!["\u{1}r0;r-1\u{1}+1".to_string()];
        aba.colunas_formula = vec![ColunaFormulaBruta {
            col: 0,
            slots: slots.clone(),
            payload: payload.clone(),
            bitmap,
            dict: vec![],
            erros: vec![ErroBruto { code: "#DIV/0!".into(), detail: None }],
        }];
        let pasta = PastaBruta { a: 0, names: None, es: None, pr: None, mc: None, ex: None, fm: None, sheets: vec![aba] };
        let bytes = crate::fshz_write::gerar_fshz2(&pasta).unwrap();

        let lida = ler_fshz(&bytes).unwrap();
        let col = &lida.sheets[0].colunas_formula[0];
        assert_eq!(col.slots, slots);
        assert_eq!(col.payload, payload);
        assert_eq!(col.erros.len(), 1);
        assert_eq!(col.erros[0].code, "#DIV/0!");
        assert_eq!(lida.sheets[0].padroes_formula.len(), 1);
    }

    #[test]
    fn estilo_de_avulsa_e_de_literal_colunar_resolve_pela_tabela() {
        let mut aba = aba_vazia("Estilos");
        aba.avulsas = vec![CelulaAvulsaBruta {
            k: 0,
            v: "a".into(),
            estilo: Some(serde_json::json!({"negrito": true})),
        }];
        aba.estilos_literais = vec![(1, serde_json::json!({"negrito": true}))];
        aba.linhas = 1;
        let pasta = PastaBruta { a: 0, names: None, es: None, pr: None, mc: None, ex: None, fm: None, sheets: vec![aba] };
        let bytes = crate::fshz_write::gerar_fshz2(&pasta).unwrap();

        let lida = ler_fshz(&bytes).unwrap();
        let aba = &lida.sheets[0];
        let negrito = serde_json::json!({"negrito": true});
        assert_eq!(aba.avulsas[0].estilo.map(|i| &aba.estilos[i]), Some(&negrito));
        assert_eq!(aba.estilos_diretos.len(), 1);
        assert_eq!(aba.estilos_diretos[0].0, 1);
        assert_eq!(aba.estilos[aba.estilos_diretos[0].1], negrito);
        // O mesmo estilo nas duas células: uma entrada só na tabela.
        assert_eq!(aba.avulsas[0].estilo, Some(aba.estilos_diretos[0].1));
    }

    /// O GzDecoder nativo substitui a descompressão do Web Worker legado:
    /// preserva bytes arbitrários (Arrow, MessagePack), NUL e texto UTF-8.
    #[test]
    fn gunzip_nativo_substitui_worker_sem_corromper_bytes() {
        use std::io::Write;
        let mut entrada = Vec::with_capacity(128 * 1024);
        for i in 0..128 * 1024 {
            entrada.push((i % 251) as u8);
        }
        entrada.extend_from_slice("Gráficos – planilha 🚂\0\r\n".as_bytes());
        let mut encoder = flate2::write::GzEncoder::new(
            Vec::new(), flate2::Compression::default(),
        );
        encoder.write_all(&entrada).unwrap();
        let gzip = encoder.finish().unwrap();
        assert_eq!(gunzip(&gzip).unwrap(), entrada);
    }

    /// O checksum de gzip é obrigatório: dados corrompidos não podem virar
    /// uma pasta de trabalho parcialmente válida.
    #[test]
    fn gunzip_nativo_rejeita_checksum_invalido_ou_truncamento() {
        use std::io::Write;
        let mut encoder = flate2::write::GzEncoder::new(
            Vec::new(), flate2::Compression::default(),
        );
        encoder.write_all(b"FeatherSheet FSH3").unwrap();
        let bytes = encoder.finish().unwrap();
        assert_eq!(gunzip(&bytes).unwrap(), b"FeatherSheet FSH3");
        let mut adulterado = bytes.clone();
        let crc = adulterado.len() - 8;
        adulterado[crc] ^= 0x7f;
        assert!(gunzip(&adulterado).is_err(), "CRC alterado deve falhar");
        assert!(gunzip(&bytes[..bytes.len()-5]).is_err(),
            "Trailer gzip incompleto deve falhar");
    }

    #[test]
    fn arquivo_invalido_da_erro_em_vez_de_panic() {
        assert!(ler_fshz(b"lixo qualquer, nem gzip nem json").is_err());
    }
}
