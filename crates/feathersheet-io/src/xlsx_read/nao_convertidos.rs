//! O que um `.xlsx`/`.xlsm` tem e o FeatherSheet ainda não converte (ActiveX, segmentação de
//! dados…), para o F12 mostrar por aba. Só lê a lista do zip e os `.rels`; não monta
//! planilha nenhuma. Minigráficos saíram da lista: são convertidos (`minigraficos.rs`).
use super::{entry_text, sheet, xml};
use std::io::Cursor;
use zip::ZipArchive;

/// Um tipo de item não convertido: a aba (`None` = do arquivo inteiro), o nome em português
/// (a interface traduz) e quantos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NaoConvertido {
    pub aba: Option<String>,
    pub o_que: &'static str,
    pub quantos: usize,
}

/// Pelo caminho do alvo de um `.rels` da aba.
fn pelo_alvo(alvo: &str) -> Option<&'static str> {
    let a = alvo.to_ascii_lowercase();
    Some(if a.contains("activex/") { "Controle ActiveX" }
    else if a.contains("slicers/") { "Segmentação de dados" }
    else if a.contains("timelines/") { "Linha do tempo (filtro de datas)" }
    else if a.contains("embeddings/") { "Objeto incorporado (OLE)" }
    else { return None })
}

/// Do arquivo inteiro, pelo nome da parte no zip.
fn pela_parte(nome: &str) -> Option<&'static str> {
    let n = nome.to_ascii_lowercase();
    Some(if n.starts_with("xl/chartsheets/sheet") && n.ends_with(".xml") { "Planilha de gráfico" }
    else if n.starts_with("xl/macrosheets/") && n.ends_with(".xml") { "Folha de macro do Excel 4 (XLM)" }
    else if n.starts_with("xl/dialogsheets/") && n.ends_with(".xml") { "Folha de diálogo do Excel 5" }
    else if n.starts_with("xl/model/") { "Modelo de dados (Power Pivot)" }
    else if n == "xl/connections.xml" { "Conexão de dados externos" }
    else if n.starts_with("xl/querytables/") && n.ends_with(".xml") { "Consulta de tabela (Power Query)" }
    // Excel 365: a conversa (respostas, autores) mora aqui; o app junta as respostas numa anotação só, e é assim que grava.
    else if n.starts_with("xl/threadedcomments/") && n.ends_with(".xml") { "Comentário em conversa (vira anotação simples)" }
    else { return None })
}

/// `../drawings/drawing1.xml` visto de `xl/worksheets` vira `xl/drawings/drawing1.xml`.
fn caminho_relativo(pasta: &str, alvo: &str) -> String {
    if let Some(abs) = alvo.strip_prefix('/') { return abs.to_owned(); }
    let mut partes: Vec<&str> = pasta.split('/').filter(|p| !p.is_empty()).collect();
    for p in alvo.split('/') {
        match p { ".." => { partes.pop(); } "." | "" => {} outro => partes.push(outro) }
    }
    partes.join("/")
}

/// Os trechos `<…tag …>…</…tag>` (de qualquer uma das `tags`) do XML lido aos pedaços: a aba pode ter
/// centenas de MB e só os trechos ficam na memória. Com ou sem prefixo (`<x14:dataValidations>`).
fn trechos(mut leitor: impl std::io::Read, tags: &[&[u8]]) -> Vec<String> {
    let mut saida = Vec::new();
    let mut janela: Vec<u8> = Vec::new();
    let mut pedaco = vec![0u8; 1 << 16];
    // Bloco aberto e a marca que o fecha (com o prefixo da abertura: `</x14:tag>`).
    let mut dentro: Option<(Vec<u8>, Vec<u8>)> = None;
    let achar = |h: &[u8], a: &[u8]| h.windows(a.len()).position(|w| w == a);
    loop {
        let n = match leitor.read(&mut pedaco) { Ok(0) | Err(_) => break, Ok(n) => n };
        janela.extend_from_slice(&pedaco[..n]);
        loop {
            match &mut dentro {
                None => {
                    // A primeira das marcas na janela.
                    let Some((i, tag)) = tags.iter().filter_map(|t| achar(&janela, t).map(|i| (i, *t))).min_by_key(|(i, _)| *i) else { break };
                    let antes = &janela[..i];
                    let lt = antes.iter().rposition(|&c| c == b'<');
                    let depois = janela.get(i + tag.len()).copied();
                    let ok = lt.is_some_and(|j| { let p = &antes[j + 1..]; (p.is_empty() || p.ends_with(b":")) && !p.starts_with(b"/") && !p.contains(&b' ') });
                    match (ok, depois) {
                        (_, None) => break, // pede mais bytes para saber o que vem depois
                        (true, Some(b' ' | b'>')) => {
                            let j = lt.expect("conferido no ok");
                            let fecho = [b"</".as_slice(), &janela[j + 1..i], tag, b">"].concat();
                            dentro = Some((janela[j..].to_vec(), fecho));
                            janela.clear();
                        }
                        _ => { janela.drain(..i + tag.len()); }
                    }
                }
                Some((bloco, fecho)) => {
                    bloco.append(&mut janela);
                    // `<tag …/>` fecha na própria abertura (sem isso o resto da aba entraria no bloco).
                    let sozinha = bloco.iter().position(|&c| c == b'>').filter(|&g| g > 0 && bloco[g - 1] == b'/').map(|g| g + 1);
                    let Some(fim) = sozinha.or_else(|| achar(bloco, fecho).map(|i| i + fecho.len())) else { break };
                    janela = bloco.split_off(fim);
                    saida.push(String::from_utf8_lossy(bloco).into_owned());
                    dentro = None;
                }
            }
        }
        // Fora de bloco, guarda só a ponta (uma marca pode vir partida entre dois pedaços).
        if dentro.is_none() && janela.len() > 256 { janela.drain(..janela.len() - 256); }
    }
    saida
}

/// Quantas vezes a marca `<tag` (com ou sem prefixo, `<x:tag`) abre no XML.
fn abre(texto: &str, tag: &str) -> usize {
    texto.match_indices(tag).filter(|(i, _)| {
        let antes = &texto[..*i];
        let depois = texto[i + tag.len()..].chars().next();
        let inicio = antes.rfind('<').map(|j| &antes[j + 1..]);
        // Logo depois de `<` ou de `<prefixo:`, e não `</`.
        matches!(depois, Some(' ' | '>' | '/')) && inicio.is_some_and(|p| p.is_empty() || (p.ends_with(':') && !p.starts_with('/') && !p.contains([' ', '"'])))
    }).count()
}

pub fn nao_convertidos(bytes: Vec<u8>) -> Result<Vec<NaoConvertido>, String> {
    let mut z = ZipArchive::new(Cursor::new(std::sync::Arc::<[u8]>::from(bytes))).map_err(|e| feathersheet_core::trf!("Não parece um arquivo ZIP: {e}"))?;
    let mut saida: Vec<NaoConvertido> = Vec::new();
    let mut somar = |aba: Option<&str>, o_que: &'static str, n: usize| {
        if n == 0 { return; }
        match saida.iter_mut().find(|x| x.aba.as_deref() == aba && x.o_que == o_que) {
            Some(x) => x.quantos += n,
            None => saida.push(NaoConvertido { aba: aba.map(str::to_owned), o_que, quantos: n }),
        }
    };
    let livro = xml::parse_xml(&entry_text(&mut z, "xl/workbook.xml")?).map_err(|e| e.to_string())?;
    let rels = sheet::read_rels(&entry_text(&mut z, "xl/_rels/workbook.xml.rels")?);
    for no in livro.child("sheets").into_iter().flat_map(|s| s.children_named("sheet")) {
        let (Some(nome), Some(alvo)) = (no.attr("name"), no.attr("r:id").and_then(|id| rels.get(id))) else { continue };
        let caminho = match alvo.strip_prefix('/') { Some(a) => a.to_owned(), None => format!("xl/{}", alvo.trim_start_matches("./")) };
        if !caminho.starts_with("xl/worksheets/") { continue; } // gráfico/macro/diálogo: contados pela parte
        // Validação de tipo que o motor não tem (tamanho do texto, personalizada qualquer): a leitura deixa
        // de fora e o Ctrl+S apagaria. Só o trecho `<dataValidations>` é guardado, não a aba inteira.
        // shortcut: descompacta cada aba (≈1 s a mais no 1º Ctrl+S de um .xlsx de centenas de MB); se pesar,
        // guardar a contagem que a leitura já faz (`validacoes::ler`) na abertura.
        if let Ok(f) = z.by_name(&caminho) {
            let mut blocos = trechos(f, &[b"dataValidations", b"conditionalFormatting", b"conditionalFormattings", b"autoFilter"]);
            // AutoFiltro da aba, fora de tabela (o da tabela mora na parte da tabela): o app só filtra tabela.
            let e_filtro = |b: &String| b.trim_start_matches('<').split([' ', '/', '>']).next().is_some_and(|n| n.ends_with("autoFilter"));
            somar(Some(nome), "Filtro da planilha (fora de tabela)", blocos.iter().filter(|b| e_filtro(b)).count());
            blocos.retain(|b| !e_filtro(b));
            if !blocos.is_empty() {
                // Os blocos x14 entram como `extLst/ext`, onde os leitores de sempre procuram.
                let (x14, classicos): (Vec<_>, Vec<_>) = blocos.into_iter().partition(|b| b.starts_with("<x14:"));
                let xml = format!("<worksheet>{}<extLst><ext>{}</ext></extLst></worksheet>", classicos.concat(), x14.concat());
                if let Ok(raiz) = xml::parse_xml(&xml) {
                    somar(Some(nome), "Validação de dados sem equivalente", super::validacoes::ler(&raiz).1);
                    // Ícones, período de datas, desvio padrão…: o estilo (dxf) não importa para contar.
                    somar(Some(nome), "Formatação condicional sem equivalente", super::condicional::ler(&raiz, &[]).fora.len());
                }
            }
        }
        let (pasta, arquivo) = caminho.rsplit_once('/').unwrap_or(("", &caminho));
        for alvo in sheet::read_rels(&entry_text(&mut z, &format!("{pasta}/_rels/{arquivo}.rels")).unwrap_or_default()).values() {
            if let Some(o) = pelo_alvo(alvo) { somar(Some(nome), o, 1); }
            // Desenho da aba: desde 20/11/2026 a forma vai para o .xlsx gravado, mas sem degradê, sombra,
            // ajustes e grupo. Contar aqui segue fazendo o Ctrl+S pedir "Salvar como" no arquivo original.
            let a = alvo.to_ascii_lowercase();
            if a.contains("drawings/") && !a.contains("vmldrawing") && a.ends_with(".xml") {
                let desenho = caminho_relativo(pasta, alvo);
                let texto = entry_text(&mut z, &desenho).unwrap_or_default();
                somar(Some(nome), "Forma ou caixa de texto", texto.matches("<xdr:sp>").count() + texto.matches("<xdr:sp ").count());
            }
        }
    }
    // Formatos que o estilo do FeatherSheet não guarda: o .xlsx gravado perderia (vira sólido ou some).
    let estilos = entry_text(&mut z, "xl/styles.xml").unwrap_or_default();
    somar(None, "Preenchimento em degradê", abre(&estilos, "gradientFill"));
    somar(None, "Borda diagonal", ["diagonalUp=\"1\"", "diagonalDown=\"1\"", "diagonalUp=\"true\"", "diagonalDown=\"true\""].iter().map(|a| estilos.matches(a).count()).sum());
    somar(None, "Preenchimento com hachura", estilos.match_indices("patternType=\"").filter(|(i, m)| {
        let resto = &estilos[i + m.len()..];
        !["none\"", "solid\"", "gray125\""].iter().any(|v| resto.starts_with(v))
    }).count());
    // Texto com parte formatada (negrito numa palavra, cor num trecho): a célula guarda um formato só.
    let textos = entry_text(&mut z, "xl/sharedStrings.xml").unwrap_or_default();
    somar(None, "Texto com formatação em parte da célula", textos.split("</si>").filter(|si| si.contains("<rPr") || si.contains(":rPr")).count());
    let partes: Vec<String> = z.file_names().map(str::to_owned).collect();
    for p in &partes {
        if let Some(o) = pela_parte(p) { somar(None, o, 1); }
    }
    Ok(saida)
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[test]
    fn acha_activex_e_partes_do_arquivo_e_nao_conta_minigrafico() {
        let mut bytes = Vec::new();
        {
            let mut zw = zip::ZipWriter::new(Cursor::new(&mut bytes));
            let mut f = |n: &str, t: &str| { zw.start_file(n, SimpleFileOptions::default()).unwrap(); zw.write_all(t.as_bytes()).unwrap(); };
            f("xl/workbook.xml", r#"<workbook xmlns:r="r"><sheets><sheet name="Vendas" r:id="rId1"/><sheet name="Gráf" r:id="rId2"/></sheets></workbook>"#);
            f("xl/_rels/workbook.xml.rels", r#"<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Target="chartsheets/sheet1.xml"/></Relationships>"#);
            f("xl/worksheets/sheet1.xml", r#"<worksheet><sheetData/><dataValidations count="2"><dataValidation type="textLength" operator="lessThan" sqref="A1"><formula1>5</formula1></dataValidation><dataValidation type="list" sqref="B1"><formula1>"a,b"</formula1></dataValidation></dataValidations><autoFilter ref="A1:B9"/><conditionalFormatting sqref="A1:A9"><cfRule type="iconSet" priority="1"><iconSet><cfvo type="percent" val="0"/></iconSet></cfRule></conditionalFormatting><extLst><x14:sparkline></x14:sparkline><x14:sparkline></x14:sparkline></extLst></worksheet>"#);
            f("xl/worksheets/_rels/sheet1.xml.rels", r#"<Relationships><Relationship Id="rId1" Target="../activeX/activeX1.xml"/><Relationship Id="rId2" Target="../activeX/activeX2.xml"/><Relationship Id="rId3" Target="../drawings/drawing1.xml"/></Relationships>"#);
            f("xl/drawings/drawing1.xml", "<xdr:wsDr><xdr:twoCellAnchor><xdr:pic><xdr:nvPicPr/></xdr:pic></xdr:twoCellAnchor><xdr:twoCellAnchor><xdr:graphicFrame/></xdr:twoCellAnchor></xdr:wsDr>");
            f("xl/chartsheets/sheet1.xml", "<chartsheet/>");
            f("xl/connections.xml", "<connections/>");
            f("xl/threadedComments/threadedComment1.xml", "<ThreadedComments/>");
            f("xl/styles.xml", r#"<styleSheet><fills><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill>
              <fill><patternFill patternType="lightGrid"/></fill><fill><gradientFill degree="90"><stop position="0"/></gradientFill></fill></fills>
              <borders><border diagonalUp="1"><diagonal style="thin"/></border></borders></styleSheet>"#);
            f("xl/sharedStrings.xml", r#"<sst><si><t>liso</t></si><si><r><rPr><b/></rPr><t>Total</t></r><r><t> do mês</t></r></si></sst>"#);
            zw.finish().unwrap();
        }
        let mut r = nao_convertidos(bytes).unwrap();
        r.sort_by_key(|x| x.o_que);
        let v: Vec<_> = r.iter().map(|x| (x.aba.as_deref(), x.o_que, x.quantos)).collect();
        assert_eq!(v, vec![
            (None, "Borda diagonal", 1),
            (None, "Comentário em conversa (vira anotação simples)", 1),
            (None, "Conexão de dados externos", 1),
            (Some("Vendas"), "Controle ActiveX", 2),
            (Some("Vendas"), "Filtro da planilha (fora de tabela)", 1),
            (Some("Vendas"), "Formatação condicional sem equivalente", 1),
            (None, "Planilha de gráfico", 1),
            (None, "Preenchimento com hachura", 1),
            (None, "Preenchimento em degradê", 1),
            (None, "Texto com formatação em parte da célula", 1),
            (Some("Vendas"), "Validação de dados sem equivalente", 1),
        ]);
        // Pedaço a pedaço: a marca partida entre dois `read` ainda é achada.
        struct AosPoucos<'a>(&'a [u8]);
        impl std::io::Read for AosPoucos<'_> {
            fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> { let n = self.0.len().min(3).min(b.len()); b[..n].copy_from_slice(&self.0[..n]); self.0 = &self.0[n..]; Ok(n) }
        }
        let xml = b"<a><x14:dataValidations n=\"1\"><x/></x14:dataValidations><dataValidationsX/><autoFilter ref=\"A1\"/><dataValidations>y</dataValidations></a>";
        assert_eq!(trechos(AosPoucos(xml), &[b"dataValidations", b"autoFilter"]),
            vec!["<x14:dataValidations n=\"1\"><x/></x14:dataValidations>", "<autoFilter ref=\"A1\"/>", "<dataValidations>y</dataValidations>"]);
        assert_eq!(abre("<x:gradientFill a='1'></x:gradientFill><gradientFill/>", "gradientFill"), 2);
    }
}
