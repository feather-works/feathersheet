//! Formatar e mexer na estrutura: `sheet_formatar`, `sheet_estrutura` e
//! `sheet_mesclar`. O pedido vira o mesmo `EstiloParcial` da janela Formatar
//! células e as mesmas operações do motor que a faixa usa; cada chamada é
//! uma transação (um Ctrl+Z), menos inserir/excluir, que o motor não desfaz.
use std::collections::BTreeMap;

use feathersheet_core::celulas::estilo_parcial::{Campo, EstiloParcial};
use feathersheet_core::celulas::refs::{col_name, Rect, MAX_COLS, MAX_ROWS};
use feathersheet_core::operacoes::autoajuste;
use feathersheet_io::mesclas;
use serde_json::Value as Json;

use crate::ferramentas::{Saida, MAX_ESCREVER};
use crate::hospede::Hospede;
use crate::texto::*;

/// Largura de coluna vazia e altura mínima de linha, em pixels (as da grade).
const LARGURA_PADRAO: u32 = 122;
const ALTURA_PADRAO: u32 = 28;

const FORMATOS: &[(&str, &str)] = &[
    ("numero", "number"), ("inteiro", "integer"), ("moeda", "currency"), ("contabil", "accounting"),
    ("porcentagem", "percent"), ("cientifico", "scientific"), ("data", "date"), ("data_hora", "datetime"),
    ("hora", "time"), ("duracao", "duration"), ("texto", "text"),
];

/// `"#1a2b3c"`, `"#abc"` ou nada (`null`, `""`, `"nenhuma"`) para tirar a cor.
fn cor(v: &Json, campo: &str) -> Result<Campo, String> {
    let t = match v {
        Json::Null => return Ok(Campo::Nulo),
        Json::String(t) => t.trim(),
        _ => return Err(format!("`{campo}`: use \"#RRGGBB\" ou null.")),
    };
    if t.is_empty() || t.eq_ignore_ascii_case("nenhuma") { return Ok(Campo::Nulo); }
    let hex = t.trim_start_matches('#');
    if !(hex.len() == 3 || hex.len() == 6) || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("`{campo}`: \"{t}\" não é cor; use \"#RRGGBB\" (ex.: \"#d93025\")."));
    }
    Ok(Campo::Texto(format!("#{}", hex.to_ascii_lowercase())))
}

fn numero(args: &Json, campo: &str, min: f64, max: f64) -> Result<Option<f64>, String> {
    match args.get(campo) {
        None => Ok(None),
        Some(v) => {
            let n = v.as_f64().ok_or(format!("`{campo}` precisa ser número."))?;
            if !(min..=max).contains(&n) { return Err(format!("`{campo}` vai de {min} a {max}.")); }
            Ok(Some(n))
        }
    }
}

/// O pedido de formatação como o patch do motor (sem as bordas, que dependem
/// da posição de cada célula).
fn patch_do_pedido(args: &Json) -> Result<EstiloParcial, String> {
    let mut p = EstiloParcial::new();
    if args.get("limpar").and_then(Json::as_bool) == Some(true) {
        for k in ["bold", "italic", "underline", "strike", "wrap", "fontSize", "fontFamily", "decimals",
            "borders", "align", "vAlign", "format", "color", "bg", "rotation", "indent", "textVertical"] {
            p.insert(k.into(), Campo::Nulo);
        }
    }
    for (campo, chave) in [("negrito", "bold"), ("italico", "italic"), ("sublinhado", "underline"),
        ("tachado", "strike"), ("quebrar", "wrap")] {
        if let Some(v) = args.get(campo) {
            let b = v.as_bool().ok_or(format!("`{campo}` precisa ser true ou false."))?;
            p.insert(chave.into(), Campo::Logico(b));
        }
    }
    if let Some(f) = args.get("fonte") {
        p.insert("fontFamily".into(), match f.as_str().map(str::trim) {
            None | Some("") => Campo::Nulo,
            Some(nome) => Campo::Texto(nome.into()),
        });
    }
    if let Some(n) = numero(args, "tamanho", 6., 96.)? { p.insert("fontSize".into(), Campo::Numero(n.round())); }
    if let Some(v) = args.get("cor") { p.insert("color".into(), cor(v, "cor")?); }
    if let Some(v) = args.get("fundo") { p.insert("bg".into(), cor(v, "fundo")?); }
    if let Some(a) = arg_texto(args, "alinhamento") {
        p.insert("align".into(), match a {
            "esquerda" => Campo::Texto("left".into()),
            "centro" => Campo::Texto("center".into()),
            "direita" => Campo::Texto("right".into()),
            "auto" | "geral" => Campo::Nulo,
            outro => return Err(format!("alinhamento \"{outro}\": use esquerda, centro, direita ou auto.")),
        });
    }
    if let Some(a) = arg_texto(args, "vertical") {
        p.insert("vAlign".into(), match a {
            "topo" => Campo::Texto("top".into()),
            "base" => Campo::Texto("bottom".into()),
            "meio" => Campo::Nulo,
            outro => return Err(format!("vertical \"{outro}\": use topo, meio ou base.")),
        });
    }
    if let Some(n) = numero(args, "rotacao", -90., 90.)? {
        p.insert("rotation".into(), if n == 0. { Campo::Nulo } else { Campo::Numero(n.round()) });
    }
    if let Some(n) = numero(args, "recuo", 0., 15.)? {
        p.insert("indent".into(), if n == 0. { Campo::Nulo } else { Campo::Numero(n.round()) });
    }
    if let Some(f) = arg_texto(args, "formato") {
        p.insert("format".into(), match f {
            "geral" | "auto" => Campo::Nulo,
            _ => {
                let nome = FORMATOS.iter().find(|(pt, _)| *pt == f).map(|(_, en)| *en).ok_or_else(|| format!(
                    "formato \"{f}\": use geral, {}. Formato personalizado (código) ainda não existe no motor.",
                    FORMATOS.iter().map(|(pt, _)| *pt).collect::<Vec<_>>().join(", ")))?;
                Campo::Texto(nome.into())
            }
        });
    }
    if let Some(n) = numero(args, "casas", 0., 30.)? { p.insert("decimals".into(), Campo::Numero(n.round())); }
    Ok(p)
}

/// Os lados que cada célula de `area` recebe para a opção de borda pedida
/// (vazio = a célula não muda). Os de fora seguem a beira da seleção, como
/// no Excel: "superior" é só a linha de cima.
fn lados(opcao: &str, area: &Rect, r: u32, c: u32) -> Vec<&'static str> {
    let beira = [("top", r == area.r0), ("right", c == area.c1), ("bottom", r == area.r1), ("left", c == area.c0)];
    beira.into_iter().filter(|&(lado, na_beira)| na_beira && match opcao {
        "externas" => true,
        "superior" => lado == "top",
        "inferior" => lado == "bottom",
        "esquerda" => lado == "left",
        "direita" => lado == "right",
        _ => false,
    }).map(|(lado, _)| lado).collect()
}

pub fn formatar(h: &mut dyn Hospede, args: &Json) -> Saida {
    let (aba, area) = intervalo(h, args, "intervalo")?;
    let patch = patch_do_pedido(args)?;
    let bordas = arg_texto(args, "bordas");
    if let Some(b) = bordas {
        if !["todas", "externas", "nenhuma", "superior", "inferior", "esquerda", "direita"].contains(&b) {
            return Err(format!("bordas \"{b}\": use todas, externas, nenhuma, superior, inferior, esquerda ou direita."));
        }
    }
    let cor_borda = match args.get("cor_borda") {
        Some(v) => match cor(v, "cor_borda")? { Campo::Texto(t) => Some(t), _ => None },
        None => None,
    };
    if patch.is_empty() && bordas.is_none() {
        return Err("Nada para formatar: informe ao menos um campo (negrito, cor, fundo, formato, bordas…).".into());
    }
    // Bordas que não são "todas"/"nenhuma" mexem célula a célula (só a beira
    // da seleção); a beira de colunas inteiras seria grande demais.
    let por_celula = bordas.is_some_and(|b| !matches!(b, "todas" | "nenhuma"));
    let beira = 2 * (area.r1 - area.r0 + 1) as u64 + 2 * (area.c1 - area.c0 + 1) as u64;
    if por_celula && beira > MAX_ESCREVER {
        return Err(format!("A beira de {} tem {beira} células; para bordas, use um intervalo menor (até a área usada).", nome_area(&area)));
    }
    h.pasta_mut().editar(aba, |sheet| {
        sheet.abrir_transacao();
        if !patch.is_empty() { sheet.set_style(&area, &patch); }
        match bordas {
            Some("nenhuma") => {
                let p = EstiloParcial::from([("borders".to_string(), Campo::Nulo)]);
                sheet.set_style(&area, &p);
            }
            Some("todas") => {
                let mut o: BTreeMap<String, Campo> = ["top", "right", "bottom", "left"].iter()
                    .map(|l| (l.to_string(), Campo::Logico(true))).collect();
                if let Some(c) = &cor_borda { o.insert("color".into(), Campo::Texto(c.clone())); }
                sheet.set_style(&area, &EstiloParcial::from([("borders".to_string(), Campo::Objeto(o))]));
            }
            Some(opcao) => {
                // Só a beira: soma os lados novos aos que a célula já tinha.
                let mut celulas: Vec<(u32, u32)> = Vec::new();
                for c in area.c0..=area.c1 { celulas.push((area.r0, c)); celulas.push((area.r1, c)); }
                for r in area.r0..=area.r1 { celulas.push((r, area.c0)); celulas.push((r, area.c1)); }
                celulas.sort_unstable();
                celulas.dedup();
                for (r, c) in celulas {
                    let novos = lados(opcao, &area, r, c);
                    if novos.is_empty() { continue; }
                    let mut o = match sheet.estilo_mesclado(r, c).and_then(|e| e.get("borders").cloned()) {
                        Some(Campo::Objeto(o)) => o,
                        _ => BTreeMap::new(),
                    };
                    for l in novos { o.insert(l.to_string(), Campo::Logico(true)); }
                    if let Some(cb) = &cor_borda { o.insert("color".into(), Campo::Texto(cb.clone())); }
                    sheet.set_style(&Rect::cell(r, c), &EstiloParcial::from([("borders".to_string(), Campo::Objeto(o))]));
                }
            }
            None => {}
        }
        sheet.fechar_transacao();
    });
    h.marcar_alterado();
    let mut feito: Vec<&str> = patch.keys().map(String::as_str).collect();
    if bordas.is_some() { feito.push("borders"); }
    Ok(conteudo(format!("Formatado {}!{} ({}). Um Ctrl+Z desfaz. Confira com sheet_ler modo \"formatado\"{}.",
        h.pasta().nomes()[aba], nome_area(&area), feito.join(", "),
        if h.ao_vivo() { " ou sheet_ver" } else { "" })))
}

fn eixo(coluna: bool, a: u32, b: u32) -> String {
    if coluna { format!("{}:{}", col_name(a), col_name(b)) } else { format!("{}:{}", a + 1, b + 1) }
}

/// Linhas ou colunas do intervalo: "3:5", "B:D", "A1:C4" (usa as linhas ou
/// as colunas dele, pelo `eixo`).
fn faixa(area: &Rect, coluna: bool) -> (u32, u32) {
    if coluna { (area.c0, area.c1) } else { (area.r0, area.r1) }
}

pub fn estrutura(h: &mut dyn Hospede, args: &Json) -> Saida {
    let acao = arg_texto(args, "acao").ok_or("Falta `acao`.")?;
    let coluna = match arg_texto(args, "eixo") {
        Some("colunas" | "coluna") => true,
        Some("linhas" | "linha") => false,
        Some(outro) => return Err(format!("eixo \"{outro}\": use linhas ou colunas.")),
        None => return Err("Falta `eixo`: linhas ou colunas.".into()),
    };
    let (aba, area) = intervalo(h, args, "intervalo")?;
    // Aba protegida: inserir/excluir não entram no desfazer, então o portão do `chamar` não os vê; confere antes.
    {
        use feathersheet_core::planilha::sheet::protecao::{Permissao as P, Recusa};
        let precisa = match (acao, coluna) {
            ("inserir", true) => Some(P::InserirColunas), ("inserir", false) => Some(P::InserirLinhas),
            ("excluir", true) => Some(P::ExcluirColunas), ("excluir", false) => Some(P::ExcluirLinhas),
            (_, true) => Some(P::FormatarColunas), (_, false) => Some(P::FormatarLinhas),
        };
        if let Some(p) = precisa.filter(|&p| h.pasta().aba(aba).is_some_and(|x| x.sheet.proibe(p))) {
            return Err(format!("{} (aba protegida; desproteja com sheet_proteger).", Recusa::sem_permissao(p).0));
        }
    }
    let (a, b) = faixa(&area, coluna);
    let quantas = b - a + 1;
    let nome = eixo(coluna, a, b);
    let texto = match acao {
        "inserir" | "excluir" => {
            let removendo = acao == "excluir";
            if !h.reflui_tabelas() {
                let tem = h.pasta().aba(aba).and_then(|x| x.sheet.tb.as_deref())
                    .is_some_and(|tb| !matches!(tb.trim(), "" | "[]"));
                if tem {
                    return Err("Esta aba tem tabelas/AutoFiltro, que só o aplicativo sabe ajustar ao inserir/excluir. Abra o FeatherSheet e chame de novo (ao vivo).".into());
                }
            }
            let limite = if coluna { MAX_COLS } else { MAX_ROWS };
            if !removendo {
                let usado = h.pasta().aba(aba).map(|x| x.sheet.used_rect());
                let ultimo = usado.map(|u| if coluna { u.c1 } else { u.r1 }).unwrap_or(0);
                if ultimo.saturating_add(quantas) >= limite {
                    return Err("Não cabe: os dados empurrados passariam do fim da planilha.".into());
                }
            }
            let pasta = h.pasta_mut();
            let feito = match (coluna, removendo) {
                (true, true) => pasta.excluir_colunas(aba, a, quantas),
                (true, false) => pasta.inserir_colunas(aba, a, quantas),
                (false, true) => pasta.excluir_linhas(aba, a, quantas),
                (false, false) => pasta.inserir_linhas(aba, a, quantas),
            };
            if feito.is_none() { return Err(format!("Não consegui {acao} {nome}.")); }
            let aviso = h.refluir_estrutura(aba, coluna, a, quantas, removendo).err()
                .map(|e| format!(" Atenção: {e}")).unwrap_or_default();
            h.marcar_alterado();
            format!("{} {quantas} {} em {nome}; as fórmulas foram ajustadas (o que apontava para o excluído vira #REF!). \
                Isto não tem Ctrl+Z: para voltar, faça a operação contrária.{aviso}",
                if removendo { "Excluídas" } else { "Inseridas" }, if coluna { "coluna(s)" } else { "linha(s)" })
        }
        "ocultar" | "reexibir" => {
            let ocultar = acao == "ocultar";
            let mut r = Ok(0);
            h.pasta_mut().editar(aba, |s| { r = s.alterar_visibilidade_dimensoes(coluna, ocultar, a, b); });
            let n = r?;
            if n > 0 { h.marcar_alterado(); }
            match (n, ocultar) {
                (0, true) => format!("{nome} já estava(m) oculta(s)."),
                (0, false) => format!("Nada oculto em {nome}."),
                (n, true) => format!("{n} oculta(s) em {nome}. Um Ctrl+Z desfaz."),
                (n, false) => format!("{n} reexibida(s) perto de {nome}. Um Ctrl+Z desfaz."),
            }
        }
        "tamanho" => {
            let px = arg_num(args, "pixels", 0) as u32;
            if !(1..=2000).contains(&px) {
                return Err("Informe `pixels` de 1 a 2000 (largura da coluna ou altura da linha). Para ocultar use acao=ocultar.".into());
            }
            if quantas as u64 > MAX_ESCREVER { return Err("Intervalo grande demais; use até a área usada.".into()); }
            let mut mudou = false;
            h.pasta_mut().editar(aba, |s| {
                s.abrir_transacao();
                for i in a..=b { mudou |= s.definir_dimensao(coluna, i, px); }
                s.fechar_transacao();
            });
            if mudou { h.marcar_alterado(); }
            format!("{} de {nome}: {px} px. Um Ctrl+Z desfaz.", if coluna { "Largura" } else { "Altura" })
        }
        "autoajustar" => {
            let m = h.pasta().aba(aba).map(|x| mesclas::ler(&x.sheet)).unwrap_or_default();
            let valores = if coluna {
                autoajuste::larguras_ideais(h.pasta(), aba, a, b, LARGURA_PADRAO, &m)
            } else {
                autoajuste::alturas_ideais(h.pasta(), aba, a, b, ALTURA_PADRAO, &m)
            };
            let mut mudou = false;
            h.pasta_mut().editar(aba, |s| {
                s.abrir_transacao();
                for &(i, px) in &valores { mudou |= s.definir_dimensao(coluna, i, px); }
                s.fechar_transacao();
            });
            if mudou { h.marcar_alterado(); }
            let lista: Vec<String> = valores.iter().take(12)
                .map(|&(i, px)| format!("{}={px}", if coluna { col_name(i) } else { (i + 1).to_string() })).collect();
            format!("{} ajustada(s) ao conteúdo: {}{}. Um Ctrl+Z desfaz.", valores.len(), lista.join(", "),
                if valores.len() > 12 { " …" } else { "" })
        }
        outra => return Err(format!("Ação desconhecida: {outra}. Use inserir, excluir, ocultar, reexibir, tamanho ou autoajustar.")),
    };
    Ok(conteudo(texto))
}

pub fn mesclar(h: &mut dyn Hospede, args: &Json) -> Saida {
    let acao = arg_texto(args, "acao").unwrap_or("mesclar");
    let (aba, area) = intervalo(h, args, "intervalo")?;
    if acao != "desmesclar" && area.area() < 2 {
        return Err("Selecione duas ou mais células para mesclar.".into());
    }
    if acao == "atraves" && area.c0 == area.c1 {
        return Err("Mesclar através precisa de mais de uma coluna.".into());
    }
    if acao == "atraves" && area.r1 - area.r0 >= 10_000 {
        return Err("Mesclar através aceita até 10000 linhas.".into());
    }
    if !["mesclar", "centralizar", "atraves", "desmesclar"].contains(&acao) {
        return Err(format!("Ação desconhecida: {acao}. Use mesclar, centralizar, atraves ou desmesclar."));
    }
    let mut mudou = false;
    h.pasta_mut().editar(aba, |s| {
        s.abrir_transacao();
        match acao {
            "desmesclar" => mudou = mesclas::desmesclar(s, area.clone()),
            "atraves" => {
                // Mescla antiga de várias linhas engoliria as novas: desfaz antes, como o Excel.
                mudou |= mesclas::desmesclar(s, area.clone());
                for r in area.r0..=area.r1 {
                    mudou |= mesclas::mesclar(s, Rect::new(r, area.c0, r, area.c1));
                }
            }
            _ => {
                mudou = mesclas::mesclar(s, area.clone());
                if mudou && acao == "centralizar" {
                    s.set_style(&area, &EstiloParcial::from([("align".to_string(), Campo::Texto("center".into()))]));
                }
            }
        }
        s.fechar_transacao();
    });
    if !mudou {
        return Ok(conteudo(if acao == "desmesclar" { "Não há células mescladas no intervalo." } else { "O intervalo já estava mesclado assim." }));
    }
    h.marcar_alterado();
    let regioes = mesclas::ler(&h.pasta().aba(aba).ok_or("Aba inexistente")?.sheet);
    let lista: Vec<String> = regioes.iter().take(20).map(nome_area).collect();
    Ok(conteudo(format!("{} em {}. Um Ctrl+Z desfaz. Mesclagens da aba: {}{}",
        match acao { "desmesclar" => "Desmesclado", "centralizar" => "Mesclado e centralizado", "atraves" => "Mesclado através", _ => "Mesclado" },
        nome_area(&area), if lista.is_empty() { "nenhuma".into() } else { lista.join(", ") },
        if regioes.len() > 20 { " …" } else { "" })))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::ferramentas::chamar;
    use crate::hospede::Avulso;
    use feathersheet_core::celulas::style::NumberFormat;
    use feathersheet_core::celulas::values::Value;
    use serde_json::json;

    fn ok(h: &mut Avulso, nome: &str, args: Json) -> String {
        match chamar(h, nome, &args) {
            Ok(v) => v[0]["text"].as_str().unwrap().to_string(),
            Err(e) => panic!("{nome} falhou: {e}"),
        }
    }

    fn estilo(h: &Avulso, r: u32, c: u32) -> feathersheet_core::celulas::style::Style {
        h.pasta.aba(0).unwrap().sheet.estilo_resolvido(r, c).unwrap_or_default()
    }

    #[test]
    fn formatar_fonte_cores_numero_e_desfazer() {
        let mut h = Avulso::default();
        ok(&mut h, "sheet_escrever", json!({ "celula": "A1", "valores": [["Total", 1234.5]] }));
        ok(&mut h, "sheet_formatar", json!({ "intervalo": "A1:B1", "negrito": true, "cor": "#D93025", "fundo": "#fff2b2",
            "alinhamento": "centro", "tamanho": 14, "fonte": "Arial" }));
        ok(&mut h, "sheet_formatar", json!({ "intervalo": "B1", "formato": "moeda", "casas": 2 }));
        let s = estilo(&h, 0, 1);
        assert!(s.bold && s.font_size == Some(14.) && s.color.as_deref() == Some("#d93025") && s.bg.as_deref() == Some("#fff2b2"));
        assert_eq!(s.format, NumberFormat::Currency);
        let lido = ok(&mut h, "sheet_ler", json!({ "intervalo": "B1", "modo": "formatado" }));
        assert!(lido.contains("R$"), "{lido}");
        ok(&mut h, "sheet_desfazer", json!({}));
        assert_eq!(estilo(&h, 0, 1).format, NumberFormat::Auto, "um Ctrl+Z desfaz a chamada");
        assert!(estilo(&h, 0, 1).bold, "a chamada anterior continua");
        assert!(chamar(&mut h, "sheet_formatar", &json!({ "intervalo": "A1", "cor": "vermelho" })).unwrap_err().contains("#RRGGBB"));
        assert!(chamar(&mut h, "sheet_formatar", &json!({ "intervalo": "A1" })).unwrap_err().contains("Nada para formatar"));
    }

    #[test]
    fn bordas_externas_so_na_beira_e_somam_as_existentes() {
        let mut h = Avulso::default();
        ok(&mut h, "sheet_formatar", json!({ "intervalo": "B2", "bordas": "superior" }));
        ok(&mut h, "sheet_formatar", json!({ "intervalo": "B2:D4", "bordas": "externas", "cor_borda": "#0b57d0" }));
        let b = |r, c| estilo(&h, r, c).borders.unwrap_or_default();
        let canto = b(1, 1);
        assert!(canto.top && canto.left && !canto.right && !canto.bottom);
        assert_eq!(canto.color.as_deref(), Some("#0b57d0"));
        assert!(estilo(&h, 2, 2).borders.is_none(), "o miolo não ganha borda");
        let fundo = b(3, 3);
        assert!(fundo.bottom && fundo.right && !fundo.top);
        ok(&mut h, "sheet_formatar", json!({ "intervalo": "B2:D4", "bordas": "nenhuma" }));
        assert!(estilo(&h, 1, 1).borders.is_none());
    }

    #[test]
    fn estrutura_insere_ajusta_formula_e_reflui_mescla() {
        let mut h = Avulso::default();
        ok(&mut h, "sheet_escrever", json!({ "celula": "A1", "valores": [[1], [2], ["=SOMA(A1:A2)"]] }));
        ok(&mut h, "sheet_mesclar", json!({ "intervalo": "B1:C1", "acao": "centralizar" }));
        let r = ok(&mut h, "sheet_estrutura", json!({ "acao": "inserir", "eixo": "linhas", "intervalo": "1:2" }));
        assert!(r.contains("Inseridas 2 linha(s)"), "{r}");
        let f = formula_exibida(&h.pasta.aba(0).unwrap().sheet.get_raw(4, 0));
        assert_eq!(f, "=SOMA(A3:A4)");
        assert_eq!(h.pasta.valor(0, 4, 0), Value::Number(3.));
        assert_eq!(mesclas::ler(&h.pasta.aba(0).unwrap().sheet), vec![Rect::new(2, 1, 2, 2)]);
        assert_eq!(estilo(&h, 2, 1).align, feathersheet_core::celulas::style::Align::Center);
        ok(&mut h, "sheet_estrutura", json!({ "acao": "excluir", "eixo": "colunas", "intervalo": "C:C" }));
        assert!(mesclas::ler(&h.pasta.aba(0).unwrap().sheet).is_empty(), "mescla de uma coluna só some");
    }

    #[test]
    fn estrutura_tamanho_ocultar_e_autoajuste() {
        let mut h = Avulso::default();
        ok(&mut h, "sheet_escrever", json!({ "celula": "A1", "valores": [["um texto bem comprido para a coluna crescer"]] }));
        ok(&mut h, "sheet_estrutura", json!({ "acao": "autoajustar", "eixo": "colunas", "intervalo": "A:B" }));
        let larg = |h: &Avulso, c| h.pasta.aba(0).unwrap().sheet.col_widths.get(&c).copied();
        assert!(larg(&h, 0).unwrap() > LARGURA_PADRAO);
        ok(&mut h, "sheet_estrutura", json!({ "acao": "tamanho", "eixo": "colunas", "intervalo": "C:D", "pixels": 80 }));
        assert_eq!((larg(&h, 2), larg(&h, 3)), (Some(80), Some(80)));
        ok(&mut h, "sheet_desfazer", json!({}));
        assert_eq!(larg(&h, 2), None, "um Ctrl+Z volta as duas");
        ok(&mut h, "sheet_estrutura", json!({ "acao": "ocultar", "eixo": "linhas", "intervalo": "2:3" }));
        assert_eq!(h.pasta.aba(0).unwrap().sheet.row_heights.get(&1), Some(&0));
        ok(&mut h, "sheet_estrutura", json!({ "acao": "reexibir", "eixo": "linhas", "intervalo": "2:2" }));
        assert!(h.pasta.aba(0).unwrap().sheet.row_heights.is_empty());
    }

    #[test]
    fn sem_o_app_recusa_inserir_em_aba_com_tabela() {
        let mut h = Avulso::default();
        h.pasta.aba_mut(0).unwrap().sheet.tb = Some(r#"[{"id":"t1","nome":"Tabela1","area":{"r0":0,"c0":0,"r1":3,"c1":1}}]"#.into());
        let e = chamar(&mut h, "sheet_estrutura", &json!({ "acao": "inserir", "eixo": "linhas", "intervalo": "2:2" })).unwrap_err();
        assert!(e.contains("tabelas"), "{e}");
    }
}
