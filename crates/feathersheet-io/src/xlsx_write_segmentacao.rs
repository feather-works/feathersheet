//! Segmentação de dados no `.xlsx` (#145): o caminho inverso do leitor (`xlsx_read/segmentacoes.rs`). Pelo
//! MS-XLSX (extensões do Excel 2010), cada segmentação são quatro coisas:
//!
//! - `xl/slicerCaches/slicerCacheK.xml`: o campo (`sourceName`), a dinâmica que ele filtra e os itens, cada um
//!   pela posição no `sharedItems` do cache da dinâmica (`x`), escolhido com `s="1"`;
//! - `xl/slicers/slicerN.xml` da aba: o cartão (nome, legenda), ligado ao cache pelo nome;
//! - um `graphicFrame` no desenho da aba (`sle:slicer`, dentro de `mc:AlternateContent`) que põe o cartão na tela;
//! - o nome definido `Slicer_<campo>` (`#N/A`), o `x14:slicerCaches` do livro e o `x14:slicerList` da aba.
//!
//! A escolha também já vai como filtro da própria dinâmica (itens ocultos, `xlsx_write_dinamica`): o resultado
//! que o Excel mostra é o mesmo, com ou sem a segmentação.
// shortcut: a segmentação cita só a dinâmica dela. As ligadas (Conexões de relatório, `liga`) vão com o filtro,
// mas sem a corrente: no Excel, segmentação de várias dinâmicas exige que elas dividam o mesmo cache, e cada
// dinâmica daqui grava o seu. Juntar os caches de mesma origem quando alguém pedir a corrente no Excel.
// shortcut: a linha do tempo (`linhasTempo`, x15:timeline) ainda não vai para o `.xlsx`.

use std::collections::HashSet;
use std::fmt::Write as _;

use crate::xlsx_write::escape_xml;
use crate::xlsx_write_dinamica::ParteDinamica;

const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const NS_X14: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";
const NS_MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const NS_A14: &str = "http://schemas.microsoft.com/office/drawing/2010/main";
const NS_SLE: &str = "http://schemas.microsoft.com/office/drawing/2010/slicer";
const REL_CACHE: &str = "http://schemas.microsoft.com/office/2007/relationships/slicerCache";
const REL_SEGMENTACAO: &str = "http://schemas.microsoft.com/office/2007/relationships/slicer";
const TIPO_CACHE: &str = "application/vnd.ms-excel.slicerCache+xml";
const TIPO_SEGMENTACAO: &str = "application/vnd.ms-excel.slicer+xml";
const EXT_LIVRO: &str = "{BBE1A952-AA13-448e-AADC-164F8A28A991}";
const EXT_ABA: &str = "{A8765BA9-456A-4dab-B4F3-ACF838C121DE}";
const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// Uma segmentação de uma dinâmica: o nome do campo e, item a item do cache, se está escolhido.
pub(crate) struct Segmentacao {
    pub campo: String,
    pub escolhidos: Vec<bool>,
}

/// As segmentações de uma aba, prontas para o pacote.
pub(crate) struct DaAba {
    /// O `xl/slicers/slicerN.xml`.
    pub xml: String,
    /// As âncoras para o desenho da aba (vão antes de `</xdr:wsDr>`).
    pub ancoras: String,
}

/// Os caches de todas as abas: (nome definido, XML), na ordem do número do arquivo.
#[derive(Default)]
pub(crate) struct Livro {
    caches: Vec<(String, String)>,
    /// Números das abas com `slicerN.xml`.
    abas: Vec<usize>,
    usados: HashSet<String>,
}

/// O nome único na pasta (sem diferenciar maiúsculas), como o Excel: "Campo", "Campo 1"…
fn unico(usados: &mut HashSet<String>, base: &str, separador: &str) -> String {
    let mut nome = base.to_string();
    let mut k = 1;
    while !usados.insert(nome.to_lowercase()) {
        nome = format!("{base}{separador}{k}");
        k += 1;
    }
    nome
}

/// `Slicer_<campo>` só com o que um nome definido aceita.
fn nome_do_cache(campo: &str) -> String {
    let limpo: String = campo.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect();
    format!("Slicer_{limpo}")
}

impl Livro {
    /// As segmentações das dinâmicas desta aba (`aba` a partir de 1 = `sheetId`/número do arquivo).
    pub(crate) fn da_aba(&mut self, partes: &[ParteDinamica], aba: usize) -> Option<DaAba> {
        let mut cartoes = String::new();
        let mut ancoras = String::new();
        let mut k = 0;
        for p in partes {
            for s in &p.segmentacoes {
                let nome = unico(&mut self.usados, &s.campo, " ");
                let cache = unico(&mut self.usados, &nome_do_cache(&s.campo), "");
                let itens: String = s.escolhidos.iter().enumerate()
                    .map(|(x, &sim)| if sim { format!("<i x=\"{x}\" s=\"1\"/>") } else { format!("<i x=\"{x}\"/>") })
                    .collect();
                self.caches.push((cache.clone(), format!(
                    "{XML_DECL}<slicerCacheDefinition xmlns=\"{NS_X14}\" xmlns:mc=\"{NS_MC}\" mc:Ignorable=\"x\" xmlns:x=\"{NS}\" name=\"{c}\" sourceName=\"{campo}\"><pivotTables><pivotTable tabId=\"{aba}\" name=\"{dinamica}\"/></pivotTables><data><tabular pivotCacheId=\"{id}\"><items count=\"{n}\">{itens}</items></tabular></data></slicerCacheDefinition>",
                    c = escape_xml(&cache), campo = escape_xml(&s.campo), dinamica = escape_xml(&p.nome), id = p.n,
                    n = s.escolhidos.len(),
                )));
                let _ = write!(cartoes, "<slicer name=\"{n}\" cache=\"{c}\" caption=\"{t}\" rowHeight=\"241300\"/>",
                    n = escape_xml(&nome), c = escape_xml(&cache), t = escape_xml(&s.campo));
                // À direita do resultado, lado a lado: 3 colunas e 10 linhas cada, como o cartão do app.
                let (r0, _, _, c1) = p.area;
                let c = c1 + 2 + 4 * k;
                let ponto = |c: u32, r: u32| format!("<xdr:col>{c}</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>{r}</xdr:row><xdr:rowOff>0</xdr:rowOff>");
                let id = 200_000 + self.caches.len();
                let n = escape_xml(&nome);
                let _ = write!(ancoras,
                    "<xdr:twoCellAnchor editAs=\"oneCell\"><xdr:from>{de}</xdr:from><xdr:to>{ate}</xdr:to><mc:AlternateContent xmlns:mc=\"{NS_MC}\" xmlns:a14=\"{NS_A14}\"><mc:Choice Requires=\"a14\"><xdr:graphicFrame macro=\"\"><xdr:nvGraphicFramePr><xdr:cNvPr id=\"{id}\" name=\"{n}\"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/></xdr:xfrm><a:graphic><a:graphicData uri=\"{NS_SLE}\"><sle:slicer xmlns:sle=\"{NS_SLE}\" name=\"{n}\"/></a:graphicData></a:graphic></xdr:graphicFrame></mc:Choice><mc:Fallback><xdr:sp macro=\"\" textlink=\"\"><xdr:nvSpPr><xdr:cNvPr id=\"{id}\" name=\"{n}\"/><xdr:cNvSpPr><a:spLocks noTextEdit=\"1\"/></xdr:cNvSpPr></xdr:nvSpPr><xdr:spPr><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></xdr:spPr><xdr:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Slicer: {n}</a:t></a:r></a:p></xdr:txBody></xdr:sp></mc:Fallback></mc:AlternateContent><xdr:clientData/></xdr:twoCellAnchor>",
                    de = ponto(c, r0), ate = ponto(c + 3, r0 + 10),
                );
                k += 1;
            }
        }
        if k == 0 { return None; }
        self.abas.push(aba);
        Some(DaAba {
            xml: format!("{XML_DECL}<slicers xmlns=\"{NS_X14}\" xmlns:mc=\"{NS_MC}\" mc:Ignorable=\"x\" xmlns:x=\"{NS}\">{cartoes}</slicers>"),
            ancoras,
        })
    }

    /// A relação da aba para o `slicerN.xml`.
    pub(crate) fn relacao_da_aba(aba: usize) -> String {
        format!("<Relationship Id=\"rIdSegmentacoes\" Type=\"{REL_SEGMENTACAO}\" Target=\"../slicers/slicer{aba}.xml\"/>")
    }

    /// O `<ext>` do `extLst` da aba.
    pub(crate) fn ext_da_aba() -> String {
        format!("<ext uri=\"{EXT_ABA}\" xmlns:x14=\"{NS_X14}\"><x14:slicerList><x14:slicer r:id=\"rIdSegmentacoes\"/></x14:slicerList></ext>")
    }

    /// Os `definedName` dos caches (o Excel cria um `#N/A` para cada).
    pub(crate) fn nomes_definidos(&self) -> String {
        self.caches.iter().map(|(nome, _)| format!("<definedName name=\"{}\">#N/A</definedName>", escape_xml(nome))).collect()
    }

    /// O `extLst` do workbook.xml (último filho do CT_Workbook); vazio sem segmentação.
    pub(crate) fn ext_do_livro(&self) -> String {
        if self.caches.is_empty() { return String::new(); }
        let itens: String = (1..=self.caches.len()).map(|k| format!("<x14:slicerCache r:id=\"rIdSegmentacao{k}\"/>")).collect();
        format!("<extLst><ext uri=\"{EXT_LIVRO}\" xmlns:x14=\"{NS_X14}\"><x14:slicerCaches>{itens}</x14:slicerCaches></ext></extLst>")
    }

    pub(crate) fn relacoes_do_livro(&self) -> String {
        (1..=self.caches.len()).map(|k| format!(
            "<Relationship Id=\"rIdSegmentacao{k}\" Type=\"{REL_CACHE}\" Target=\"slicerCaches/slicerCache{k}.xml\"/>"
        )).collect()
    }

    pub(crate) fn tipos_de_conteudo(&self) -> String {
        let caches = (1..=self.caches.len())
            .map(|k| format!("<Override PartName=\"/xl/slicerCaches/slicerCache{k}.xml\" ContentType=\"{TIPO_CACHE}\"/>"));
        let abas = self.abas.iter()
            .map(|a| format!("<Override PartName=\"/xl/slicers/slicer{a}.xml\" ContentType=\"{TIPO_SEGMENTACAO}\"/>"));
        caches.chain(abas).collect()
    }

    pub(crate) fn arquivos(self) -> Vec<(String, Vec<u8>)> {
        self.caches.into_iter().enumerate()
            .map(|(k, (_, xml))| (format!("xl/slicerCaches/slicerCache{}.xml", k + 1), xml.into_bytes()))
            .collect()
    }
}
