//! As ferramentas de planilha: ler, escrever, abas, nomes, arquivo e o que
//! age na tela. Os dados agregados (estatística, consulta, busca) estão em
//! [`crate::dados`]; macros em [`crate::macros`]; formatar e estrutura em
//! [`crate::formato`].
use std::path::Path;

use feathersheet_core::celulas::refs::{cell_name, Rect};
use feathersheet_core::celulas::style::format_value;
use feathersheet_core::celulas::values::Value;
use serde_json::{json, Value as Json};

use crate::hospede::{Hospede, SO_AO_VIVO};
use crate::texto::*;

/// Teto de células de uma leitura: o bastante para conferir, longe de despejar a planilha.
pub const MAX_LER: u64 = 2_000;
/// Teto de células de uma escrita numa chamada.
pub const MAX_ESCREVER: u64 = 200_000;

pub type Saida = Result<Vec<Json>, String>;

/// Chama a ferramenta `nome`. Erro volta como texto para a IA corrigir o pedido.
pub fn chamar(h: &mut dyn Hospede, nome: &str, args: &Json) -> Saida {
    match nome {
        "sheet_resumo" => resumo(h),
        "sheet_ler" => ler(h, args),
        "sheet_escrever" => escrever(h, args),
        "sheet_limpar" => limpar(h, args),
        "sheet_aba" => aba(h, args),
        "sheet_nomes" => nomes(h, args),
        "sheet_desfazer" => desfazer(h, args),
        "sheet_formula" => formula(h, args),
        "sheet_abrir" => {
            let caminho = arg_texto(args, "caminho").ok_or("Falta `caminho`.")?;
            Ok(conteudo(h.abrir(Path::new(caminho))?))
        }
        "sheet_salvar" => Ok(conteudo(h.salvar(arg_texto(args, "caminho").map(Path::new))?)),
        "sheet_selecionar" => {
            let (aba, area) = intervalo(h, args, "intervalo")?;
            h.definir_aba_ativa(aba);
            h.selecionar(area.clone())?;
            Ok(conteudo(format!("Selecionado {} em {}", nome_area(&area), h.pasta().nomes()[aba])))
        }
        "sheet_comando" => comando(h, args),
        "sheet_tela" => h.tela().map(conteudo).ok_or_else(|| SO_AO_VIVO.into()),
        "sheet_ver" | "sheet_mouse" => Err(SO_AO_VIVO.into()),
        "sheet_estatistica" => crate::dados::estatistica(h, args),
        "sheet_amostra" => crate::dados::amostra(h, args),
        "sheet_localizar" => crate::dados::localizar(h, args),
        "sheet_consulta" => crate::dados::consulta(h, args),
        "sheet_macro" => crate::macros::macro_(h, args),
        "sheet_formatar" => crate::formato::formatar(h, args),
        "sheet_estrutura" => crate::formato::estrutura(h, args),
        "sheet_mesclar" => crate::formato::mesclar(h, args),
        "sheet_ordenar" => crate::edicao::ordenar(h, args),
        "sheet_filtrar" => crate::edicao::filtrar(h, args),
        "sheet_substituir" => crate::edicao::substituir(h, args),
        "sheet_mover" => crate::edicao::mover(h, args),
        "sheet_copiar" => crate::edicao::copiar(h, args),
        "sheet_condicional" => crate::condicional::condicional(h, args),
        "sheet_validacao" => crate::validacao::validacao(h, args),
        "sheet_dinamica" => {
            let texto = h.dinamica(args)?;
            if !crate::permissoes::so_le("sheet_dinamica", args) {
                h.marcar_alterado();
            }
            Ok(conteudo(texto))
        }
        outro => Err(format!("Ferramenta desconhecida: {outro}")),
    }
}

fn resumo(h: &dyn Hospede) -> Saida {
    let pasta = h.pasta();
    let mut s = format!("Pasta: {}{}\nModo: {}\nCálculo: {}\n",
        h.caminho().map(|p| p.display().to_string()).unwrap_or_else(|| "(sem arquivo)".into()),
        if h.alterado() { " · não salva" } else { "" },
        if h.ao_vivo() { "ao vivo (aplicativo aberto)" } else { "arquivo (sem o aplicativo)" },
        if pasta.calculo_manual() { "manual (F9 calcula)" } else { "automático" });
    s.push_str("Abas:\n");
    for i in 0..pasta.len() {
        let Some(aba) = pasta.aba(i) else { continue };
        let usado = aba.sheet.used_rect();
        let area = if aba.sheet.is_empty() { "vazia".into() } else { nome_area(&Rect::new(0, 0, usado.r1, usado.c1)) };
        s.push_str(&format!("  {}{}: {area}, {} célula(s){}\n", if i == h.aba_ativa() { "* " } else { "  " },
            aba.sheet.name, aba.sheet.len(), if aba.sheet.oculta { " (oculta)" } else { "" }));
        // Cabeçalho provável: a primeira linha usada, quando é texto.
        if !aba.sheet.is_empty() && usado.c1 < 60 {
            let valores: Vec<Value> = (0..=usado.c1).map(|c| pasta.valor(i, 0, c)).collect();
            if valores.iter().any(|v| matches!(v, Value::Text(_))) {
                let titulos: Vec<String> = valores.iter().map(valor_texto).collect();
                s.push_str(&format!("      linha 1: {}\n", curto(&titulos.join(" | "), 300)));
            }
        }
    }
    let nomes = pasta.nomeados();
    if !nomes.is_empty() {
        s.push_str("Nomes definidos:\n");
        for n in nomes {
            s.push_str(&format!("  {} = {}{}\n", n.nome,
                n.area.sheet.as_deref().map(|a| format!("{a}!")).unwrap_or_default(), nome_area(&n.area)));
        }
    }
    if let Some(sel) = h.selecao() {
        s.push_str(&format!("Seleção na tela: {}\n", nome_area(&sel)));
    }
    Ok(conteudo(s))
}

fn ler(h: &dyn Hospede, args: &Json) -> Saida {
    let (aba, mut area) = intervalo(h, args, "intervalo")?;
    let pasta = h.pasta();
    let sheet = &pasta.aba(aba).ok_or("Aba inexistente")?.sheet;
    // Coluna ou linha inteira: só até a área usada.
    let usado = sheet.used_rect();
    area.r1 = area.r1.min(usado.r1.max(area.r0));
    area.c1 = area.c1.min(usado.c1.max(area.c0));
    if area.area() > MAX_LER {
        return Err(format!("{} tem {} células; o teto é {MAX_LER}. Leia um pedaço menor, ou use sheet_amostra/sheet_estatistica/sheet_consulta.",
            nome_area(&area), area.area()));
    }
    let modo = arg_texto(args, "modo").unwrap_or("valores");
    let corpo = tabela(&area, |r, c| {
        let bruto = sheet.get_raw(r, c);
        let formula = bruto.starts_with('=').then(|| formula_exibida(&bruto));
        let valor = || {
            let v = pasta.valor(aba, r, c);
            if modo == "formatado" { format_value(&v, sheet.estilo_resolvido(r, c).as_ref()) } else { valor_texto(&v) }
        };
        let texto = match (modo, formula) {
            ("formulas", Some(f)) => f,
            ("ambos", Some(f)) => format!("{} {{{f}}}", valor()),
            _ => valor(),
        };
        curto(&texto, 120)
    });
    Ok(conteudo(format!("{}!{} ({modo})\n{corpo}", sheet.name, nome_area(&area))))
}

/// Escreve numa transação por aba: um Ctrl+Z desfaz a chamada inteira.
fn escrever(h: &mut dyn Hospede, args: &Json) -> Saida {
    if let Some(formula) = args.get("formula") {
        return preencher_formula(h, args, formula);
    }
    let campo = if args.get("celula").is_some() { "celula" } else { "intervalo" };
    let (aba, inicio) = intervalo(h, args, campo)?;
    let linhas = args.get("valores").and_then(Json::as_array)
        .ok_or("Falta `valores`: lista de linhas, cada linha uma lista de células (ex.: [[\"Nome\", 10], [\"Ana\", \"=B1*2\"]]).")?;
    let total: u64 = linhas.iter().map(|l| l.as_array().map_or(1, Vec::len) as u64).sum();
    if total > MAX_ESCREVER {
        return Err(format!("{total} células numa chamada; o teto é {MAX_ESCREVER}. Divida em partes."));
    }
    let mut celulas: Vec<(u32, u32, String)> = Vec::new();
    for (i, linha) in linhas.iter().enumerate() {
        let itens: Vec<Json> = linha.as_array().cloned().unwrap_or_else(|| vec![linha.clone()]);
        for (j, v) in itens.iter().enumerate() {
            celulas.push((inicio.r0 + i as u32, inicio.c0 + j as u32, conteudo_da_ia(v)));
        }
    }
    let quantas = celulas.len();
    let fim = celulas.iter().fold((inicio.r0, inicio.c0), |(r, c), (lr, lc, _)| (r.max(*lr), c.max(*lc)));
    h.pasta_mut().editar(aba, |sheet| {
        sheet.abrir_transacao();
        for (r, c, texto) in &celulas {
            sheet.set_cell(*r, *c, texto);
        }
        sheet.fechar_transacao();
    });
    h.marcar_alterado();
    let area = Rect::new(inicio.r0, inicio.c0, fim.0, fim.1);
    Ok(conteudo(format!("{quantas} célula(s) escritas em {}!{}. Confira com sheet_ler (modo \"ambos\").",
        h.pasta().nomes()[aba], nome_area(&area))))
}

/// `formula` na primeira célula de `intervalo` e a alça de preenchimento no
/// resto: as referências relativas andam, como arrastar no aplicativo.
fn preencher_formula(h: &mut dyn Hospede, args: &Json, formula: &Json) -> Saida {
    use feathersheet_core::operacoes::fill::shift_formula;
    let (aba, area) = intervalo(h, args, "intervalo")?;
    let texto = conteudo_da_ia(formula);
    // Coluna vazia abaixo: em bloco pela coluna empacotada, sem teto — a
    // fórmula em 13,7 milhões de linhas leva milissegundos (17/10/2026).
    let em_bloco = area.r1 > area.r0 && texto.starts_with('=') && h.pasta().aba(aba).is_some_and(|a| {
        (area.c0..=area.c1).all(|c| {
            let segunda = shift_formula(&texto, 1, (c - area.c0) as i32);
            a.sheet.faixa_vazia(c, area.r0, area.r1) && a.sheet.formula_cabe_em_bloco(c, area.r0 + 1, &segunda)
        })
    });
    if em_bloco {
        h.pasta_mut().editar(aba, |sheet| {
            sheet.abrir_transacao();
            for c in area.c0..=area.c1 {
                let dc = (c - area.c0) as i32;
                sheet.set_cell(area.r0, c, &shift_formula(&texto, 0, dc));
                sheet.preencher_formula_faixa_vazia(c, area.r0 + 1, area.r1, &shift_formula(&texto, 1, dc));
            }
            sheet.fechar_transacao();
        });
        h.marcar_alterado();
        return Ok(conteudo(format!("Fórmula preenchida em {}!{} ({} célula(s), em bloco).",
            h.pasta().nomes()[aba], nome_area(&area), area.area())));
    }
    if area.area() > MAX_ESCREVER {
        return Err(format!("{} células; o teto é {MAX_ESCREVER} (sem teto quando o intervalo está vazio e a fórmula não é volátil nem lê a própria coluna).", area.area()));
    }
    let origem = Rect::new(area.r0, area.c0, area.r0, area.c0);
    let mut quantas = 1;
    h.pasta_mut().editar(aba, |sheet| {
        sheet.abrir_transacao();
        sheet.set_cell(area.r0, area.c0, &texto);
        if area.r1 > area.r0 {
            quantas += sheet.preencher(&origem, &Rect::new(area.r0, area.c0, area.r1, area.c0));
        }
        if area.c1 > area.c0 {
            let coluna = Rect::new(area.r0, area.c0, area.r1, area.c0);
            quantas += sheet.preencher(&coluna, &area);
        }
        sheet.fechar_transacao();
    });
    h.marcar_alterado();
    Ok(conteudo(format!("Fórmula preenchida em {}!{} ({quantas} célula(s)).", h.pasta().nomes()[aba], nome_area(&area))))
}

fn limpar(h: &mut dyn Hospede, args: &Json) -> Saida {
    let (aba, mut area) = intervalo(h, args, "intervalo")?;
    let usado = h.pasta().aba(aba).ok_or("Aba inexistente")?.sheet.used_rect();
    area.r1 = area.r1.min(usado.r1);
    area.c1 = area.c1.min(usado.c1);
    if area.r1 < area.r0 || area.c1 < area.c0 {
        return Ok(conteudo("Nada para limpar: o intervalo está fora da área usada."));
    }
    h.pasta_mut().editar(aba, |sheet| {
        sheet.abrir_transacao();
        for r in area.r0..=area.r1 {
            for c in area.c0..=area.c1 {
                if !sheet.get_raw(r, c).is_empty() { sheet.set_cell(r, c, ""); }
            }
        }
        sheet.fechar_transacao();
    });
    h.marcar_alterado();
    Ok(conteudo(format!("Conteúdo de {} limpo (o formato fica).", nome_area(&area))))
}

fn aba(h: &mut dyn Hospede, args: &Json) -> Saida {
    let acao = arg_texto(args, "acao").unwrap_or("listar");
    let nome = arg_texto(args, "nome");
    let indice = || indice_aba(h.pasta(), &Json::String(nome.unwrap_or("").into()));
    let texto = match acao {
        "listar" => {
            let nomes = h.pasta().nomes();
            nomes.iter().enumerate().map(|(i, n)| format!("{i}: {n}{}", if i == h.aba_ativa() { " (ativa)" } else { "" }))
                .collect::<Vec<_>>().join("\n")
        }
        "ativar" => { let i = indice()?; h.definir_aba_ativa(i); format!("Aba ativa: {}", h.pasta().nomes()[i]) }
        "criar" => {
            let nome = nome.ok_or("Falta `nome`.")?;
            if indice().is_ok() { return Err(format!("Já existe a aba \"{nome}\".")); }
            let i = h.pasta_mut().criar_aba(nome);
            h.definir_aba_ativa(i);
            h.marcar_alterado();
            format!("Aba \"{nome}\" criada e ativa")
        }
        "renomear" => {
            let i = indice()?;
            let novo = arg_texto(args, "novo_nome").ok_or("Falta `novo_nome`.")?;
            if !h.pasta_mut().renomear_aba(i, novo) { return Err(format!("Não consegui renomear para \"{novo}\" (nome repetido ou inválido).")); }
            h.marcar_alterado();
            format!("Aba renomeada para \"{novo}\"")
        }
        "excluir" => {
            let i = indice()?;
            if h.pasta().len() <= 1 { return Err("A pasta precisa de pelo menos uma aba.".into()); }
            h.pasta_mut().remover_aba(i);
            h.definir_aba_ativa(h.aba_ativa().min(h.pasta().len() - 1));
            h.marcar_alterado();
            "Aba excluída (Ctrl+Z desfaz)".into()
        }
        outra => return Err(format!("Ação desconhecida: {outra}. Use listar, ativar, criar, renomear ou excluir.")),
    };
    Ok(conteudo(texto))
}

fn nomes(h: &mut dyn Hospede, args: &Json) -> Saida {
    let acao = arg_texto(args, "acao").unwrap_or("listar");
    let texto = match acao {
        "listar" => {
            let lista: Vec<String> = h.pasta().nomeados().into_iter().map(|n| format!("{} = {}{}", n.nome,
                n.area.sheet.as_deref().map(|a| format!("{a}!")).unwrap_or_default(), nome_area(&n.area))).collect();
            if lista.is_empty() { "Nenhum nome definido.".into() } else { lista.join("\n") }
        }
        "definir" => {
            let nome = arg_texto(args, "nome").ok_or("Falta `nome`.")?;
            let (aba, mut area) = intervalo(h, args, "intervalo")?;
            area.sheet = Some(h.pasta().nomes()[aba].clone());
            let pasta = h.pasta_mut();
            pasta.remover_nome(nome);
            pasta.definir_nome(nome, area.clone());
            h.marcar_alterado();
            format!("{nome} = {}!{}", h.pasta().nomes()[aba], nome_area(&area))
        }
        "remover" => {
            let nome = arg_texto(args, "nome").ok_or("Falta `nome`.")?;
            if !h.pasta_mut().remover_nome(nome) { return Err(format!("Não existe o nome \"{nome}\".")); }
            h.marcar_alterado();
            format!("Nome \"{nome}\" removido")
        }
        outra => return Err(format!("Ação desconhecida: {outra}. Use listar, definir ou remover.")),
    };
    Ok(conteudo(texto))
}

fn desfazer(h: &mut dyn Hospede, args: &Json) -> Saida {
    let refazer = arg_bool(args, "refazer", false);
    let aba = aba_do_arg(h, args)?;
    let mut feito = false;
    h.pasta_mut().editar(aba, |sheet| { feito = if refazer { sheet.redo() } else { sheet.undo() }; });
    if feito { h.marcar_alterado(); }
    Ok(conteudo(match (feito, refazer) {
        (true, false) => "Desfeito o último passo da aba.",
        (true, true) => "Refeito.",
        (false, _) => "Nada para desfazer/refazer nesta aba.",
    }))
}

/// Tudo sobre uma célula: conteúdo, valor, quem ela lê e quem a lê, e o
/// caminho do erro quando há um.
fn formula(h: &dyn Hospede, args: &Json) -> Saida {
    let (aba, area) = intervalo(h, args, "celula")?;
    let (r, c) = (area.r0, area.c0);
    let pasta = h.pasta();
    let sheet = &pasta.aba(aba).ok_or("Aba inexistente")?.sheet;
    let bruto = sheet.get_raw(r, c);
    let valor = pasta.valor(aba, r, c);
    let mut s = format!("{}!{}\nconteúdo: {}\nvalor: {}\n", sheet.name, cell_name(r, c),
        if bruto.starts_with('=') { formula_exibida(&bruto) } else { bruto.clone() }, valor_texto(&valor));
    let precedentes = sheet.precedentes(r, c);
    if !precedentes.is_empty() {
        let lista: Vec<String> = precedentes.iter().map(|p| format!("{}{}",
            p.aba.as_deref().map(|a| format!("{a}!")).unwrap_or_default(), nome_area(&p.area))).collect();
        s.push_str(&format!("lê (precedentes): {}\n", lista.join(", ")));
    }
    let dependentes = sheet.dependentes(r, c);
    if !dependentes.is_empty() {
        let lista: Vec<String> = dependentes.iter().take(50).map(|&(r, c)| cell_name(r, c)).collect();
        s.push_str(&format!("é lida por (dependentes): {}{}\n", lista.join(", "),
            if dependentes.len() > 50 { format!(" e mais {}", dependentes.len() - 50) } else { String::new() }));
    }
    // Passo a passo, como Fórmulas → Avaliar fórmula (até 40 passos).
    if let Some(mut a) = feathersheet_core::formula::avaliar_passos::Avaliacao::nova(pasta, aba, r, c) {
        let mut textos = vec![a.estado(pasta).texto];
        while textos.len() <= 40 && a.avancar(pasta) {
            let t = a.estado(pasta).texto;
            if textos.last() != Some(&t) { textos.push(t); }
        }
        if textos.len() > 1 { s.push_str(&format!("passo a passo: {}\n", textos.join("  →  "))); }
    }
    if let Some(area) = sheet.area_derramada_em(r, c) {
        s.push_str(&format!("matriz derramada: {}\n", nome_area(&area)));
    }
    if matches!(valor, Value::Error(_)) {
        if let Some(e) = feathersheet_core::formula::explicar::explicar_erro(pasta, aba, r, c) {
            s.push_str(&format!("erro {}: {}\n{}\n", e.code, e.meaning, e.summary));
            for p in &e.steps {
                s.push_str(&format!("  {} {} → {}\n", p.cell, p.formula, p.culprit));
            }
            s.push_str(&format!("dica: {}\n", e.hint));
        }
    }
    Ok(conteudo(s))
}

fn comando(h: &mut dyn Hospede, args: &Json) -> Saida {
    if !h.ao_vivo() { return Err(SO_AO_VIVO.into()); }
    let nome = arg_texto(args, "comando").unwrap_or("listar");
    if nome == "listar" {
        let lista: Vec<String> = h.comandos().into_iter().map(|(n, d)| format!("{n}: {d}")).collect();
        return Ok(conteudo(lista.join("\n")));
    }
    let extra = args.get("args").cloned().unwrap_or_else(|| json!({}));
    let status = h.comando(nome, &extra)?;
    Ok(conteudo(format!("{nome}: feito. Status: {}", if status.is_empty() { "(vazio)" } else { &status })))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::hospede::Avulso;

    fn texto(r: Saida) -> String {
        r.unwrap()[0]["text"].as_str().unwrap().to_string()
    }

    #[test]
    fn preencher_formula_em_milhoes_de_linhas_vai_em_bloco() {
        let mut h = Avulso::default();
        h.pasta.escrever_celula(0, 0, 0, "4".into());
        let r = texto(preencher_formula(&mut h, &json!({ "intervalo": "B1:C2000000" }), &json!("=A1*2")));
        assert!(r.contains("em bloco"), "{r}");
        let s = &h.pasta.aba(0).unwrap().sheet;
        assert_eq!(s.get_raw(1_999_999, 1), "=A2000000*2");
        assert_eq!(s.get_raw(1_999_999, 2), "=B2000000*2");
        assert_eq!(h.pasta.valor(0, 0, 2), feathersheet_core::Value::Number(16.));
    }

    #[test]
    fn escrever_ler_e_desfazer_numa_chamada() {
        let mut h = Avulso::default();
        let r = escrever(&mut h, &json!({ "celula": "A1", "valores": [["Item", "Valor"], ["a", 1.5], ["b", "=B2*2"]] }));
        assert!(texto(r).contains("6 célula(s)"));
        let lido = texto(ler(&h, &json!({ "intervalo": "A1:B3", "modo": "ambos" })));
        assert!(lido.contains("1.5") && lido.contains("3 {=B2*2}"), "{lido}");
        texto(desfazer(&mut h, &json!({})));
        assert!(h.pasta.aba(0).unwrap().sheet.is_empty(), "um Ctrl+Z desfaz a chamada inteira");
    }

    #[test]
    fn formula_em_ptbr_e_preenchimento_relativo() {
        let mut h = Avulso::default();
        escrever(&mut h, &json!({ "celula": "A1", "valores": [[1], [2], [3]] })).unwrap();
        preencher_formula(&mut h, &json!({ "intervalo": "B1:B3" }), &json!("=A1*1,5")).unwrap();
        assert_eq!(h.pasta.valor(0, 2, 1), Value::Number(4.5));
        escrever(&mut h, &json!({ "celula": "C1", "valores": [["=SOMA(A1:A3;10)"]] })).unwrap();
        assert_eq!(h.pasta.valor(0, 0, 2), Value::Number(16.));
        let f = texto(formula(&h, &json!({ "celula": "C1" })));
        assert!(f.contains("=SOMA(A1:A3;10)") && f.contains("lê (precedentes): A1:A3"), "{f}");
        assert!(f.contains("passo a passo: =SOMA(A1:A3;10)  →  =16"), "{f}");
    }

    #[test]
    fn ler_tem_teto_e_abas_e_nomes_funcionam() {
        let mut h = Avulso::default();
        escrever(&mut h, &json!({ "celula": "A1", "valores": [[1]] })).unwrap();
        escrever(&mut h, &json!({ "celula": "A3000", "valores": [[1]] })).unwrap();
        assert!(ler(&h, &json!({ "intervalo": "A:B" })).unwrap_err().contains("teto"));
        texto(aba(&mut h, &json!({ "acao": "criar", "nome": "Dados" })));
        assert_eq!(h.aba_ativa(), 1);
        escrever(&mut h, &json!({ "celula": "Planilha1!B1", "valores": [["=Dados!A1+1"]] })).unwrap();
        texto(nomes(&mut h, &json!({ "acao": "definir", "nome": "Base", "intervalo": "Planilha1!A1:A3" })));
        assert!(texto(resumo(&h)).contains("Base = Planilha1!A1:A3"));
        assert!(chamar(&mut h, "sheet_ver", &json!({})).unwrap_err().contains("abra o FeatherSheet"));
    }

    #[test]
    fn abrir_e_salvar_fshz() {
        let pasta = std::env::temp_dir().join(format!("fsheet_mcp_{}", std::process::id()));
        std::fs::create_dir_all(&pasta).unwrap();
        let arquivo = pasta.join("t.fshz");
        let mut h = Avulso::default();
        escrever(&mut h, &json!({ "celula": "A1", "valores": [[2, "=A1*3"]] })).unwrap();
        texto(chamar(&mut h, "sheet_salvar", &json!({ "caminho": arquivo.to_str() })));
        let mut outro = Avulso::default();
        texto(chamar(&mut outro, "sheet_abrir", &json!({ "caminho": arquivo.to_str() })));
        assert_eq!(outro.pasta.valor(0, 0, 1), Value::Number(6.));
        let _ = std::fs::remove_dir_all(&pasta);
    }
}
