//! Preencher um modelo `.docx` trocando marcadores (`#NOME#` → "Ana"), como o Localizar e Substituir do Word que a
//! "Planilha de Informações" fazia pelo VBA. A troca é feita no XML, sem passar pelo modelo de documento: o resto
//! do arquivo (estilos, tabelas, imagens, cabeçalhos) sai byte a byte como entrou.
//!
//! O Word costuma quebrar um marcador em vários `<w:t>` (`#NO` + `ME#`, por revisão ou correção ortográfica): os
//! `<w:t>` de um parágrafo são lidos como um texto só, e o trecho trocado fica no primeiro pedaço do marcador.

use std::io::Read;

/// Teto do `.docx` de modelo e de cada parte descompactada (um ZIP pequeno pode inflar muito).
const LIMITE_BYTES: u64 = 64 * 1024 * 1024;

/// As partes com texto do documento: corpo, cabeçalhos, rodapés e notas.
fn parte_com_texto(nome: &str) -> bool {
    nome == "word/document.xml" || nome == "word/footnotes.xml" || nome == "word/endnotes.xml"
        || (nome.starts_with("word/header") || nome.starts_with("word/footer")) && nome.ends_with(".xml")
}

/// Troca cada `(de, para)` em ordem e devolve o `.docx` novo e quantas trocas houve.
pub fn preencher(modelo: &[u8], trocas: &[(String, String)]) -> Result<(Vec<u8>, usize), String> {
    if modelo.len() as u64 > LIMITE_BYTES {
        return Err(feathersheet_core::tr!("O modelo .docx passa de 64 MB.").into());
    }
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(modelo))
        .map_err(|_| feathersheet_core::tr!("O modelo não é um .docx (não abriu como ZIP).").to_string())?;
    if z.by_name("word/document.xml").is_err() {
        return Err(feathersheet_core::tr!("O modelo não é um .docx (falta o word/document.xml).").into());
    }
    let mut entradas = Vec::with_capacity(z.len());
    let mut total = 0;
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| e.to_string())?;
        if f.is_dir() { continue; }
        let nome = f.name().to_owned();
        let mut bytes = Vec::new();
        f.by_ref().take(LIMITE_BYTES + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > LIMITE_BYTES {
            return Err(feathersheet_core::trf!("Uma parte do modelo ({nome}) passa de 64 MB."));
        }
        if parte_com_texto(&nome) {
            if let Ok(xml) = String::from_utf8(bytes.clone()) {
                let (novo, n) = substituir_xml(&xml, trocas);
                total += n;
                bytes = novo.into_bytes();
            }
        }
        entradas.push((nome, bytes));
    }
    Ok((crate::zip_write::zipar(entradas)?, total))
}

/// Um `<w:t>` do XML: onde começa e termina o conteúdo, e o texto já sem entidades.
struct Pedaco {
    abre: usize,
    inicio: usize,
    fim: usize,
    texto: String,
}

/// As trocas num XML do Word; devolve o XML e quantas trocas.
pub(crate) fn substituir_xml(xml: &str, trocas: &[(String, String)]) -> (String, usize) {
    let (pedacos, grupos) = pedacos_por_paragrafo(xml);
    let mut textos: Vec<String> = pedacos.iter().map(|p| p.texto.clone()).collect();
    let mut total = 0;
    for (de, para) in trocas.iter().filter(|(de, _)| !de.is_empty()) {
        for grupo in &grupos {
            total += trocar_no_grupo(&mut textos, grupo, de, para);
        }
    }
    if total == 0 { return (xml.to_owned(), 0); }
    // Reescreve só os `<w:t>` que mudaram; o resto do XML vai como estava.
    let mut saida = String::with_capacity(xml.len());
    let mut desde = 0;
    for (p, novo) in pedacos.iter().zip(&textos) {
        if *novo == p.texto { continue; }
        let tag = &xml[p.abre..p.inicio];
        saida.push_str(&xml[desde..p.abre]);
        // Espaço no começo/fim some sem o `xml:space="preserve"`.
        if tag.contains("xml:space") { saida.push_str(tag); } else { saida.push_str("<w:t xml:space=\"preserve\">"); }
        saida.push_str(&escapar(novo));
        desde = p.fim;
    }
    saida.push_str(&xml[desde..]);
    (saida, total)
}

/// Os `<w:t>` do XML e os grupos (índices) de cada parágrafo: marcador não passa de um parágrafo para outro.
fn pedacos_por_paragrafo(xml: &str) -> (Vec<Pedaco>, Vec<Vec<usize>>) {
    let (mut pedacos, mut grupos, mut atual) = (Vec::new(), Vec::new(), Vec::new());
    let mut i = 0;
    while let Some(rel) = xml[i..].find('<') {
        let abre = i + rel;
        let resto = &xml[abre..];
        let Some(fecha_tag) = resto.find('>') else { break };
        let tag = &resto[..=fecha_tag];
        let nome = tag[1..].split(|c: char| c.is_whitespace() || c == '>' || c == '/').next().unwrap_or("");
        if nome == "w:p" || nome == "/w:p" {
            if !atual.is_empty() { grupos.push(std::mem::take(&mut atual)); }
        } else if nome == "w:t" && !tag.ends_with("/>") {
            let inicio = abre + fecha_tag + 1;
            if let Some(f) = xml[inicio..].find("</w:t>") {
                let fim = inicio + f;
                atual.push(pedacos.len());
                pedacos.push(Pedaco { abre, inicio, fim, texto: desescapar(&xml[inicio..fim]) });
                i = fim + "</w:t>".len();
                continue;
            }
        }
        i = abre + fecha_tag + 1;
    }
    if !atual.is_empty() { grupos.push(atual); }
    (pedacos, grupos)
}

/// Troca `de` por `para` no texto do grupo (os pedaços de um parágrafo). O trecho novo fica no pedaço onde o
/// marcador começa; o resto do marcador some dos pedaços seguintes.
fn trocar_no_grupo(textos: &mut [String], grupo: &[usize], de: &str, para: &str) -> usize {
    let mut feitas = 0;
    let mut desde = 0; // posição (em bytes do texto juntado) a partir da qual procurar
    loop {
        let junto: String = grupo.iter().map(|&k| textos[k].as_str()).collect();
        let Some(rel) = junto[desde..].find(de) else { return feitas };
        let (ini, fim) = (desde + rel, desde + rel + de.len());
        let mut pos = 0;
        let mut colocado = false;
        for &k in grupo {
            let (a, b) = (pos, pos + textos[k].len());
            pos = b;
            if b <= ini || a >= fim { continue; }
            let (corte_a, corte_b) = (ini.max(a) - a, fim.min(b) - a);
            let t = &textos[k];
            let novo = format!("{}{}{}", &t[..corte_a], if colocado { "" } else { para }, &t[corte_b..]);
            colocado = true;
            textos[k] = novo;
        }
        feitas += 1;
        desde = ini + para.len();
    }
}

fn desescapar(t: &str) -> String {
    if !t.contains('&') { return t.to_owned(); }
    let mut s = String::with_capacity(t.len());
    let mut resto = t;
    while let Some(i) = resto.find('&') {
        s.push_str(&resto[..i]);
        let depois = &resto[i..];
        let Some(fim) = depois.find(';').filter(|&f| f <= 10) else { s.push('&'); resto = &depois[1..]; continue };
        let ent = &depois[1..fim];
        let c = match ent {
            "amp" => Some('&'), "lt" => Some('<'), "gt" => Some('>'), "quot" => Some('"'), "apos" => Some('\''),
            _ => ent.strip_prefix("#x").and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| ent.strip_prefix('#').and_then(|d| d.parse().ok())).and_then(char::from_u32),
        };
        match c { Some(c) => { s.push(c); resto = &depois[fim + 1..]; } None => { s.push('&'); resto = &depois[1..]; } }
    }
    s.push_str(resto);
    s
}

fn escapar(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod testes {
    use super::*;

    fn trocas(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter().map(|(a, b)| ((*a).to_owned(), (*b).to_owned())).collect()
    }

    #[test]
    fn marcador_quebrado_entre_pedacos_e_trocado_sem_mexer_no_resto() {
        let xml = r#"<w:document><w:body><w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Eu, #NO</w:t></w:r><w:r><w:t>ME#, CPF #CPF#</w:t></w:r></w:p><w:p><w:r><w:t xml:space="preserve"> fim #NOME#</w:t></w:r></w:p></w:body></w:document>"#;
        let (novo, n) = substituir_xml(xml, &trocas(&[("#NOME#", "Ana & Bia <Ltda>"), ("#CPF#", "123")]));
        assert_eq!(n, 3);
        assert!(novo.contains("<w:rPr><w:b/></w:rPr><w:t xml:space=\"preserve\">Eu, Ana &amp; Bia &lt;Ltda&gt;</w:t>"), "{novo}");
        assert!(novo.contains("<w:t xml:space=\"preserve\">, CPF 123</w:t>"), "{novo}");
        assert!(novo.contains("<w:t xml:space=\"preserve\"> fim Ana &amp; Bia &lt;Ltda&gt;</w:t>"), "{novo}");
    }

    #[test]
    fn marcador_nao_atravessa_paragrafos_e_sem_troca_o_xml_fica_igual() {
        let xml = "<w:p><w:r><w:t>#NO</w:t></w:r></w:p><w:p><w:r><w:t>ME#</w:t></w:r></w:p>";
        assert_eq!(substituir_xml(xml, &trocas(&[("#NOME#", "x")])), (xml.to_owned(), 0));
        // Entidade no modelo conta como o caractere; troca pelo próprio marcador não gira para sempre.
        let xml = "<w:p><w:r><w:t>R&amp;D #A#</w:t></w:r></w:p>";
        let (novo, n) = substituir_xml(xml, &trocas(&[("R&D", "P&D"), ("#A#", "#A##A#")]));
        assert_eq!(n, 2);
        assert!(novo.contains(">P&amp;D #A##A#</w:t>"), "{novo}");
    }

    #[test]
    fn docx_inteiro_troca_corpo_e_cabecalho_e_recusa_o_que_nao_e_docx() {
        let corpo = r#"<w:document><w:body><w:p><w:r><w:t>Procuração de #NOME#</w:t></w:r></w:p></w:body></w:document>"#;
        let cabecalho = r#"<w:hdr><w:p><w:r><w:t>#NOME#</w:t></w:r></w:p></w:hdr>"#;
        let modelo = crate::zip_write::zipar(vec![
            ("[Content_Types].xml".into(), b"<Types/>".to_vec()),
            ("word/document.xml".into(), corpo.as_bytes().to_vec()),
            ("word/header1.xml".into(), cabecalho.as_bytes().to_vec()),
            ("word/media/image1.png".into(), vec![0x89, b'P', b'N', b'G', 0, 1, 2]),
        ]).unwrap();
        let (saida, n) = preencher(&modelo, &trocas(&[("#NOME#", "Ana")])).unwrap();
        assert_eq!(n, 2);
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(saida)).unwrap();
        let ler = |z: &mut zip::ZipArchive<std::io::Cursor<Vec<u8>>>, n: &str| {
            let mut b = Vec::new();
            z.by_name(n).unwrap().read_to_end(&mut b).unwrap();
            b
        };
        assert!(String::from_utf8(ler(&mut z, "word/document.xml")).unwrap().contains("Procuração de Ana"));
        assert!(String::from_utf8(ler(&mut z, "word/header1.xml")).unwrap().contains(">Ana</w:t>"));
        assert_eq!(ler(&mut z, "word/media/image1.png"), vec![0x89, b'P', b'N', b'G', 0, 1, 2], "imagem byte a byte");
        assert!(preencher(b"nao e zip", &[]).is_err());
        let sem_word = crate::zip_write::zipar(vec![("a.txt".into(), b"x".to_vec())]).unwrap();
        assert!(preencher(&sem_word, &[]).is_err());
    }
}
