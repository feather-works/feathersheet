//! Árvore de XML pequena, para as partes do `.xlsx` — espelha `parseXml`/
//! `child`/`children` de `src/io/xlsx.ts:33-160`. Usa a crate `quick-xml`
//! (não um analisador à mão como o TypeScript: lá não havia dependência
//! disponível; aqui há, e o alvo são arquivos reais bem formados).
//!
//! Nome dos nós vem sem prefixo de namespace (`local_name`), igual ao JS —
//! o projeto não resolve namespace de verdade, só casa pelo nome local.
//! Atributo mantém o nome como veio (com prefixo, ex. `r:id`), também igual
//! ao JS, porque o resto do código lê `attrs.get("r:id")` direto.

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::collections::HashMap;
use std::io::BufRead;

mod rapido;

#[derive(Debug, Default, Clone)]
pub struct XmlNode {
    pub name: String,
    pub attrs: HashMap<String, String>,
    pub children: Vec<XmlNode>,
    pub text: String,
}

impl XmlNode {
    pub fn child(&self, name: &str) -> Option<&XmlNode> {
        self.children.iter().find(|c| c.name == name)
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a XmlNode> {
        self.children.iter().filter(move |c| c.name == name)
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(|s| s.as_str())
    }
}

fn local_name(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    match s.find(':') {
        Some(i) => s[i + 1..].to_string(),
        None => s.to_string(),
    }
}

fn read_attrs(e: &quick_xml::events::BytesStart, decoder: quick_xml::Decoder) -> HashMap<String, String> {
    let mut attrs = HashMap::new();
    for a in e.attributes().flatten() {
        let key = String::from_utf8_lossy(a.key.as_ref()).to_string();
        let val = a
            .decode_and_unescape_value(decoder)
            .map(|c| c.to_string())
            .unwrap_or_default();
        attrs.insert(key, val);
    }
    attrs
}

/// Faz o mesmo que `parseXml` do TypeScript: devolve o primeiro elemento de
/// topo (um documento com vários elementos de topo, XML inválido mas
/// tolerado, só olha o primeiro). Erro só em XML realmente quebrado (tag
/// nunca fechada em condições que o `quick-xml` não consegue recuperar).
pub fn parse_xml(src: &str) -> Result<XmlNode, String> {
    parse_xml_impl(Reader::from_reader(src.as_bytes()), None)
}

/// Lê a mesma estrutura do XLSX, mas emite cada `row` de `sheetData`
/// imediatamente para o consumidor, em vez de acumular dezenas de milhares
/// de nós XML até o fim da planilha. Os outros nós (cols, links, mesclas,
/// condicional) continuam na árvore e são tratados pelo mesmo importador.
/// A API geral `parse_xml` continua com seu comportamento original.
pub fn parse_xml_linhas(
    src: &str,
    callback: &mut dyn FnMut(&XmlNode),
) -> Result<XmlNode, String> {
    parse_xml_items_reader(src.as_bytes(), "sheetData", "row", callback)
}

/// Lê diretamente de um fluxo ZIP sem montar a entrada XML inteira na RAM.
pub fn parse_xml_items_reader<R: BufRead>(
    source: R,
    parent: &str,
    item: &str,
    callback: &mut dyn FnMut(&XmlNode),
) -> Result<XmlNode, String> {
    let mut adapter = |node: &XmlNode| { callback(node); Ok(()) };
    parse_xml_items_reader_result(source, parent, item, &mut adapter)
}

/// Variante cancelável: interrompe a descompressão assim que o consumidor
/// detectar excesso de células/strings, sem varrer gigabytes até o EOF.
pub fn parse_xml_items_reader_result<R: BufRead>(
    source: R,
    parent: &str,
    item: &str,
    callback: &mut dyn FnMut(&XmlNode) -> Result<(), String>,
) -> Result<XmlNode, String> {
    parse_xml_impl(Reader::from_reader(source), Some((parent, item, callback)))
}

fn parse_xml_impl<R: BufRead>(
    mut reader: Reader<R>,
    mut callback: Option<(&str, &str, &mut dyn FnMut(&XmlNode) -> Result<(), String>)>,
) -> Result<XmlNode, String> {
    reader.config_mut().trim_text(false);
    let mut stack: Vec<XmlNode> = vec![XmlNode::default()];
    let mut buf = Vec::new();

    loop {
        let decoder = reader.decoder();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let node = XmlNode {
                    name: local_name(e.name().as_ref()),
                    attrs: read_attrs(&e, decoder),
                    ..Default::default()
                };
                stack.push(node);
            }
            Ok(Event::Empty(e)) => {
                let node = XmlNode {
                    name: local_name(e.name().as_ref()),
                    attrs: read_attrs(&e, decoder),
                    ..Default::default()
                };
                let emitir = callback.as_ref().is_some_and(|(pai, item, _)|
                    stack.last().is_some_and(|n| n.name == *pai) && node.name == *item
                );
                if emitir {
                    if let Some((_, _, ler)) = callback.as_mut() { ler(&node)?; }
                } else if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                }
            }
            Ok(Event::End(_)) => {
                if stack.len() > 1 {
                    let node = stack.pop().expect("verificado acima");
                    let emitir = callback.as_ref().is_some_and(|(pai, item, _)|
                        stack.last().is_some_and(|n| n.name == *pai) && node.name == *item
                    );
                    if emitir {
                        if let Some((_, _, ler)) = callback.as_mut() { ler(&node)?; }
                    } else {
                        stack.last_mut().expect("raiz sempre existe").children.push(node);
                    }
                }
            }
            Ok(Event::Text(e)) => {
                let txt = e.decode().unwrap_or_default();
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&txt);
                }
            }
            // `&amp;`, `&#233;` etc. chegam como evento à parte (não dentro
            // do Text), diferente do parser à mão do JS — resolve aqui pra
            // ficar igual: 5 entidades nomeadas + referência numérica.
            Ok(Event::GeneralRef(e)) => {
                let resolved = e.resolve_char_ref().ok().flatten().map(|c| c.to_string()).or_else(|| {
                    let name = e.decode().ok()?;
                    match name.as_ref() {
                        "lt" => Some("<".to_string()),
                        "gt" => Some(">".to_string()),
                        "amp" => Some("&".to_string()),
                        "quot" => Some("\"".to_string()),
                        "apos" => Some("'".to_string()),
                        _ => None,
                    }
                });
                if let (Some(top), Some(txt)) = (stack.last_mut(), resolved) {
                    top.text.push_str(&txt);
                }
            }
            Ok(Event::CData(e)) => {
                let txt = String::from_utf8_lossy(e.as_ref()).to_string();
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&txt);
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {} // comentário, PI, declaração — ignorados, igual ao JS
            Err(e) => return Err(format!("XML inválido: {e}")),
        }
        buf.clear();
    }

    let mut root = stack.pop().unwrap_or_default();
    Ok(if root.children.is_empty() {
        root
    } else {
        root.children.remove(0)
    })
}

// ---------------------------------------------------------------- linhas rápidas

/// Texto com "existe ou não", reaproveitando a memória de uma linha para a
/// outra (`limpar` não devolve a capacidade).
#[derive(Debug, Default)]
pub struct Campo {
    tem: bool,
    texto: String,
}

impl Campo {
    fn limpar(&mut self) {
        self.tem = false;
        self.texto.clear();
    }
    fn definir(&mut self, valor: &str) {
        self.tem = true;
        self.texto.clear();
        self.texto.push_str(valor);
    }
    fn marcar(&mut self) {
        self.tem = true;
    }
    pub fn get(&self) -> Option<&str> {
        self.tem.then_some(self.texto.as_str())
    }
}

/// Uma `<c>` do `sheetData`, só com o que a importação usa: atributos
/// `r`/`s`/`t`/`cm`, a `<f>` (texto e `t`/`si`/`ref`), o texto do `<v>` e o
/// texto inline (`<is>`, das `<t>` diretas ou das `<r><t>`, sem as `<rPh>`).
#[derive(Debug, Default)]
pub struct CelulaXml {
    pub r: Campo,
    pub s: Campo,
    pub t: Campo,
    pub cm: Campo,
    /// Metadado de valor (`vm`): a imagem "Colocar na célula" do Excel.
    pub vm: Campo,
    pub f: Campo,
    pub f_t: Campo,
    pub f_si: Campo,
    pub f_ref: Campo,
    pub v: Campo,
    pub inline: Campo,
}

impl CelulaXml {
    fn limpar(&mut self) {
        for c in [&mut self.r, &mut self.s, &mut self.t, &mut self.cm, &mut self.vm, &mut self.f, &mut self.f_t,
            &mut self.f_si, &mut self.f_ref, &mut self.v, &mut self.inline] {
            c.limpar();
        }
    }
    pub fn attr(&self, nome: &str) -> Option<&str> {
        match nome {
            "r" => self.r.get(),
            "s" => self.s.get(),
            "t" => self.t.get(),
            "cm" => self.cm.get(),
            "vm" => self.vm.get(),
            _ => None,
        }
    }
}

/// Uma `<row>`: os atributos que a importação lê e as células. As células
/// e os textos são reaproveitados linha a linha: ler 13 milhões de células
/// não cria um objeto (nem um mapa de atributos) por nó, como a árvore.
#[derive(Debug, Default)]
pub struct LinhaXml {
    pub r: Campo,
    pub s: Campo,
    pub custom_format: Campo,
    pub hidden: Campo,
    pub ht: Campo,
    pub custom_height: Campo,
    pub outline_level: Campo,
    pub collapsed: Campo,
    celulas: Vec<CelulaXml>,
    usadas: usize,
}

impl LinhaXml {
    fn limpar(&mut self) {
        for c in [&mut self.r, &mut self.s, &mut self.custom_format, &mut self.hidden, &mut self.ht,
            &mut self.custom_height, &mut self.outline_level, &mut self.collapsed] {
            c.limpar();
        }
        self.usadas = 0;
    }
    fn nova_celula(&mut self) -> &mut CelulaXml {
        if self.usadas == self.celulas.len() {
            self.celulas.push(CelulaXml::default());
        }
        let c = &mut self.celulas[self.usadas];
        c.limpar();
        self.usadas += 1;
        c
    }
    pub fn celulas(&self) -> &[CelulaXml] {
        &self.celulas[..self.usadas]
    }
    pub fn attr(&self, nome: &str) -> Option<&str> {
        match nome {
            "r" => self.r.get(),
            "s" => self.s.get(),
            "customFormat" => self.custom_format.get(),
            "hidden" => self.hidden.get(),
            "ht" => self.ht.get(),
            "customHeight" => self.custom_height.get(),
            "outlineLevel" => self.outline_level.get(),
            "collapsed" => self.collapsed.get(),
            _ => None,
        }
    }
}

/// Nome local (sem prefixo de namespace), sem alocar.
fn local(raw: &[u8]) -> &[u8] {
    match raw.iter().position(|b| *b == b':') {
        Some(i) => &raw[i + 1..],
        None => raw,
    }
}

/// Copia os atributos pedidos de um elemento para os campos, sem mapa.
fn atributos(e: &quick_xml::events::BytesStart, decoder: quick_xml::Decoder,
    campos: &mut [(&[u8], &mut Campo)]) {
    for a in e.attributes().flatten() {
        let chave = a.key.as_ref();
        if let Some((_, campo)) = campos.iter_mut().find(|(nome, _)| *nome == chave) {
            match a.decode_and_unescape_value(decoder) {
                Ok(v) => campo.definir(&v),
                Err(_) => campo.marcar(),
            }
        }
    }
}

/// `&amp;`, `&#233;`… como o leitor da árvore resolve.
fn entidade(e: &quick_xml::events::BytesRef) -> Option<String> {
    e.resolve_char_ref().ok().flatten().map(|c| c.to_string()).or_else(|| {
        let nome = e.decode().ok()?;
        Some(match nome.as_ref() {
            "lt" => "<", "gt" => ">", "amp" => "&", "quot" => "\"", "apos" => "'",
            _ => return None,
        }.to_string())
    })
}

/// Para onde vai o texto lido dentro da célula.
#[derive(Clone, Copy, PartialEq)]
enum Alvo { Nada, F, V, Inline }

/// Lê o conteúdo de uma `<row>` já aberta até o `</row>`.
fn ler_linha<R: BufRead>(reader: &mut Reader<R>, buf: &mut Vec<u8>, linha: &mut LinhaXml) -> Result<(), String> {
    let mut alvo = Alvo::Nada;
    let mut dentro_is = false;
    let mut em_rph = false;
    loop {
        buf.clear();
        let decoder = reader.decoder();
        let evento = reader.read_event_into(buf).map_err(|e| format!("XML inválido: {e}"))?;
        match evento {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let vazio = matches!(evento, Event::Empty(_));
                match local(e.name().as_ref()) {
                    b"c" => {
                        let c = linha.nova_celula();
                        atributos(e, decoder, &mut [(b"r", &mut c.r), (b"s", &mut c.s), (b"t", &mut c.t), (b"cm", &mut c.cm), (b"vm", &mut c.vm)]);
                        alvo = Alvo::Nada;
                    }
                    b"f" if linha.usadas > 0 => {
                        let c = &mut linha.celulas[linha.usadas - 1];
                        c.f.marcar();
                        atributos(e, decoder, &mut [(b"t", &mut c.f_t), (b"si", &mut c.f_si), (b"ref", &mut c.f_ref)]);
                        if !vazio { alvo = Alvo::F; }
                    }
                    b"v" if linha.usadas > 0 => {
                        linha.celulas[linha.usadas - 1].v.marcar();
                        if !vazio { alvo = Alvo::V; }
                    }
                    b"is" if linha.usadas > 0 => {
                        linha.celulas[linha.usadas - 1].inline.marcar();
                        dentro_is = !vazio;
                    }
                    b"rPh" if dentro_is && !vazio => em_rph = true,
                    b"t" if dentro_is && !em_rph && !vazio => alvo = Alvo::Inline,
                    _ => {}
                }
            }
            Event::End(ref e) => match local(e.name().as_ref()) {
                b"row" => return Ok(()),
                b"f" | b"v" | b"t" => alvo = Alvo::Nada,
                b"is" => dentro_is = false,
                b"rPh" => em_rph = false,
                _ => {}
            },
            Event::Text(ref e) if alvo != Alvo::Nada => {
                let txt = e.decode().unwrap_or_default();
                empurrar(linha, alvo, &txt);
            }
            Event::GeneralRef(ref e) if alvo != Alvo::Nada => {
                if let Some(txt) = entidade(e) { empurrar(linha, alvo, &txt); }
            }
            Event::CData(ref e) if alvo != Alvo::Nada => {
                let txt = String::from_utf8_lossy(e.as_ref()).to_string();
                empurrar(linha, alvo, &txt);
            }
            Event::Eof => return Err("XML inválido: <row> sem fim".into()),
            _ => {}
        }
    }
}

fn empurrar(linha: &mut LinhaXml, alvo: Alvo, txt: &str) {
    let Some(c) = linha.usadas.checked_sub(1).map(|i| &mut linha.celulas[i]) else { return };
    let campo = match alvo {
        Alvo::F => &mut c.f,
        Alvo::V => &mut c.v,
        Alvo::Inline => &mut c.inline,
        Alvo::Nada => return,
    };
    campo.texto.push_str(txt);
}

/// Como [`parse_xml_items_reader_result`] com `sheetData`/`row`, mas cada
/// linha chega como [`LinhaXml`] (sem árvore por célula). O resto do
/// documento (colunas, mesclas, links, condicional…) volta na árvore.
pub fn parse_xml_linhas_rapidas<R: BufRead>(
    mut source: R,
    ler: &mut dyn FnMut(&LinhaXml) -> Result<(), String>,
) -> Result<XmlNode, String> {
    use std::io::Read;
    // As linhas vão pelo leitor direto nos bytes (`rapido`); o documento sem
    // elas, pelo quick_xml. Sem `sheetData` reconhecível, tudo pelo quick_xml.
    let mut prefixo = Vec::new();
    let Some((vazio, sobra)) = rapido::ate_sheet_data(&mut source, &mut prefixo)? else {
        return parse_xml_linhas_quick(std::io::Cursor::new(prefixo).chain(source), ler);
    };
    let mut resto = if vazio {
        sobra
    } else {
        let mut linha = LinhaXml::default();
        rapido::linhas(&mut source, sobra, &mut linha, ler)?
    };
    source.read_to_end(&mut resto).map_err(|e| format!("XML: {e}"))?;
    if vazio {
        // `<sheetData/>` já fechou no prefixo.
        prefixo.extend_from_slice(&resto);
        return parse_xml_linhas_quick(&prefixo[..], &mut |_| Ok(()));
    }
    parse_xml_linhas_quick(std::io::Cursor::new(prefixo).chain(&resto[..]), &mut |_| Ok(()))
}

/// O leitor das linhas pelo quick_xml (o de antes do `rapido`): fica para o
/// XML que o leitor direto não reconhece e como referência nos testes.
fn parse_xml_linhas_quick<R: BufRead>(
    source: R,
    ler: &mut dyn FnMut(&LinhaXml) -> Result<(), String>,
) -> Result<XmlNode, String> {
    let mut reader = Reader::from_reader(source);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<XmlNode> = vec![XmlNode::default()];
    let mut buf = Vec::new();
    let mut buf_linha = Vec::new();
    let mut linha = LinhaXml::default();
    loop {
        let decoder = reader.decoder();
        let evento = reader.read_event_into(&mut buf);
        match evento {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e))
                if local(e.name().as_ref()) == b"row" && stack.last().is_some_and(|n| n.name == "sheetData") =>
            {
                let vazio = matches!(evento, Ok(Event::Empty(_)));
                linha.limpar();
                atributos(e, decoder, &mut [
                    (b"r", &mut linha.r), (b"s", &mut linha.s), (b"customFormat", &mut linha.custom_format),
                    (b"hidden", &mut linha.hidden), (b"ht", &mut linha.ht), (b"customHeight", &mut linha.custom_height),
                    (b"outlineLevel", &mut linha.outline_level), (b"collapsed", &mut linha.collapsed),
                ]);
                if !vazio {
                    ler_linha(&mut reader, &mut buf_linha, &mut linha)?;
                }
                ler(&linha)?;
            }
            Ok(Event::Start(e)) => {
                stack.push(XmlNode { name: local_name(e.name().as_ref()), attrs: read_attrs(&e, decoder), ..Default::default() });
            }
            Ok(Event::Empty(e)) => {
                let node = XmlNode { name: local_name(e.name().as_ref()), attrs: read_attrs(&e, decoder), ..Default::default() };
                stack.last_mut().expect("raiz sempre existe").children.push(node);
            }
            Ok(Event::End(_)) => {
                if stack.len() > 1 {
                    let node = stack.pop().expect("verificado acima");
                    stack.last_mut().expect("raiz sempre existe").children.push(node);
                }
            }
            Ok(Event::Text(e)) => {
                let txt = e.decode().unwrap_or_default();
                if let Some(top) = stack.last_mut() { top.text.push_str(&txt); }
            }
            Ok(Event::GeneralRef(e)) => {
                if let (Some(top), Some(txt)) = (stack.last_mut(), entidade(&e)) { top.text.push_str(&txt); }
            }
            Ok(Event::CData(e)) => {
                let txt = String::from_utf8_lossy(e.as_ref()).to_string();
                if let Some(top) = stack.last_mut() { top.text.push_str(&txt); }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(format!("XML inválido: {e}")),
        }
        buf.clear();
    }
    let mut root = stack.pop().unwrap_or_default();
    Ok(if root.children.is_empty() { root } else { root.children.remove(0) })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// As linhas que cada leitor entrega, em texto, e a árvore do resto.
    pub(super) fn ler_com(xml: &[u8], rapido: bool, capacidade: usize) -> (Vec<String>, String) {
        let mut linhas = Vec::new();
        let mut ler = |l: &LinhaXml| { linhas.push(format!("{l:?}").replace(&format!("usadas: {}", l.usadas), "")
            .split("celulas:").next().unwrap_or("").to_string() + &format!("{:?}", l.celulas())); Ok(()) };
        let fonte = std::io::BufReader::with_capacity(capacidade, xml);
        let arvore = if rapido { parse_xml_linhas_rapidas(fonte, &mut ler) } else { parse_xml_linhas_quick(fonte, &mut ler) };
        fn canon(n: &XmlNode, s: &mut String) {
            let mut attrs: Vec<_> = n.attrs.iter().collect();
            attrs.sort();
            // Espaço entre as linhas caía no texto do sheetData; ninguém lê.
            s.push_str(&format!("<{} {attrs:?} {:?}>", n.name, n.text.trim()));
            for c in &n.children { canon(c, s); }
            s.push_str("</>");
        }
        let arvore = arvore.map(|a| { let mut s = String::new(); canon(&a, &mut s); s });
        (linhas, format!("{arvore:?}"))
    }

    /// `FEATHERSHEET_XLSX=a.xlsx,b.xlsx`: os dois leitores em todas as abas,
    /// comparando um resumo de cada linha e medindo o tempo.
    #[test]
    #[ignore]
    fn leitor_rapido_igual_nos_arquivos() {
        use std::hash::{Hash, Hasher};
        use std::io::Read;
        let Ok(lista) = std::env::var("FEATHERSHEET_XLSX") else { return };
        for caminho in lista.split(',') {
            let f = std::fs::File::open(caminho).unwrap();
            let mut z = zip::ZipArchive::new(std::io::BufReader::new(f)).unwrap();
            let nomes: Vec<String> = z.file_names().filter(|n| n.starts_with("xl/worksheets/") && n.ends_with(".xml")).map(String::from).collect();
            for nome in nomes {
                let mut resultado = Vec::new();
                for rapido in [false, true] {
                    let mut entrada = std::io::BufReader::with_capacity(64 * 1024, z.by_name(&nome).unwrap());
                    let mut h = std::collections::hash_map::DefaultHasher::new();
                    let mut n = 0u64;
                    let t = std::time::Instant::now();
                    let mut ler = |l: &LinhaXml| {
                        n += 1;
                        for c in l.celulas() { format!("{c:?}").hash(&mut h); }
                        format!("{:?}{:?}{:?}{:?}", l.r, l.s, l.ht, l.hidden).hash(&mut h);
                        Ok(())
                    };
                    let arvore = if rapido { parse_xml_linhas_rapidas(&mut entrada, &mut ler) } else { parse_xml_linhas_quick(&mut entrada, &mut ler) };
                    let mut resto = Vec::new();
                    let _ = entrada.read_to_end(&mut resto);
                    eprintln!("{nome} {}: {n} linhas em {:?}", if rapido { "rápido" } else { "quick" }, t.elapsed());
                    resultado.push((n, h.finish(), arvore.map(|a| a.children.len())));
                }
                assert_eq!(resultado[0], resultado[1], "{caminho} {nome}");
            }
        }
    }

    #[test]
    fn leitor_rapido_das_linhas_igual_ao_quick_xml() {
        let casos: &[&str] = &[
            r#"<?xml version="1.0"?><worksheet xmlns="x"><dimension ref="A1:C3"/><sheetData><row r="1" spans="1:3" ht="20" customHeight="1"><c r="A1" s="2" t="s"><v>0</v></c><c r="B1"><f>SUM(A1:A3)&amp;"x"</f><v>6</v></c><c r="C1" t="inlineStr"><is><r><t xml:space="preserve"> a &lt;b&gt; </t></r><r><t>c&#233;&#x41;</t></r><rPh sb="0" eb="1"><t>ヨミ</t></rPh></is></c></row><row r="2"/><!-- nota --><row r="3" hidden="1"><c r="A3" t="str"><f t="shared" ref="A3:A9" si="0">B3*2</f><v>texto &quot;com&quot; aspas</v></c><c r="B3"><f t="shared" si="0"/><v>1</v></c><c r="C3" s='4'><v><![CDATA[a<b]]></v></c><c r="D3" t="e"><v>#N/A</v></c><c r="E3" s="7"/></row></sheetData><mergeCells count="1"><mergeCell ref="A1:B1"/></mergeCells><conditionalFormatting sqref="A1"><cfRule type="expression" priority="1"><formula>A1&gt;0</formula></cfRule></conditionalFormatting></worksheet>"#,
            r#"<x:worksheet xmlns:x="y"><x:sheetData><x:row r="1"><x:c r="A1"><x:v>5</x:v></x:c></x:row></x:sheetData></x:worksheet>"#,
            r#"<worksheet><sheetData/><cols><col min="1" max="2" width="9"/></cols></worksheet>"#,
            r#"<worksheet><sheetData></sheetData></worksheet>"#,
            r#"<worksheet><sheetData><row r="1"><c r="A1"><v>1</v></c><c r="B1" vm="1" cm="1"><v>2</v></c></row></sheetData></worksheet>"#,
            r#"<worksheet><cols/></worksheet>"#,
            "<worksheet><sheetData>\r\n  <row r=\"1\">\r\n    <c r=\"A1\"><v>\r\n7\r\n</v></c>\r\n  </row>\r\n</sheetData></worksheet>",
            r#"<worksheet><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>a &desconhecida; b</t></is></c><c r="B1" s="&bogus;"><v>1</v></c></row></sheetData></worksheet>"#,
        ];
        for (i, xml) in casos.iter().enumerate() {
            let xml = xml.replace("\\r\\n", "\r\n").replace("\\\"", "\"");
            for cap in [3, 64, 8192] {
                let esperado = ler_com(xml.as_bytes(), false, 8192);
                let obtido = ler_com(xml.as_bytes(), true, cap);
                if obtido != esperado {
                    let (a, b) = (format!("{obtido:?}"), format!("{esperado:?}"));
                    let d = a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
                    let ini = d.saturating_sub(150);
                    panic!("caso {i}, buffer {cap}\nrápido: {}\nquick:  {}", &a[ini..(d + 150).min(a.len())], &b[ini..(d + 150).min(b.len())]);
                }
            }
        }
    }

    #[test]
    fn parser_cancela_fluxo_ao_receber_erro_no_callback() {
        // A parte final é XML inválido: o parser deve parar no primeiro row
        // e devolver o erro solicitado, sem tentar ler o restante do ZIP.
        let xml = "<worksheet><sheetData><row r=\"1\"/></sheetData><invalida";
        let erro = parse_xml_items_reader_result(
            xml.as_bytes(), "sheetData", "row", &mut |_| Err("limite atingido".into()),
        ).unwrap_err();
        assert_eq!(erro, "limite atingido");
    }

    #[test]
    fn leitor_stream_de_itens_preserva_metadados_e_descarta_so_as_linhas() {
        use std::io::{BufReader, Cursor};
        let xml = br#"<worksheet><sheetData><row r="1"><c r="A1"><v>7</v></c></row><row r="2"/></sheetData><mergeCells><mergeCell ref="A1:A2"/></mergeCells></worksheet>"#;
        let mut rows = Vec::new();
        // BufRead pequeno: confirma que o leitor nao depende de todo o XML
        // estar numa unica String em memoria, como ocorre num arquivo ZIP.
        let source = BufReader::with_capacity(7, Cursor::new(xml));
        let root = parse_xml_items_reader(source, "sheetData", "row", &mut |row| {
            rows.push(row.attr("r").unwrap_or("").to_owned());
        }).unwrap();
        assert_eq!(rows, ["1", "2"]);
        assert!(root.child("sheetData").unwrap().children.is_empty());
        assert_eq!(root.child("mergeCells").unwrap().children.len(), 1);
    }

    #[test]
    fn leitor_incremental_de_linhas_descarta_a_arvore_de_celulas() {
        let mut linhas = Vec::new();
        let xml = r#"<worksheet><cols><col min="1" max="1" width="30"/></cols><sheetData><row r="1"><c r="A1"><v>42</v></c></row><row r="2"><c r="B2"><v>91</v></c></row><row r="3"/></sheetData><mergeCells><mergeCell ref="A1:B1"/></mergeCells></worksheet>"#;
        let arvore = parse_xml_linhas(xml, &mut |linha| {
            linhas.push((linha.attr("r").unwrap_or("").to_string(), linha.children.len()));
        }).unwrap();
        assert_eq!(linhas, vec![("1".into(), 1), ("2".into(), 1), ("3".into(), 0)]);
        assert!(arvore.child("sheetData").unwrap().children.is_empty());
        assert_eq!(arvore.child("mergeCells").unwrap().children.len(), 1);
        assert_eq!(arvore.child("cols").unwrap().children.len(), 1);
        // O parser genérico não deve descartar linhas!
        assert_eq!(parse_xml(xml).unwrap().child("sheetData").unwrap().children.len(), 3);
    }

    #[test]
    fn atributos_texto_e_aninhamento() {
        let xml = r#"<sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1"/></row></sheetData>"#;
        let root = parse_xml(xml).unwrap();
        assert_eq!(root.name, "sheetData");
        let row = root.child("row").unwrap();
        assert_eq!(row.attr("r"), Some("1"));
        let cells: Vec<_> = row.children_named("c").collect();
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0].attr("t"), Some("s"));
        assert_eq!(cells[0].child("v").unwrap().text, "0");
        assert_eq!(cells[1].attr("r"), Some("B1"));
    }

    #[test]
    fn entidades_e_cdata_voltam_ao_texto_original() {
        let xml = r#"<t>a &amp; b &lt;c&gt; &#233; &#x41;<![CDATA[<cru>]]></t>"#;
        let root = parse_xml(xml).unwrap();
        assert_eq!(root.text, "a & b <c> é A<cru>");
    }

    #[test]
    fn comentario_e_prologo_nao_viram_no() {
        let xml = r#"<?xml version="1.0"?><raiz><!-- comentário --><a/></raiz>"#;
        let root = parse_xml(xml).unwrap();
        assert_eq!(root.name, "raiz");
        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].name, "a");
    }

    #[test]
    fn namespace_casa_pelo_nome_local() {
        let xml = r#"<r:root xmlns:r="urn:x"><r:id>5</r:id></r:root>"#;
        let root = parse_xml(xml).unwrap();
        assert_eq!(root.name, "root");
        assert_eq!(root.child("id").unwrap().text, "5");
    }

    #[test]
    fn linhas_rapidas_leem_o_mesmo_que_a_arvore() {
        let xml = r#"<x:worksheet xmlns:x="a"><x:sheetData>
            <x:row r="1" ht="20" customHeight="1" outlineLevel="2"><x:c r="A1" s="3" t="s"><x:v>7</x:v></x:c>
              <x:c r="B1"><x:f t="shared" si="0" ref="B1:B9">A1&amp;"&lt;x"</x:f><x:v>1.5</x:v></x:c>
              <x:c r="C1" t="inlineStr"><x:is><x:r><x:t>Olá </x:t></x:r><x:r><x:t>mundo</x:t></x:r><x:rPh><x:t>fon</x:t></x:rPh></x:is></x:c>
              <x:c r="D1" s="2"/></x:row>
            <x:row r="2"/>
          </x:sheetData><x:mergeCells><x:mergeCell ref="A1:A2"/></x:mergeCells></x:worksheet>"#;
        let mut vistas = Vec::new();
        let raiz = parse_xml_linhas_rapidas(xml.as_bytes(), &mut |l| {
            let cs: Vec<String> = l.celulas().iter().map(|c| format!("{}|{}|{}|{}|{}|{}|{}",
                c.attr("r").unwrap_or(""), c.attr("s").unwrap_or("-"), c.attr("t").unwrap_or("-"),
                c.f.get().unwrap_or("-"), c.f_si.get().unwrap_or("-"), c.v.get().unwrap_or("-"),
                c.inline.get().unwrap_or("-"))).collect();
            vistas.push((l.attr("r").unwrap_or("").to_owned(), l.attr("ht").map(str::to_owned), l.attr("outlineLevel").map(str::to_owned), cs));
            Ok(())
        }).unwrap();
        assert_eq!(vistas.len(), 2);
        assert_eq!(vistas[0].1.as_deref(), Some("20"));
        assert_eq!(vistas[0].2.as_deref(), Some("2"));
        assert_eq!(vistas[0].3, [
            "A1|3|s|-|-|7|-",
            "B1|-|-|A1&\"<x\"|0|1.5|-",
            "C1|-|inlineStr|-|-|-|Olá mundo",
            "D1|2|-|-|-|-|-",
        ]);
        assert!(vistas[1].3.is_empty());
        assert!(raiz.child("mergeCells").is_some(), "o resto do documento continua na árvore");
    }
}
