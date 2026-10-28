//! Tabela dinâmica no `.xlsx` (meta da v0.1.0): grava a definição para o
//! Excel abrir o arquivo e reconhecer a dinâmica como dinâmica, não como
//! valores soltos.
//!
//! Para cada dinâmica (`sheet.dn` da aba de DESTINO):
//!
//! - `xl/pivotCache/pivotCacheDefinitionN.xml`: a origem
//!   (`cacheSource`/`worksheetSource`) e um `cacheField` por coluna dela,
//!   com o cabeçalho de nome. Os campos usados em Linhas, Colunas e Filtros
//!   levam os itens (`sharedItems`), lidos da origem pelo motor.
//! - `xl/pivotTables/pivotTableN.xml`: os campos, o layout, os subtotais, os
//!   totais gerais, a ordem dos itens, o "mostrar valores como", os filtros
//!   e a `location` (= `areaResultado`).
//! - as relações (livro → cache, aba → tabela, tabela → cache), o
//!   `<pivotCaches>` do workbook.xml e os tipos de conteúdo.
//!
//! **Sem `pivotCacheRecords`:** `saveData="0"` + `refreshOnLoad="1"` (e
//! `invalid="1"`), o Excel recalcula a dinâmica ao abrir — é o que o Google
//! Planilhas grava. O arquivo fica menor e não se copiam milhões de
//! registros da origem. Pelo mesmo motivo não vão `rowItems`/`colItems`: o
//! Excel os refaz na atualização. As células do resultado continuam
//! gravadas como sempre (`aba_xml`): é o que aparece antes de atualizar (e
//! no Modo de Exibição Protegido, que não atualiza).
//!
//! É o caminho inverso do leitor (o trecho das dinâmicas em
//! `xlsx_read/mod.rs`): tudo o que vai aqui volta igual por lá — os testes
//! de ida e volta em `tests/dinamica_xlsx.rs` conferem.
//!
//! Fica de fora:
//! - **Moda**: o Excel não tem; a medida vai como Soma.
//! - **Campos calculados** (colunas negativas): o campo sai da dinâmica.
//! - **Agrupamento** (faixas e partes de data, `fieldGroup`) e o `datas`
//!   antigo: o campo vai sem agrupar.
//! - **Filtro de relatório sem espaço acima da dinâmica**: o Excel desenha
//!   os filtros nas linhas de cima da `location` (uma por filtro, mais uma em
//!   branco). Sem essas linhas, o campo não vai para Filtros: os itens vão
//!   ocultos no campo fora dos eixos — o FeatherSheet lê o filtro de volta,
//!   o Excel mostra a dinâmica sem ele.
//! - O tema de cores da dinâmica: vai o estilo padrão do Excel.
//! - **Campo em Filtros e em Linhas ao mesmo tempo** (`paginaTambem`): o
//!   Excel não aceita o mesmo campo como página e linha. Vai só como linha,
//!   com os itens que o filtro tira ocultos (`h="1"`): a tabela mostra o
//!   mesmo; a célula do filtro em cima dela fica como valor solto.

use std::collections::HashSet;
use std::fmt::Write as _;

use feathersheet_core::celulas::number::to_js_string;
use feathersheet_core::celulas::refs::cell_name;
use feathersheet_core::celulas::values::{to_text, Value};
use feathersheet_core::dinamica::exibicao::{CampoBase, Eixo, Extras, ItemBase, MostrarComo, Ordenacao};
use feathersheet_core::dinamica::pivot::AgregacaoOp;
use feathersheet_core::planilha::workbook::Workbook;
use serde_json::Value as Json;

use crate::xlsx_write::escape_xml;

const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const NS_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_PKG_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";
/// Extensões do Excel 2010 (`x14`): o que o esquema de 2006 não tem.
const NS_X14: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";
/// `ext` do `x14:dataField` (classificação e % do total acumulado).
const EXT_CAMPO_DE_VALOR: &str = "{E15A36E0-9728-4e99-A89B-3F7291B0FE68}";
/// `ext` do `x14:pivotField` (repetir rótulos dos itens).
const EXT_CAMPO: &str = "{2946ED86-A175-432a-8AC1-64E0C546D7DE}";

const TIPO_CACHE: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml";
const TIPO_TABELA: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml";

/// `baseItem` especiais do "mostrar valores como": "(anterior)" e "(seguinte)".
const ITEM_ANTERIOR: u32 = 1_048_828;
const ITEM_SEGUINTE: u32 = 1_048_829;
/// O pseudo-campo Valores (−2 em 32 bits sem sinal), no `autoSortScope`.
const CAMPO_VALORES: u32 = 4_294_967_294;

/// Uma dinâmica pronta para o pacote. `n` numera os arquivos e é o `cacheId`.
pub(crate) struct ParteDinamica {
    n: usize,
    cache: String,
    tabela: String,
}

struct Medida {
    /// Índice do campo (coluna dentro da origem).
    campo: usize,
    op: AgregacaoOp,
    nome: Option<String>,
    mostrar: MostrarComo,
}

struct Filtro {
    campo: usize,
    /// Chaves do filtro do painel (texto da célula; `(Vazio)` = vazia).
    valores: HashSet<String>,
    excluir: bool,
}

impl Filtro {
    /// O item com esta chave fica fora da dinâmica? Lista vazia = "(Tudo)",
    /// como no painel (`avancado::opcoes`).
    fn oculta(&self, chave: &str) -> bool {
        !self.valores.is_empty() && self.valores.contains(chave) == self.excluir
    }
}

/// A dinâmica do JSON do painel, já em índices de campo do Excel (posição
/// dentro da origem) e sem o que o Excel não representa.
struct Plano {
    nome: Option<String>,
    folha: usize,
    aba: String,
    /// (r0, c0, r1, c1) da origem, com o cabeçalho na r0.
    origem: (u32, u32, u32, u32),
    /// (r0, c0, r1, c1) do resultado na aba de destino.
    area: (u32, u32, u32, u32),
    linhas: Vec<usize>,
    colunas: Vec<usize>,
    medidas: Vec<Medida>,
    ordem_linhas: Vec<Ordenacao>,
    ordem_colunas: Vec<Ordenacao>,
    filtros: Vec<Filtro>,
    tabular: bool,
    repetir: bool,
    subtotais: bool,
    recolhidos: HashSet<String>,
    totais_linhas: bool,
    totais_colunas: bool,
}

fn retangulo(v: &Json) -> Option<(u32, u32, u32, u32)> {
    let n = |k: &str| v[k].as_u64().and_then(|x| u32::try_from(x).ok());
    let (r0, c0, r1, c1) = (n("r0")?, n("c0")?, n("r1")?, n("c1")?);
    (r0 <= r1 && c0 <= c1).then_some((r0, c0, r1, c1))
}

/// Texto para valor de atributo: além do escape, a quebra de linha vira
/// referência (o XML troca quebra crua em atributo por espaço).
fn atributo(texto: &str) -> String {
    escape_xml(texto).replace('\n', "&#10;").replace('\r', "&#13;").replace('\t', "&#9;")
}

fn plano(pasta: &Workbook, destino: usize, meta: &Json) -> Option<Plano> {
    let origem = retangulo(&meta["fonte"]["area"])?;
    if origem.2 <= origem.0 {
        return None; // só o cabeçalho: nada para o Excel resumir
    }
    let nomes = pasta.nomes();
    let folha = match meta["fonte"]["aba"].as_str() {
        Some(nome) => nomes.iter().position(|n| n == nome)
            .or_else(|| nomes.iter().position(|n| n.eq_ignore_ascii_case(nome)))?,
        None => destino,
    };
    // Coluna absoluta da origem → índice do campo; negativa (campo
    // calculado) ou fora da origem → `None`.
    let campo = |v: &Json| -> Option<usize> {
        let c = v.as_i64()?;
        (c >= origem.1 as i64 && c <= origem.3 as i64).then(|| (c - origem.1 as i64) as usize)
    };
    // (posição na lista do painel, campo), só dos que o Excel representa.
    let sobra = |lista: &Json| -> Vec<(usize, usize)> {
        lista.as_array()
            .map(|a| a.iter().enumerate().filter_map(|(p, v)| Some((p, campo(v)?))).collect())
            .unwrap_or_default()
    };
    let linhas = sobra(&meta["linhas"]);
    let colunas = sobra(&meta["colunas"]);
    let valores: Vec<(usize, &Json)> = meta["valores"].as_array()?
        .iter().enumerate()
        .filter(|(_, v)| campo(&v["col"]).is_some())
        .collect();
    if (linhas.is_empty() && colunas.is_empty()) || valores.is_empty() {
        return None;
    }
    // O Excel não aceita o mesmo campo duas vezes nos eixos.
    let mut nos_eixos = HashSet::new();
    if !linhas.iter().chain(colunas.iter()).all(|(_, c)| nos_eixos.insert(*c)) {
        return None;
    }

    let extras: Extras = serde_json::from_value(meta["extras"].clone()).unwrap_or_default();
    let nova_posicao = |lista: &[(usize, usize)], antiga: usize| lista.iter().position(|(p, _)| *p == antiga);
    let nova_medida = |antiga: usize| valores.iter().position(|(p, _)| *p == antiga);
    let base = |c: &CampoBase| -> Option<CampoBase> {
        let lista = if c.eixo == Eixo::Linhas { &linhas } else { &colunas };
        Some(CampoBase { eixo: c.eixo, posicao: nova_posicao(lista, c.posicao)? })
    };
    // O campo base acompanha a saída dos campos calculados; se o próprio
    // campo base saiu, a medida volta ao normal.
    let seguir = |m: &MostrarComo| -> MostrarComo {
        use MostrarComo::*;
        let novo = match m {
            PercDe { campo, item } => base(campo).map(|campo| PercDe { campo, item: item.clone() }),
            DiferencaDe { campo, item } => base(campo).map(|campo| DiferencaDe { campo, item: item.clone() }),
            PercDiferencaDe { campo, item } => base(campo).map(|campo| PercDiferencaDe { campo, item: item.clone() }),
            TotalAcumuladoEm { campo } => base(campo).map(|campo| TotalAcumuladoEm { campo }),
            PercTotalAcumuladoEm { campo } => base(campo).map(|campo| PercTotalAcumuladoEm { campo }),
            ClassificarMenorMaior { campo } => base(campo).map(|campo| ClassificarMenorMaior { campo }),
            ClassificarMaiorMenor { campo } => base(campo).map(|campo| ClassificarMaiorMenor { campo }),
            outro => Some(outro.clone()),
        };
        novo.unwrap_or_default()
    };
    let medidas: Vec<Medida> = valores.iter().map(|(p, v)| Medida {
        campo: campo(&v["col"]).unwrap_or(0),
        op: AgregacaoOp::do_str(v.get("como").or_else(|| v.get("op")).and_then(Json::as_str).unwrap_or("soma")),
        nome: v["nome"].as_str().filter(|n| !n.trim().is_empty()).map(str::to_string),
        mostrar: seguir(extras.mostrar_como(*p)),
    }).collect();
    let ordem = |eixo: Eixo, lista: &[(usize, usize)]| -> Vec<Ordenacao> {
        lista.iter().map(|(p, _)| match extras.campo(eixo, *p).ordenacao {
            Ordenacao::PorValor { medida, crescente } => match nova_medida(medida) {
                Some(medida) => Ordenacao::PorValor { medida, crescente },
                None => Ordenacao::Padrao,
            },
            o => o,
        }).collect()
    };
    let mut filtros: Vec<Filtro> = Vec::new();
    for f in meta["filtros"].as_array().into_iter().flatten() {
        let Some(c) = campo(&f["col"]) else { continue };
        if filtros.iter().any(|x| x.campo == c) {
            continue;
        }
        filtros.push(Filtro {
            campo: c,
            valores: f["valores"].as_array().into_iter().flatten()
                .filter_map(|v| v.as_str().map(str::to_string)).collect(),
            excluir: f["excluir"].as_bool().unwrap_or(false),
        });
    }
    let area = retangulo(&meta["areaResultado"]).or_else(|| {
        let r = u32::try_from(meta["destino"]["row"].as_u64()?).ok()?;
        let c = u32::try_from(meta["destino"]["col"].as_u64()?).ok()?;
        Some((r, c, r, c))
    })?;
    Some(Plano {
        nome: meta["nome"].as_str().filter(|n| !n.trim().is_empty()).map(str::to_string),
        folha,
        aba: nomes.get(folha)?.clone(),
        origem,
        area,
        ordem_linhas: ordem(Eixo::Linhas, &linhas),
        ordem_colunas: ordem(Eixo::Colunas, &colunas),
        linhas: linhas.iter().map(|(_, c)| *c).collect(),
        colunas: colunas.iter().map(|(_, c)| *c).collect(),
        medidas,
        filtros,
        tabular: meta["layout"] != "compacto",
        // Mesmo padrão do painel (`avancado::opcoes_de_json`).
        repetir: meta["repetirRotulos"].as_bool().unwrap_or(true),
        subtotais: meta["subtotais"].as_bool().unwrap_or(false),
        recolhidos: meta["recolhidos"].as_array().into_iter().flatten()
            .filter_map(|v| v.as_str().map(str::to_string)).collect(),
        totais_linhas: extras.totais_gerais_linhas,
        totais_colunas: extras.totais_gerais_colunas,
    })
}

// ------------------------------------------------------------ itens

/// Um item do `sharedItems`, com o tipo que o Excel distingue.
enum Item {
    Vazio,
    Texto(String),
    Numero(f64),
    Logico(bool),
    Erro(&'static str),
}

/// Os itens de um campo, na ordem em que a dinâmica do FeatherSheet os
/// mostra (pela chave, como `dinamica_layout::ordenar` no padrão). O item
/// `i` do `pivotField` é o `sharedItems` `i`: posição e `x` coincidem, e o
/// `baseItem` não depende de qual dos dois o Excel quer dizer.
struct Itens {
    lista: Vec<Item>,
    /// A chave do filtro do painel de cada item.
    chaves: Vec<String>,
}

/// A chave que o filtro do painel compara (`parse_date_key` sem agrupar).
fn chave_do_filtro(v: &Value) -> String {
    let t = to_text(v);
    if t.trim().is_empty() { "(Vazio)".to_string() } else { t }
}

/// Só os erros que o Excel conhece entram no cache.
fn erro_excel(codigo: &str) -> &'static str {
    match codigo {
        "#NULL!" => "#NULL!",
        "#DIV/0!" => "#DIV/0!",
        "#VALUE!" => "#VALUE!",
        "#REF!" => "#REF!",
        "#NAME?" => "#NAME?",
        "#NUM!" => "#NUM!",
        _ => "#N/A",
    }
}

fn itens_do_campo(pasta: &Workbook, folha: usize, origem: (u32, u32, u32, u32), campo: usize) -> Itens {
    let col = origem.1 + campo as u32;
    let mut vistos: HashSet<String> = HashSet::new();
    let mut achados: Vec<(String, Item)> = Vec::new();
    for r in origem.0 + 1..=origem.2 {
        let v = pasta.valor(folha, r, col);
        // A identidade do item para o Excel: texto sem diferenciar maiúsculas.
        let (identidade, item) = match &v {
            Value::Empty => ("m".to_string(), Item::Vazio),
            Value::Text(t) => (format!("s{}", t.to_lowercase()), Item::Texto(t.clone())),
            Value::Number(x) if x.is_finite() => (format!("n{}", to_js_string(*x)), Item::Numero(*x)),
            Value::Number(_) => ("e#NUM!".to_string(), Item::Erro("#NUM!")),
            Value::Bool(b) => (format!("b{b}"), Item::Logico(*b)),
            Value::Error(e) => {
                let codigo = erro_excel(e.code.as_str());
                (format!("e{codigo}"), Item::Erro(codigo))
            }
        };
        if vistos.insert(identidade) {
            achados.push((chave_do_filtro(&v), item));
        }
    }
    achados.sort_by(|a, b| a.0.cmp(&b.0));
    let (chaves, lista) = achados.into_iter().unzip();
    Itens { lista, chaves }
}

/// `<sharedItems>` com os atributos de tipo coerentes com os itens (o Excel
/// confere `containsNumber`, `containsString`... contra o conteúdo).
fn itens_compartilhados(itens: &[Item]) -> String {
    let (mut texto, mut vazio, mut numero, mut logico, mut erro, mut longo) = (false, false, false, false, false, false);
    let mut inteiro = true;
    let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut corpo = String::new();
    for item in itens {
        match item {
            Item::Vazio => {
                vazio = true;
                corpo.push_str("<m/>");
            }
            Item::Texto(t) => {
                texto = true;
                longo |= t.chars().count() > 255;
                let _ = write!(corpo, "<s v=\"{}\"/>", atributo(t));
            }
            Item::Numero(x) => {
                numero = true;
                inteiro &= x.fract() == 0.0;
                min = min.min(*x);
                max = max.max(*x);
                let _ = write!(corpo, "<n v=\"{}\"/>", to_js_string(*x));
            }
            Item::Logico(b) => {
                logico = true;
                let _ = write!(corpo, "<b v=\"{}\"/>", u8::from(*b));
            }
            Item::Erro(c) => {
                erro = true;
                let _ = write!(corpo, "<e v=\"{}\"/>", atributo(c));
            }
        }
    }
    let mut attrs = String::new();
    if !(texto || vazio || logico || erro) {
        attrs.push_str(" containsSemiMixedTypes=\"0\"");
    }
    if !texto {
        attrs.push_str(" containsString=\"0\"");
    }
    if vazio {
        attrs.push_str(" containsBlank=\"1\"");
    }
    if [texto, numero, logico, erro].iter().filter(|&&x| x).count() > 1 {
        attrs.push_str(" containsMixedTypes=\"1\"");
    }
    if numero {
        attrs.push_str(" containsNumber=\"1\"");
        if inteiro {
            attrs.push_str(" containsInteger=\"1\"");
        }
        let _ = write!(attrs, " minValue=\"{}\" maxValue=\"{}\"", to_js_string(min), to_js_string(max));
    }
    if longo {
        attrs.push_str(" longText=\"1\"");
    }
    format!("<sharedItems{attrs} count=\"{}\">{corpo}</sharedItems>", itens.len())
}

/// O nome de cada campo: o cabeçalho da origem. O Excel exige nomes não
/// vazios e únicos (sem diferenciar maiúsculas): vazio vira "Coluna N" (o
/// mesmo da dinâmica do FeatherSheet) e repetido ganha um número.
fn nomes_dos_campos(pasta: &Workbook, folha: usize, origem: (u32, u32, u32, u32)) -> Vec<String> {
    let mut usados = HashSet::new();
    (origem.1..=origem.3).map(|c| {
        let t = to_text(&pasta.valor(folha, origem.0, c));
        let base = if t.trim().is_empty() { format!("Coluna {}", c + 1) } else { t };
        let mut nome = base.clone();
        let mut k = 2;
        while !usados.insert(nome.to_lowercase()) {
            nome = format!("{base}{k}");
            k += 1;
        }
        nome
    }).collect()
}

// ------------------------------------------------------------ XML

fn cache_xml(p: &Plano, nomes: &[String], itens: &[Option<Itens>]) -> String {
    let (r0, c0, r1, c1) = p.origem;
    let mut x = format!(
        "{XML_DECL}<pivotCacheDefinition xmlns=\"{NS}\" xmlns:r=\"{NS_REL}\" invalid=\"1\" saveData=\"0\" refreshOnLoad=\"1\" createdVersion=\"6\" refreshedVersion=\"6\" minRefreshableVersion=\"3\">"
    );
    let _ = write!(
        x,
        "<cacheSource type=\"worksheet\"><worksheetSource ref=\"{}:{}\" sheet=\"{}\"/></cacheSource><cacheFields count=\"{}\">",
        cell_name(r0, c0), cell_name(r1, c1), atributo(&p.aba), nomes.len()
    );
    for (i, nome) in nomes.iter().enumerate() {
        let compartilhados = match itens.get(i).and_then(Option::as_ref) {
            Some(it) => itens_compartilhados(&it.lista),
            None => "<sharedItems/>".to_string(),
        };
        let _ = write!(x, "<cacheField name=\"{}\" numFmtId=\"0\">{compartilhados}</cacheField>", atributo(nome));
    }
    x.push_str("</cacheFields></pivotCacheDefinition>");
    x
}

/// `showDataAs` (2006), `baseField`, `baseItem` e `pivotShowAs` (x14) de
/// uma medida. Sem campo base válido, a medida vai normal.
fn mostrar_como_excel(
    m: &MostrarComo,
    p: &Plano,
    itens: &[Option<Itens>],
) -> (Option<&'static str>, usize, u32, Option<&'static str>) {
    let normal: (Option<&'static str>, usize, u32, Option<&'static str>) = (None, 0, 0, None);
    let campo_de = |c: &CampoBase| -> Option<usize> {
        let lista = if c.eixo == Eixo::Linhas { &p.linhas } else { &p.colunas };
        lista.get(c.posicao).copied()
    };
    let item_de = |campo: usize, item: &ItemBase| -> Option<u32> {
        match item {
            ItemBase::Anterior => Some(ITEM_ANTERIOR),
            ItemBase::Seguinte => Some(ITEM_SEGUINTE),
            ItemBase::Item { nome } => itens.get(campo)?.as_ref()?
                .chaves.iter().position(|c| c == nome).and_then(|i| u32::try_from(i).ok()),
        }
    };
    let com_item = |tipo: &'static str, c: &CampoBase, item: &ItemBase| {
        match campo_de(c).and_then(|f| Some((f, item_de(f, item)?))) {
            Some((f, i)) => (Some(tipo), f, i, None),
            None => normal,
        }
    };
    let com_campo = |tipo: Option<&'static str>, x14: Option<&'static str>, c: &CampoBase| match campo_de(c) {
        Some(f) => (tipo, f, 0, x14),
        None => normal,
    };
    match m {
        MostrarComo::Normal => normal,
        MostrarComo::PercTotalGeral => (Some("percentOfTotal"), 0, 0, None),
        MostrarComo::PercTotalColuna => (Some("percentOfCol"), 0, 0, None),
        MostrarComo::PercTotalLinha => (Some("percentOfRow"), 0, 0, None),
        MostrarComo::Indice => (Some("index"), 0, 0, None),
        MostrarComo::PercDe { campo, item } => com_item("percent", campo, item),
        MostrarComo::DiferencaDe { campo, item } => com_item("difference", campo, item),
        MostrarComo::PercDiferencaDe { campo, item } => com_item("percentDiff", campo, item),
        MostrarComo::TotalAcumuladoEm { campo } => com_campo(Some("runTotal"), None, campo),
        MostrarComo::PercTotalAcumuladoEm { campo } => com_campo(None, Some("percentOfRunningTotal"), campo),
        MostrarComo::ClassificarMenorMaior { campo } => com_campo(None, Some("rankAscending"), campo),
        MostrarComo::ClassificarMaiorMenor { campo } => com_campo(None, Some("rankDescending"), campo),
    }
}

fn tabela_xml(p: &Plano, n: usize, nome: &str, nomes: &[String], itens: &[Option<Itens>]) -> String {
    let (n_lin, n_col, n_med) = (p.linhas.len(), p.colunas.len(), p.medidas.len());
    // Filtros de campo fora dos eixos = filtros de relatório (`axisPage`),
    // se houver as linhas que o Excel usa acima da dinâmica.
    let candidatas: Vec<usize> = p.filtros.iter().map(|f| f.campo)
        .filter(|c| !p.linhas.contains(c) && !p.colunas.contains(c))
        .collect();
    let paginas: Vec<usize> = if (p.area.0 as usize) > candidatas.len() { candidatas } else { Vec::new() };

    let mut x = format!(
        "{XML_DECL}<pivotTableDefinition xmlns=\"{NS}\" name=\"{}\" cacheId=\"{n}\" applyNumberFormats=\"0\" applyBorderFormats=\"0\" applyFontFormats=\"0\" applyPatternFormats=\"0\" applyAlignmentFormats=\"0\" applyWidthHeightFormats=\"1\" dataCaption=\"Valores\" updatedVersion=\"6\" minRefreshableVersion=\"3\" useAutoFormatting=\"1\" itemPrintTitles=\"1\" createdVersion=\"6\" indent=\"0\"",
        atributo(nome)
    );
    // O leitor: `compact="0"` = tabular; o resto, compacto.
    x.push_str(if p.tabular { " compact=\"0\" compactData=\"0\"" } else { " outline=\"1\" outlineData=\"1\"" });
    x.push_str(" multipleFieldFilters=\"0\"");
    if !p.totais_linhas {
        x.push_str(" rowGrandTotals=\"0\"");
    }
    if !p.totais_colunas {
        x.push_str(" colGrandTotals=\"0\"");
    }
    x.push('>');

    // `location`: as primeiras linhas/coluna como o Excel as conta. Sem campo
    // em Colunas, o cabeçalho é a linha 0 (com uma medida o Excel grava 1);
    // com campo em Colunas são duas linhas de cabeçalho, mais uma para os
    // nomes das medidas quando há mais de uma.
    let (primeira_cab, primeiro_dado) = if n_col == 0 {
        (usize::from(n_med <= 1), 1)
    } else {
        (1, 1 + n_col + usize::from(n_med > 1))
    };
    let primeira_col = if p.tabular { n_lin.max(1) } else { 1 };
    let (r0, c0, r1, c1) = p.area;
    let referencia = if (r0, c0) == (r1, c1) {
        cell_name(r0, c0)
    } else {
        format!("{}:{}", cell_name(r0, c0), cell_name(r1, c1))
    };
    let _ = write!(
        x,
        "<location ref=\"{referencia}\" firstHeaderRow=\"{primeira_cab}\" firstDataRow=\"{primeiro_dado}\" firstDataCol=\"{primeira_col}\""
    );
    if !paginas.is_empty() {
        let _ = write!(x, " rowPageCount=\"{}\" colPageCount=\"1\"", paginas.len());
    }
    x.push_str("/>");

    let _ = write!(x, "<pivotFields count=\"{}\">", nomes.len());
    for campo in 0..nomes.len() {
        let pos_linha = p.linhas.iter().position(|&c| c == campo);
        let pos_coluna = p.colunas.iter().position(|&c| c == campo);
        let pagina = paginas.contains(&campo);
        x.push_str("<pivotField");
        if pos_linha.is_some() {
            x.push_str(" axis=\"axisRow\"");
        } else if pos_coluna.is_some() {
            x.push_str(" axis=\"axisCol\"");
        } else if pagina {
            x.push_str(" axis=\"axisPage\"");
        }
        if p.medidas.iter().any(|m| m.campo == campo) {
            x.push_str(" dataField=\"1\"");
        }
        if p.tabular {
            x.push_str(" compact=\"0\" outline=\"0\"");
        }
        x.push_str(" showAll=\"0\"");
        // Subtotal só nos campos de Linhas, quando a dinâmica pede (o leitor
        // confere os de fora); a dinâmica daqui não subtotaliza colunas.
        let subtotal = match (pos_linha, pos_coluna) {
            (Some(_), _) => p.subtotais,
            (None, Some(_)) => false,
            _ => true,
        };
        if !subtotal {
            x.push_str(" defaultSubtotal=\"0\"");
        }
        let ordem = pos_linha.and_then(|i| p.ordem_linhas.get(i))
            .or_else(|| pos_coluna.and_then(|i| p.ordem_colunas.get(i)))
            .copied()
            .unwrap_or_default();
        match ordem {
            Ordenacao::Crescente | Ordenacao::PorValor { crescente: true, .. } => x.push_str(" sortType=\"ascending\""),
            Ordenacao::Decrescente | Ordenacao::PorValor { crescente: false, .. } => x.push_str(" sortType=\"descending\""),
            Ordenacao::Padrao => {}
        }
        let lista = itens.get(campo).and_then(Option::as_ref);
        // Itens ocultos pelo filtro; se o filtro esconderia todos, o Excel
        // recusa (precisa de ao menos um visível) e nada se oculta.
        let filtro = p.filtros.iter().find(|f| f.campo == campo);
        let ocultos: Vec<bool> = match (lista, filtro) {
            (Some(l), Some(f)) => {
                let o: Vec<bool> = l.chaves.iter().map(|c| f.oculta(c)).collect();
                if o.iter().all(|&h| h) { vec![false; o.len()] } else { o }
            }
            (Some(l), None) => vec![false; l.chaves.len()],
            _ => Vec::new(),
        };
        if pagina && ocultos.iter().any(|&h| h) {
            x.push_str(" multipleItemSelectionAllowed=\"1\"");
        }
        let repetir = pos_linha.is_some() && p.tabular && p.repetir;
        let por_valor = match ordem {
            Ordenacao::PorValor { medida, .. } => Some(medida),
            _ => None,
        };
        if lista.is_none() && por_valor.is_none() && !repetir {
            x.push_str("/>");
            continue;
        }
        x.push('>');
        if let Some(l) = lista {
            // Recolher (`sd="0"`) só no campo de fora: o caminho do painel
            // nesse nível é a própria chave do item.
            let recolher = pos_linha == Some(0) && n_lin > 1;
            let _ = write!(x, "<items count=\"{}\">", l.chaves.len() + usize::from(subtotal));
            for (i, chave) in l.chaves.iter().enumerate() {
                x.push_str("<item");
                if ocultos.get(i).copied().unwrap_or(false) {
                    x.push_str(" h=\"1\"");
                }
                if recolher && p.recolhidos.contains(chave) {
                    x.push_str(" sd=\"0\"");
                }
                let _ = write!(x, " x=\"{i}\"/>");
            }
            if subtotal {
                x.push_str("<item t=\"default\"/>");
            }
            x.push_str("</items>");
        }
        if let Some(medida) = por_valor {
            let _ = write!(
                x,
                "<autoSortScope><pivotArea dataOnly=\"0\" outline=\"0\" fieldPosition=\"0\"><references count=\"1\"><reference field=\"{CAMPO_VALORES}\" count=\"1\" selected=\"0\"><x v=\"{medida}\"/></reference></references></pivotArea></autoSortScope>"
            );
        }
        if repetir {
            let _ = write!(
                x,
                "<extLst><ext uri=\"{EXT_CAMPO}\" xmlns:x14=\"{NS_X14}\"><x14:pivotField fillDownLabels=\"1\"/></ext></extLst>"
            );
        }
        x.push_str("</pivotField>");
    }
    x.push_str("</pivotFields>");

    if n_lin > 0 {
        let _ = write!(x, "<rowFields count=\"{n_lin}\">");
        for c in &p.linhas {
            let _ = write!(x, "<field x=\"{c}\"/>");
        }
        x.push_str("</rowFields>");
    }
    // Mais de uma medida: o pseudo-campo Valores (−2) entra em Colunas.
    let mut em_colunas: Vec<i64> = p.colunas.iter().map(|&c| c as i64).collect();
    if n_med > 1 {
        em_colunas.push(-2);
    }
    if !em_colunas.is_empty() {
        let _ = write!(x, "<colFields count=\"{}\">", em_colunas.len());
        for c in &em_colunas {
            let _ = write!(x, "<field x=\"{c}\"/>");
        }
        x.push_str("</colFields>");
    }
    if !paginas.is_empty() {
        let _ = write!(x, "<pageFields count=\"{}\">", paginas.len());
        for c in &paginas {
            let _ = write!(x, "<pageField fld=\"{c}\" hier=\"-1\"/>");
        }
        x.push_str("</pageFields>");
    }

    let _ = write!(x, "<dataFields count=\"{n_med}\">");
    // O nome da medida não pode repetir o de um campo nem o de outra medida:
    // o Excel resolve com um espaço no fim, e aqui também.
    let mut usados: HashSet<String> = nomes.iter().map(|n| n.to_lowercase()).collect();
    for m in &p.medidas {
        x.push_str("<dataField");
        if let Some(nome) = &m.nome {
            let mut nome = nome.clone();
            while !usados.insert(nome.to_lowercase()) {
                nome.push(' ');
            }
            let _ = write!(x, " name=\"{}\"", atributo(&nome));
        }
        let _ = write!(x, " fld=\"{}\"", m.campo);
        // Moda não existe no Excel: vai como soma (sem `subtotal`).
        let subtotal = match m.op {
            AgregacaoOp::Soma | AgregacaoOp::Moda => None,
            AgregacaoOp::Contagem => Some("count"),
            AgregacaoOp::Media => Some("average"),
            AgregacaoOp::Min => Some("min"),
            AgregacaoOp::Max => Some("max"),
            AgregacaoOp::DesvioPadrao => Some("stdDev"),
        };
        if let Some(s) = subtotal {
            let _ = write!(x, " subtotal=\"{s}\"");
        }
        let (mostrar, base_campo, base_item, x14) = mostrar_como_excel(&m.mostrar, p, itens);
        if let Some(s) = mostrar {
            let _ = write!(x, " showDataAs=\"{s}\"");
        }
        let _ = write!(x, " baseField=\"{base_campo}\" baseItem=\"{base_item}\"");
        if m.mostrar.formato_sugerido() == Some("0,00%") {
            x.push_str(" numFmtId=\"10\""); // 0.00%, formato embutido
        }
        match x14 {
            Some(s) => {
                let _ = write!(
                    x,
                    "><extLst><ext uri=\"{EXT_CAMPO_DE_VALOR}\" xmlns:x14=\"{NS_X14}\"><x14:dataField pivotShowAs=\"{s}\"/></ext></extLst></dataField>"
                );
            }
            None => x.push_str("/>"),
        }
    }
    x.push_str("</dataFields>");
    x.push_str("<pivotTableStyleInfo name=\"PivotStyleLight16\" showRowHeaders=\"1\" showColHeaders=\"1\" showRowStripes=\"0\" showColStripes=\"0\" showLastColumn=\"1\"/>");
    x.push_str("</pivotTableDefinition>");
    x
}

// ------------------------------------------------------------ pacote

/// As dinâmicas cujo DESTINO é a aba `indice`. `contador` numera as partes
/// na pasta inteira (o `cacheId` é o mesmo número). Definição que o Excel
/// não representa (origem sumida, só campos calculados...) fica de fora:
/// a aba continua com as células do resultado, como antes.
pub(crate) fn da_aba(pasta: &Workbook, indice: usize, contador: &mut usize) -> Vec<ParteDinamica> {
    let Some(raw) = pasta.aba(indice).and_then(|a| a.sheet.dn.as_deref()) else { return Vec::new() };
    let Ok(lista) = serde_json::from_str::<Vec<Json>>(raw) else { return Vec::new() };
    let mut nomes_na_aba: HashSet<String> = HashSet::new();
    let mut saida = Vec::new();
    for meta in &lista {
        let Some(p) = plano(pasta, indice, meta) else { continue };
        *contador += 1;
        let n = *contador;
        // O nome da dinâmica é único na aba.
        let base = p.nome.clone().unwrap_or_else(|| format!("Tabela dinâmica{n}"));
        let mut nome = base.clone();
        let mut k = 2;
        while !nomes_na_aba.insert(nome.to_lowercase()) {
            nome = format!("{base} {k}");
            k += 1;
        }
        let nomes = nomes_dos_campos(pasta, p.folha, p.origem);
        let mut itens: Vec<Option<Itens>> = (0..nomes.len()).map(|_| None).collect();
        let enumerados = p.linhas.iter().chain(&p.colunas).copied().chain(p.filtros.iter().map(|f| f.campo));
        for campo in enumerados {
            if itens.get(campo).is_some_and(Option::is_none) {
                itens[campo] = Some(itens_do_campo(pasta, p.folha, p.origem, campo));
            }
        }
        saida.push(ParteDinamica {
            n,
            cache: cache_xml(&p, &nomes, &itens),
            tabela: tabela_xml(&p, n, &nome, &nomes, &itens),
        });
    }
    saida
}

/// A relação da aba para a tabela dinâmica (a aba não cita a dinâmica no
/// XML dela: só nas relações).
pub(crate) fn relacao_da_aba(p: &ParteDinamica) -> String {
    format!(
        "<Relationship Id=\"rIdDinamica{n}\" Type=\"{NS_REL}/pivotTable\" Target=\"../pivotTables/pivotTable{n}.xml\"/>",
        n = p.n
    )
}

/// `<pivotCaches>` do workbook.xml (vazio sem dinâmica).
pub(crate) fn caches_do_livro(partes: &[ParteDinamica]) -> String {
    if partes.is_empty() {
        return String::new();
    }
    let itens: String = partes.iter()
        .map(|p| format!("<pivotCache cacheId=\"{n}\" r:id=\"rIdCacheDinamica{n}\"/>", n = p.n))
        .collect();
    format!("<pivotCaches>{itens}</pivotCaches>")
}

/// As relações do livro para cada cache.
pub(crate) fn relacoes_do_livro(partes: &[ParteDinamica]) -> String {
    partes.iter().map(|p| format!(
        "<Relationship Id=\"rIdCacheDinamica{n}\" Type=\"{NS_REL}/pivotCacheDefinition\" Target=\"pivotCache/pivotCacheDefinition{n}.xml\"/>",
        n = p.n
    )).collect()
}

pub(crate) fn tipos_de_conteudo(partes: &[ParteDinamica]) -> String {
    partes.iter().map(|p| format!(
        "<Override PartName=\"/xl/pivotCache/pivotCacheDefinition{n}.xml\" ContentType=\"{TIPO_CACHE}\"/><Override PartName=\"/xl/pivotTables/pivotTable{n}.xml\" ContentType=\"{TIPO_TABELA}\"/>",
        n = p.n
    )).collect()
}

/// Os arquivos das dinâmicas: cache, tabela e a relação tabela → cache.
pub(crate) fn arquivos(partes: Vec<ParteDinamica>) -> Vec<(String, Vec<u8>)> {
    let mut saida = Vec::with_capacity(partes.len() * 3);
    for p in partes {
        let n = p.n;
        saida.push((format!("xl/pivotCache/pivotCacheDefinition{n}.xml"), p.cache.into_bytes()));
        saida.push((format!("xl/pivotTables/pivotTable{n}.xml"), p.tabela.into_bytes()));
        saida.push((
            format!("xl/pivotTables/_rels/pivotTable{n}.xml.rels"),
            format!(
                "{XML_DECL}<Relationships xmlns=\"{NS_PKG_REL}\"><Relationship Id=\"rId1\" Type=\"{NS_REL}/pivotCacheDefinition\" Target=\"../pivotCache/pivotCacheDefinition{n}.xml\"/></Relationships>"
            ).into_bytes(),
        ));
    }
    saida
}
