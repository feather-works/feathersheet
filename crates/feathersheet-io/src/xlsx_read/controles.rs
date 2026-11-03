//! Controles de formulário do Excel (botão, botão de girar, caixa de seleção, opção, caixa de
//! combinação, lista, barra de rolagem, rótulo, caixa de grupo), lidos do VML da aba
//! (`xl/drawings/vmlDrawingN.vml`, o mesmo arquivo das notas). O VML traz tudo — posição em
//! células, texto, macro e célula ligada — nos arquivos do Excel 2007 e nos de hoje (que repetem
//! parte disso em `<controls>` e `xl/ctrlProps/`). Lido como texto: o VML do Excel tem `<br>` sem
//! fechar e não passa num leitor de XML estrito.
use crate::objetos_aba::{Ancora, ControleFormulario, ObjetoSolto, TipoControleForm};

/// Teto de controles por aba (o resto é ignorado).
const MAX_CONTROLES: usize = 512;

/// O texto de `<x:Marca>…</x:Marca>` no trecho.
fn campo<'a>(trecho: &'a str, marca: &str) -> Option<&'a str> {
    let abre = format!("<x:{marca}>");
    let ini = trecho.find(&abre)? + abre.len();
    let fim = trecho[ini..].find("</x:")? + ini;
    Some(trecho[ini..fim].trim())
}

fn numero(trecho: &str, marca: &str) -> Option<i64> {
    campo(trecho, marca)?.parse().ok()
}

/// Entidades do XML e do HTML que o Excel grava no texto do controle.
fn desescapar(t: &str) -> String {
    let mut saida = String::with_capacity(t.len());
    let mut resto = t;
    while let Some(i) = resto.find('&') {
        saida.push_str(&resto[..i]);
        resto = &resto[i..];
        let Some(fim) = resto.find(';').filter(|&f| f <= 10) else { saida.push('&'); resto = &resto[1..]; continue };
        let ent = &resto[1..fim];
        let c = match ent {
            "amp" => Some('&'), "lt" => Some('<'), "gt" => Some('>'), "quot" => Some('"'), "apos" => Some('\''), "nbsp" => Some(' '),
            _ if ent.starts_with("#x") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
            _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match c {
            Some(c) => { saida.push(c); resto = &resto[fim + 1..]; }
            None => { saida.push('&'); resto = &resto[1..]; }
        }
    }
    saida.push_str(resto);
    saida
}

/// O texto visível do `<v:textbox>`: sem as marcas, `<br>` e `</div>` viram quebra de linha.
fn texto_da_caixa(forma: &str) -> Option<String> {
    let ini = forma.find("<v:textbox")?;
    let ini = forma[ini..].find('>')? + ini + 1;
    let fim = forma[ini..].find("</v:textbox>")? + ini;
    let mut saida = String::new();
    let mut dentro = false;
    let mut marca = String::new();
    for c in forma[ini..fim].chars() {
        match c {
            '<' => { dentro = true; marca.clear(); }
            '>' if dentro => {
                dentro = false;
                let m = marca.to_ascii_lowercase();
                if (m.starts_with("br") || m == "/div") && !saida.is_empty() && !saida.ends_with('\n') { saida.push('\n'); }
            }
            _ if dentro => marca.push(c),
            '\r' | '\n' => {}
            _ => saida.push(c),
        }
    }
    let t = desescapar(saida.trim());
    (!t.is_empty()).then_some(t)
}

/// Os controles de formulário da aba neste caminho do pacote (pelo VML ligado a ela).
pub(super) fn ler_da_aba(zip: &mut zip::ZipArchive<std::io::Cursor<std::sync::Arc<[u8]>>>, caminho_aba: &str) -> Vec<ObjetoSolto> {
    let (dir, arq) = caminho_aba.rsplit_once('/').unwrap_or(("", caminho_aba));
    let Ok(rels) = super::entry_text(zip, &format!("{dir}/_rels/{arq}.rels")).and_then(|t| super::xml::parse_xml(&t)) else { return Vec::new() };
    let vmls: Vec<String> = rels.children_named("Relationship")
        .filter(|r| r.attr("Type").is_some_and(|t| t.ends_with("/vmlDrawing")))
        .filter_map(|r| r.attr("Target")).map(|t| super::formas::caminho_relativo(caminho_aba, t)).collect();
    vmls.iter().filter_map(|p| super::entry_text(zip, p).ok()).flat_map(|vml| ler(&vml)).take(MAX_CONTROLES).collect()
}

/// Os controles de formulário do VML de uma aba, na ordem do arquivo.
pub(super) fn ler(vml: &str) -> Vec<ObjetoSolto> {
    let mut saida = Vec::new();
    // `<v:shape ` com espaço: `<v:shapetype` é o molde, não um controle.
    for forma in vml.split("<v:shape ").skip(1) {
        let forma = &forma[..forma.find("</v:shape>").unwrap_or(forma.len())];
        let Some(dados) = forma.find("<x:ClientData").map(|i| &forma[i..]) else { continue };
        let tipo = dados.split("ObjectType=\"").nth(1).and_then(|t| t.split('"').next()).unwrap_or("");
        let Some(tipo) = TipoControleForm::do_excel(tipo) else { continue };
        // Âncora: coluna, deslocamento (px), linha, deslocamento, e o mesmo para o canto oposto.
        let a: Vec<f32> = campo(dados, "Anchor").unwrap_or("").split(',').filter_map(|n| n.trim().parse().ok()).collect();
        if a.len() != 8 { continue; }
        let de = Ancora { coluna: a[0] as u32, dx: a[1], linha: a[2] as u32, dy: a[3] };
        let ate = Ancora { coluna: a[4] as u32, dx: a[5], linha: a[6] as u32, dy: a[7] };
        let mut o = ObjetoSolto::novo_controle(ControleFormulario::novo(tipo), de, ate);
        o.texto = texto_da_caixa(forma);
        // `<font size="280">`: vigésimos de ponto.
        let tamanho = forma.split("<font").nth(1).and_then(|f| f.split("size=\"").nth(1)).and_then(|t| t.split('"').next())
            .and_then(|t| t.parse::<f32>().ok()).map(|v| v / 20.0).filter(|v| (4.0..=96.0).contains(v));
        if let Some(t) = tamanho {
            o.fonte = Some(crate::objetos_aba::FonteTexto { tamanho: Some(t), ..Default::default() });
        }
        let mut c = ControleFormulario::novo(tipo);
        c.celula = campo(dados, "FmlaLink").filter(|v| !v.is_empty()).map(desescapar);
        c.faixa = campo(dados, "FmlaRange").filter(|v| !v.is_empty()).map(desescapar);
        c.valor = numero(dados, "Val").or_else(|| numero(dados, "Sel"));
        c.min = numero(dados, "Min");
        c.max = numero(dados, "Max");
        c.passo = numero(dados, "Inc");
        c.pagina = numero(dados, "Page");
        c.marcado = numero(dados, "Checked").map(|v| v.clamp(0, 2) as u8);
        c.linhas = numero(dados, "DropLines").map(|v| v.clamp(0, 1000) as u32);
        c.horizontal = dados.contains("<x:Horiz");
        c.macro_excel = campo(dados, "FmlaMacro").filter(|v| !v.is_empty()).map(desescapar);
        // Como nas formas: a macro pelo nome que o conversor dá (o fim de `[0]!Modulo1.Sub`).
        o.macro_nome = c.macro_excel.as_deref().map(|m| m.rsplit(['!', '.']).next().unwrap_or(m).to_string()).filter(|m| !m.is_empty());
        if let Some(nome) = forma.split("o:spid=\"").nth(1).or_else(|| forma.split("id=\"").nth(1)).and_then(|t| t.split('"').next()) {
            o.extras.insert("vml_id".into(), serde_json::Value::String(nome.to_string()));
        }
        o.controle = Some(c);
        saida.push(o);
        if saida.len() >= MAX_CONTROLES { break; }
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    const LOFA: &str = r##"<xml xmlns:v="urn:schemas-microsoft-com:vml"><v:shapetype id="_x0000_t201" coordsize="21600,21600" o:spt="201"/><v:shape id="_x0000_s12289" type="#_x0000_t201" o:button="t">
  <v:textbox style='mso-direction-alt:auto' o:singleclick="f">
   <div style='text-align:center'><font face="Aptos Narrow" size="280"
   color="#000000">Abrir &amp; Painel</font></div>
  </v:textbox>
  <x:ClientData ObjectType="Button">
   <x:Anchor>
    13, 0, 1, 3, 14, 110, 3, 27</x:Anchor>
   <x:FmlaMacro>[0]!AbrirPainel</x:FmlaMacro>
  </x:ClientData>
 </v:shape><v:shape id="_x0000_s10241" type="#_x0000_t201">
  <x:ClientData ObjectType="Spin">
   <x:Anchor>
    1, 4, 5, 2, 1, 66, 8, 0</x:Anchor>
   <x:FmlaLink>$C$8</x:FmlaLink>
   <x:Val>3810</x:Val>
   <x:Min>1</x:Min>
   <x:Max>30000</x:Max>
   <x:Inc>1</x:Inc>
   <x:Page>10</x:Page>
  </x:ClientData>
 </v:shape><v:shape id="_x0000_s1025" type="#_x0000_t202"><x:ClientData ObjectType="Note"><x:Row>0</x:Row></x:ClientData></v:shape></xml>"##;

    #[test]
    fn le_o_botao_e_o_botao_de_girar_do_lofa_e_pula_a_nota() {
        let c = ler(LOFA);
        assert_eq!(c.len(), 2);
        let botao = &c[0];
        assert_eq!(botao.texto.as_deref(), Some("Abrir & Painel"));
        assert_eq!(botao.macro_nome.as_deref(), Some("AbrirPainel"));
        assert_eq!((botao.de.coluna, botao.de.linha, botao.ate.coluna, botao.ate.dx), (13, 1, 14, 110.0));
        let girar = c[1].controle.as_ref().unwrap();
        assert_eq!(girar.tipo, TipoControleForm::Girar);
        assert_eq!((girar.celula.as_deref(), girar.valor, girar.min, girar.max), (Some("$C$8"), Some(3810), Some(1), Some(30000)));
        assert_eq!(girar.andar(1, false), 3811);
        assert_eq!(girar.andar(-10_000, false), 1);
        assert_eq!(girar.andar(1, true), 3820);
    }
}
