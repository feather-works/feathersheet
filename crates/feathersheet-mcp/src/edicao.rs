//! Ordenar, filtrar, substituir, mover e copiar (com as opções do Colar
//! especial). Cada chamada que escreve é um Ctrl+Z na aba que mudou; tudo
//! roda no motor, nada de dados vai para a IA além do resumo do que mudou.
use feathersheet_core::celulas::refs::{cell_name, col_name, parse_range_ref, Rect};
use feathersheet_core::operacoes::colagem::{combinar, OperacaoColagem};
use feathersheet_core::operacoes::fill::shift_formula;
use feathersheet_core::operacoes::ops::{find_all_com, replace_all_em, SearchOptions};
use feathersheet_core::operacoes::ordenacao_ext::{ordenar_por_linhas, ChaveOrdem, CriterioOrdem};
use feathersheet_core::planilha::sheet::edicao::Snap;
use feathersheet_core::planilha::sheet::Sheet;
use serde_json::Value as Json;

use crate::ferramentas::{Saida, MAX_ESCREVER};
use crate::hospede::Hospede;
use crate::texto::*;

/// O intervalo até a área usada (coluna ou linha inteira não vira milhões).
fn ate_a_area_usada(sheet: &Sheet, mut area: Rect) -> Rect {
    let usado = sheet.used_rect();
    area.r1 = area.r1.min(usado.r1.max(area.r0));
    area.c1 = area.c1.min(usado.c1.max(area.c0));
    area
}

/// A coluna de uma chave: o título do cabeçalho (sem ligar para maiúsculas)
/// ou a letra da coluna; tem de cair dentro do intervalo.
fn coluna_da_chave(sheet: &Sheet, area: &Rect, cabecalho: bool, chave: &Json) -> Result<u32, String> {
    let texto = match chave {
        Json::String(s) => s.trim().to_string(),
        Json::Object(o) => o.get("coluna").and_then(Json::as_str).unwrap_or("").trim().to_string(),
        _ => String::new(),
    };
    if texto.is_empty() {
        return Err("`coluna` precisa ser texto: a letra (\"B\") ou o título do cabeçalho; número não vale.".into());
    }
    if cabecalho {
        let alvo = texto.to_lowercase();
        if let Some(c) = (area.c0..=area.c1).find(|&c| valor_texto(&sheet.get_value(area.r0, c)).trim().to_lowercase() == alvo) {
            return Ok(c);
        }
    }
    let c = parse_range_ref(&format!("{texto}1")).map(|r| r.c0)
        .ok_or_else(|| format!("Coluna \"{texto}\" não encontrada: use a letra (B) ou o título do cabeçalho."))?;
    if c < area.c0 || c > area.c1 {
        return Err(format!("A coluna {} está fora de {}.", col_name(c), nome_area(area)));
    }
    Ok(c)
}

/// Classifica as linhas de um intervalo por uma ou mais chaves.
pub fn ordenar(h: &mut dyn Hospede, args: &Json) -> Saida {
    let (aba, area) = intervalo(h, args, "intervalo")?;
    let cabecalho = arg_bool(args, "cabecalho", true);
    let maiusculas = arg_bool(args, "maiusculas", false);
    let sheet = &h.pasta().aba(aba).ok_or("Aba inexistente")?.sheet;
    let area = ate_a_area_usada(sheet, area);
    let lista = args.get("chaves").and_then(Json::as_array).cloned().unwrap_or_default();
    let mut chaves = Vec::new();
    let mut descricao = Vec::new();
    for chave in &lista {
        let indice = coluna_da_chave(sheet, &area, cabecalho, chave)?;
        let decrescente = chave.get("decrescente").and_then(Json::as_bool).unwrap_or(false);
        let criterio = match chave.get("lista").and_then(Json::as_array) {
            Some(itens) => CriterioOrdem::Lista(itens.iter().map(|v| v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string())).collect()),
            None => CriterioOrdem::Valor,
        };
        descricao.push(format!("{} {}", col_name(indice), if decrescente { "decrescente" } else { "crescente" }));
        chaves.push(ChaveOrdem { indice, ascendente: !decrescente, criterio });
    }
    if chaves.is_empty() {
        chaves.push(ChaveOrdem { indice: area.c0, ascendente: true, criterio: CriterioOrdem::Valor });
        descricao.push(format!("{} crescente", col_name(area.c0)));
    }
    let mut mudou = false;
    h.pasta_mut().editar(aba, |s| mudou = ordenar_por_linhas(s, area.clone(), &chaves, cabecalho, maiusculas));
    if !mudou {
        return Ok(conteudo(format!("{} já estava nessa ordem (ou não tem linhas para ordenar).", nome_area(&area))));
    }
    h.marcar_alterado();
    Ok(conteudo(format!("{}!{} ordenado por {}{}. Um Ctrl+Z desfaz.", h.pasta().nomes()[aba], nome_area(&area),
        descricao.join(", então "), if cabecalho { " (a 1ª linha é cabeçalho e ficou no lugar)" } else { "" })))
}

/// Dados → Remover duplicadas: o plano e a remoção moram no motor (`ops::planejar_repetidas`).
pub fn duplicados(h: &mut dyn Hospede, args: &Json) -> Saida {
    let (aba, area) = intervalo(h, args, "intervalo")?;
    let cabecalho = arg_bool(args, "cabecalho", true);
    let sheet = &h.pasta().aba(aba).ok_or("Aba inexistente")?.sheet;
    let area = ate_a_area_usada(sheet, area);
    let colunas = match args.get("colunas").and_then(Json::as_array) {
        Some(lista) if !lista.is_empty() => Some(lista.iter()
            .map(|c| coluna_da_chave(sheet, &area, cabecalho, &serde_json::json!({ "coluna": c })))
            .collect::<Result<Vec<u32>, String>>()?),
        _ => None,
    };
    let pasta = h.pasta();
    let plano = feathersheet_core::operacoes::ops::planejar_repetidas(area.clone(), cabecalho, colunas.as_deref(),
        area.r1, |r, c| pasta.valor(aba, r, c));
    let quais = colunas.as_ref().map_or("todas as colunas".to_string(),
        |c| c.iter().map(|&i| col_name(i)).collect::<Vec<_>>().join(", "));
    let Some(plano) = plano.filter(|p| p.removidas > 0) else {
        return Ok(conteudo(format!("Nenhuma linha duplicada em {} (comparando {quais}).", nome_area(&area))));
    };
    let n = plano.removidas;
    if arg_bool(args, "contar", false) {
        return Ok(conteudo(format!("{n} linha(s) duplicada(s) sairiam de {} (comparando {quais}); nada mudou.", nome_area(&area))));
    }
    h.pasta_mut().editar(aba, |s| feathersheet_core::operacoes::ops::aplicar_repetidas(s, area.c0, area.c1, &plano));
    h.marcar_alterado();
    Ok(conteudo(format!("{n} linha(s) duplicada(s) removida(s) de {} (comparando {quais}). Um Ctrl+Z desfaz.", nome_area(&area))))
}

/// Atingir meta: a busca mora no motor (`operacoes::atingir_meta::buscar`); cada tentativa
/// escreve sem histórico e só o valor achado entra no Ctrl+Z, como na janela do app.
pub fn atingir_meta(h: &mut dyn Hospede, args: &Json) -> Saida {
    use feathersheet_core::celulas::number::to_js_string;
    use feathersheet_core::celulas::input::numero_para_entrada;
    use feathersheet_core::celulas::values::Value;
    use feathersheet_core::operacoes::atingir_meta::{buscar, ITERACOES, TOLERANCIA};
    use feathersheet_core::planilha::sheet::edicao::Snap;
    let (aba, d) = intervalo(h, args, "definir")?;
    let (aba_a, a) = intervalo(h, args, "alterando")?;
    if aba != aba_a { return Err("`definir` e `alterando` precisam estar na mesma aba.".into()); }
    let alvo = args.get("para").and_then(Json::as_f64).ok_or("Falta `para` (o número a atingir).")?;
    let ((dr, dc), (ar, ac)) = ((d.r0, d.c0), (a.r0, a.c0));
    let sheet = &h.pasta().aba(aba).ok_or("Aba inexistente")?.sheet;
    if !sheet.get_raw(dr, dc).starts_with('=') { return Err(format!("{} precisa conter uma fórmula.", cell_name(dr, dc))); }
    if sheet.get_raw(ar, ac).starts_with('=') { return Err(format!("{} precisa conter um número, não uma fórmula.", cell_name(ar, ac))); }
    let original = sheet.snapshot(ar, ac);
    let estilo = original.as_ref().and_then(|s| s.style.clone());
    let inicio = match h.pasta().valor(aba, ar, ac) {
        Value::Number(n) => n,
        Value::Empty => 0.0,
        _ => return Err(format!("{} precisa conter um número.", cell_name(ar, ac))),
    };
    let pasta = h.pasta_mut();
    let busca = buscar(|x| {
        let snap = Snap { raw: numero_para_entrada(x), style: estilo.clone() };
        pasta.editar(aba, |s| { s.apply(ar, ac, Some(snap), false); });
        match pasta.valor(aba, dr, dc) { Value::Number(n) => Some(n), _ => None }
    }, inicio, alvo, ITERACOES, TOLERANCIA);
    // Volta ao original sem histórico; o valor achado entra com histórico.
    pasta.editar(aba, |s| { s.apply(ar, ac, original.clone(), false); });
    let Some(b) = busca else {
        return Err(format!("{} dá erro com o valor atual de {}.", cell_name(dr, dc), cell_name(ar, ac)));
    };
    if !b.convergiu {
        return Ok(conteudo(format!("Não achei em {} tentativas: o melhor foi {} = {} (com {} = {}). Nada mudou.",
            b.tentativas, cell_name(ar, ac), to_js_string(b.x), cell_name(dr, dc), to_js_string(b.y))));
    }
    pasta.escrever_celula(aba, ar, ac, numero_para_entrada(b.x));
    h.marcar_alterado();
    Ok(conteudo(format!("{} = {} faz {} dar {} ({} tentativa(s)). Um Ctrl+Z volta ao valor de antes.",
        cell_name(ar, ac), to_js_string(b.x), cell_name(dr, dc), to_js_string(b.y), b.tentativas)))
}

/// Dados → Consolidar: a conta mora no motor (`Workbook::consolidar`).
pub fn consolidar(h: &mut dyn Hospede, args: &Json) -> Saida {
    let textos: Vec<String> = args.get("origens").and_then(Json::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect()).unwrap_or_default();
    if textos.is_empty() { return Err("Falta `origens` (ex.: [\"Jan!A1:C20\", \"Fev!A1:C20\"]).".into()); }
    let mut origens = Vec::with_capacity(textos.len());
    for t in &textos {
        let (aba, area) = intervalo(h, &serde_json::json!({ "r": t }), "r")?;
        let sheet = &h.pasta().aba(aba).ok_or("Aba inexistente")?.sheet;
        origens.push(Rect { sheet: Some(h.pasta().nomes()[aba].clone()), ..ate_a_area_usada(sheet, area) });
    }
    let (aba, canto) = intervalo(h, args, "destino")?;
    let op = feathersheet_core::dinamica::pivot::AgregacaoOp::do_str(arg_texto(args, "conta").unwrap_or("soma"));
    let (topo, esquerda) = (arg_bool(args, "rotulos_topo", false), arg_bool(args, "rotulos_esquerda", false));
    let (alt, larg) = h.pasta_mut().consolidar(&origens, op, topo, esquerda, aba, canto.r0, canto.c0)?;
    h.marcar_alterado();
    let fim = cell_name(canto.r0 + alt - 1, canto.c0 + larg - 1);
    Ok(conteudo(format!("{} intervalo(s) consolidado(s) em {}!{}:{fim} ({alt} × {larg}). Um Ctrl+Z desfaz.",
        origens.len(), h.pasta().nomes()[aba], cell_name(canto.r0, canto.c0))))
}

/// AutoFiltro: ligar com um critério numa coluna, limpar, remover ou listar.
/// A regra do filtro (tabelas e AutoFiltro) mora no aplicativo.
pub fn filtrar(h: &mut dyn Hospede, args: &Json) -> Saida {
    let acao = arg_texto(args, "acao").unwrap_or("aplicar");
    let (aba, area) = intervalo(h, args, "intervalo")?;
    let coluna = match args.get("coluna") {
        Some(c) if !c.is_null() => {
            let sheet = &h.pasta().aba(aba).ok_or("Aba inexistente")?.sheet;
            Some(coluna_da_chave(sheet, &area, true, c)?)
        }
        _ => None,
    };
    let criterio = args.get("criterio").filter(|c| !c.is_null()).cloned();
    if acao == "aplicar" && (coluna.is_none() || criterio.is_none()) {
        return Err("Para aplicar, informe `coluna` e `criterio` (ex.: [\"SP\", \"RJ\"], \">100\", {\"op\":\"contains\",\"val\":\"x\"}).".into());
    }
    let texto = h.filtrar(aba, area, acao, coluna, criterio)?;
    if acao != "listar" {
        h.marcar_alterado();
    }
    Ok(conteudo(texto))
}

/// Substitui texto nas células (como o Ctrl+H), no intervalo, na aba ou
/// em todas. Um Ctrl+Z por aba mexida.
pub fn substituir(h: &mut dyn Hospede, args: &Json) -> Saida {
    let busca = arg_texto(args, "texto").ok_or("Falta `texto` (o que procurar).")?;
    let por = args.get("por").and_then(Json::as_str).ok_or("Falta `por` (pode ser \"\" para apagar o texto).")?;
    let opcoes = SearchOptions {
        match_case: arg_bool(args, "maiusculas", false),
        whole_cell: arg_bool(args, "inteira", false),
        in_formulas: arg_bool(args, "formulas", false),
    };
    let (abas, area): (Vec<usize>, Option<Rect>) = if arg_texto(args, "intervalo").is_some() {
        let (aba, area) = intervalo(h, args, "intervalo")?;
        (vec![aba], Some(area))
    } else if arg_bool(args, "todas_as_abas", false) {
        ((0..h.pasta().len()).collect(), None)
    } else {
        (vec![aba_do_arg(h, args)?], None)
    };
    let mut total = 0;
    let mut exemplos = Vec::new();
    for aba in abas {
        let achadas: Vec<_> = {
            let pasta = h.pasta();
            let sheet = &pasta.aba(aba).ok_or("Aba inexistente")?.sheet;
            find_all_com(sheet, busca, opcoes, |r, c| pasta.valor(aba, r, c)).into_iter()
                .filter(|m| area.as_ref().is_none_or(|a| a.contains(m.row, m.col))).collect()
        };
        if achadas.is_empty() {
            continue;
        }
        let nome = h.pasta().nomes()[aba].clone();
        let mut quantas = 0;
        h.pasta_mut().editar(aba, |s| quantas = replace_all_em(s, &achadas, busca, por, opcoes));
        total += quantas;
        for m in achadas.iter().take(20usize.saturating_sub(exemplos.len())) {
            exemplos.push(format!("{nome}!{}", cell_name(m.row, m.col)));
        }
    }
    if total == 0 {
        return Ok(conteudo(format!("Nada para substituir: \"{busca}\" não aparece{}.", if opcoes.in_formulas { "" } else { " (nos valores; com formulas=true procura no texto das fórmulas)" })));
    }
    h.marcar_alterado();
    Ok(conteudo(format!("{total} célula(s) com \"{busca}\" → \"{por}\": {}{}. Um Ctrl+Z por aba desfaz.",
        exemplos.join(", "), if total > exemplos.len() { ", …" } else { "" })))
}

/// Move um bloco na mesma aba, como arrastar pela borda: as fórmulas que
/// apontam para ele (nesta e nas outras abas) passam a apontar para o destino.
pub fn mover(h: &mut dyn Hospede, args: &Json) -> Saida {
    let (aba, origem) = intervalo(h, args, "origem")?;
    let (aba_destino, destino) = intervalo(h, args, "destino")?;
    if aba_destino != aba {
        return Err("sheet_mover move dentro da mesma aba. Para levar a outra aba, use sheet_copiar e depois sheet_limpar na origem.".into());
    }
    let dr = destino.r0 as i64 - origem.r0 as i64;
    let dc = destino.c0 as i64 - origem.c0 as i64;
    if dr == 0 && dc == 0 {
        return Ok(conteudo("O destino é a própria origem: nada a mover."));
    }
    h.pasta_mut().mover_intervalo(aba, origem.clone(), dr as i32, dc as i32)
        .ok_or("O destino sairia da planilha.")?;
    h.marcar_alterado();
    let novo = Rect::new((origem.r0 as i64 + dr) as u32, (origem.c0 as i64 + dc) as u32,
        (origem.r1 as i64 + dr) as u32, (origem.c1 as i64 + dc) as u32);
    Ok(conteudo(format!("{} movido para {} em {}; as fórmulas que liam a origem foram ajustadas. Um Ctrl+Z desfaz.",
        nome_area(&origem), nome_area(&novo), h.pasta().nomes()[aba])))
}

/// O que colar, como o Colar especial.
#[derive(Clone, Copy, PartialEq)]
enum Modo { Tudo, Valores, Formatos, Formulas, ValoresEFormatos }

/// Copia um bloco (de qualquer aba para qualquer aba) com as opções do
/// Colar especial: o que colar, transpor, operação e pular vazias.
pub fn copiar(h: &mut dyn Hospede, args: &Json) -> Saida {
    let (aba_origem, origem) = intervalo(h, args, "origem")?;
    let (aba_destino, destino) = intervalo(h, args, "destino")?;
    let modo = match arg_texto(args, "colar").unwrap_or("tudo") {
        "tudo" => Modo::Tudo,
        "valores" => Modo::Valores,
        "formatos" => Modo::Formatos,
        "formulas" => Modo::Formulas,
        "valores_e_formatos" => Modo::ValoresEFormatos,
        outro => return Err(format!("`colar` desconhecido: {outro}. Use tudo, valores, formatos, formulas ou valores_e_formatos.")),
    };
    let operacao = match arg_texto(args, "operacao").unwrap_or("nenhuma") {
        "nenhuma" => OperacaoColagem::Nenhuma,
        "somar" => OperacaoColagem::Somar,
        "subtrair" => OperacaoColagem::Subtrair,
        "multiplicar" => OperacaoColagem::Multiplicar,
        "dividir" => OperacaoColagem::Dividir,
        outro => return Err(format!("`operacao` desconhecida: {outro}. Use nenhuma, somar, subtrair, multiplicar ou dividir.")),
    };
    let transpor = arg_bool(args, "transpor", false);
    let pular_vazias = arg_bool(args, "pular_vazias", false);

    // 1) a foto da origem, antes de escrever (origem e destino podem se cruzar).
    let pasta = h.pasta();
    let fonte = &pasta.aba(aba_origem).ok_or("Aba inexistente")?.sheet;
    let origem = ate_a_area_usada(fonte, origem);
    if origem.area() > MAX_ESCREVER {
        return Err(format!("{} tem {} células; o teto é {MAX_ESCREVER} por chamada. Copie em partes.", nome_area(&origem), origem.area()));
    }
    // Vizinha de matriz derramada sem a âncora na cópia vai como valor (Excel).
    let derramada_sem_ancora = |r: u32, c: u32| fonte.area_derramada_em(r, c)
        .is_some_and(|a| (a.r0, a.c0) != (r, c) && !origem.contains(a.r0, a.c0));
    struct Foto { dr: u32, dc: u32, r: u32, c: u32, raw: String, valor: String, estilo: Option<feathersheet_core::celulas::estilo_parcial::EstiloParcial> }
    let mut fotos = Vec::with_capacity(origem.area() as usize);
    for r in origem.r0..=origem.r1 {
        for c in origem.c0..=origem.c1 {
            let valor = valor_texto(&pasta.valor(aba_origem, r, c));
            let raw = if derramada_sem_ancora(r, c) { valor.clone() } else { fonte.get_raw(r, c) };
            let (i, j) = (r - origem.r0, c - origem.c0);
            let (dr, dc) = if transpor { (j, i) } else { (i, j) };
            fotos.push(Foto { dr, dc, r, c, raw, valor, estilo: fonte.estilo_mesclado(r, c) });
        }
    }
    let (altura, largura) = if transpor { (origem.c1 - origem.c0, origem.r1 - origem.r0) } else { (origem.r1 - origem.r0, origem.c1 - origem.c0) };
    let fim = Rect::new(destino.r0, destino.c0, destino.r0.saturating_add(altura), destino.c0.saturating_add(largura));
    use feathersheet_core::celulas::refs::{MAX_COLS, MAX_ROWS};
    if fim.r1 >= MAX_ROWS || fim.c1 >= MAX_COLS {
        return Err("A cópia passaria do fim da planilha.".into());
    }

    // 2) a escrita, num Ctrl+Z da aba de destino.
    let mut escritas = 0;
    h.pasta_mut().editar(aba_destino, |s| {
        s.abrir_transacao();
        for f in fotos {
            if pular_vazias && f.raw.is_empty() {
                continue;
            }
            let (rd, cd) = (destino.r0 + f.dr, destino.c0 + f.dc);
            let atual = s.snapshot(rd, cd);
            let raw_atual = atual.as_ref().map(|a| a.raw.clone()).unwrap_or_default();
            let estilo_atual = atual.and_then(|a| a.style);
            let deslocada = || if f.raw.starts_with('=') {
                shift_formula(&f.raw, rd as i32 - f.r as i32, cd as i32 - f.c as i32)
            } else { f.raw.clone() };
            let (mut raw, estilo) = match modo {
                Modo::Tudo => (deslocada(), f.estilo),
                Modo::Formulas => (deslocada(), estilo_atual),
                Modo::Valores => (f.valor, estilo_atual),
                Modo::ValoresEFormatos => (f.valor, f.estilo),
                Modo::Formatos => (raw_atual.clone(), f.estilo),
            };
            if modo != Modo::Formatos && operacao != OperacaoColagem::Nenhuma {
                match combinar(&raw_atual, &raw, operacao) {
                    Some(novo) => raw = novo,
                    // Texto, lógico ou erro de um dos lados: a célula fica.
                    None => continue,
                }
            }
            let proxima = (!raw.is_empty() || estilo.is_some()).then(|| Snap { raw, style: estilo });
            s.apply(rd, cd, proxima, true);
            escritas += 1;
        }
        s.fechar_transacao();
    });
    h.marcar_alterado();
    let nomes = h.pasta().nomes();
    Ok(conteudo(format!("{} de {}!{} colado(s) em {}!{} ({}{}{}{}). Um Ctrl+Z desfaz.",
        escritas, nomes[aba_origem], nome_area(&origem), nomes[aba_destino], nome_area(&fim),
        match modo { Modo::Tudo => "tudo", Modo::Valores => "só valores", Modo::Formatos => "só formatos",
            Modo::Formulas => "só fórmulas", Modo::ValoresEFormatos => "valores e formatos" },
        if transpor { ", transposto" } else { "" },
        match operacao { OperacaoColagem::Nenhuma => "", OperacaoColagem::Somar => ", somando",
            OperacaoColagem::Subtrair => ", subtraindo", OperacaoColagem::Multiplicar => ", multiplicando",
            OperacaoColagem::Dividir => ", dividindo" },
        if pular_vazias { ", pulando vazias" } else { "" })))
}

#[cfg(test)]
mod testes {
    use crate::ferramentas::chamar;
    use crate::hospede::Avulso;
    use feathersheet_core::celulas::values::Value;
    use serde_json::json;

    fn ok(h: &mut Avulso, nome: &str, args: serde_json::Value) -> String {
        match chamar(h, nome, &args) {
            Ok(c) => c[0]["text"].as_str().unwrap().to_string(),
            Err(e) => panic!("{nome}: {e}"),
        }
    }

    fn pasta_exemplo() -> Avulso {
        let mut h = Avulso::default();
        ok(&mut h, "sheet_escrever", json!({ "celula": "A1", "valores": [
            ["Nome", "UF", "Valor"], ["Bia", "SP", 30], ["Ana", "RJ", 10], ["Caio", "SP", 20], ["Davi", "MG", 40]] }));
        h
    }

    fn coluna(h: &Avulso, c: u32) -> Vec<String> {
        (0..5).map(|r| crate::texto::valor_texto(&h.pasta.valor(0, r, c))).collect()
    }

    #[test]
    fn consolidar_duas_abas_pelos_rotulos() {
        let mut h = pasta_exemplo();
        ok(&mut h, "sheet_aba", json!({ "acao": "criar", "nome": "Fev" }));
        ok(&mut h, "sheet_escrever", json!({ "celula": "Fev!A1", "valores": [["Nome", "UF", "Valor"], ["Bia", "SP", 5]] }));
        let t = ok(&mut h, "sheet_consolidar", json!({ "origens": ["Planilha1!A1:C5", "Fev!A1:C2"], "destino": "Planilha1!F1",
            "rotulos_topo": true, "rotulos_esquerda": true }));
        assert!(t.contains("F1:H5"), "{t}");
        assert_eq!(crate::texto::valor_texto(&h.pasta.valor(0, 1, 7)), "35", "Bia: 30 + 5");
        assert!(chamar(&mut h, "sheet_consolidar", &json!({ "origens": ["Fev!A1:C2"], "destino": "Planilha1!F1" })).is_err(), "não escreve por cima");
    }

    #[test]
    fn ordenar_por_titulo_e_por_letra_e_desfazer() {
        let mut h = pasta_exemplo();
        let t = ok(&mut h, "sheet_ordenar", json!({ "intervalo": "A:C", "chaves": [{ "coluna": "uf" }, { "coluna": "C", "decrescente": true }] }));
        assert!(t.contains("A1:C5"), "{t}");
        assert_eq!(coluna(&h, 0), ["Nome", "Davi", "Ana", "Bia", "Caio"]);
        ok(&mut h, "sheet_desfazer", json!({}));
        assert_eq!(coluna(&h, 0), ["Nome", "Bia", "Ana", "Caio", "Davi"], "um Ctrl+Z desfaz a ordenação");
        // Lista personalizada.
        ok(&mut h, "sheet_ordenar", json!({ "intervalo": "A1:C5", "chaves": [{ "coluna": "B", "lista": ["MG", "SP", "RJ"] }] }));
        assert_eq!(coluna(&h, 1), ["UF", "MG", "SP", "SP", "RJ"]);
        assert!(chamar(&mut h, "sheet_ordenar", &json!({ "intervalo": "A1:C5", "chaves": [{ "coluna": "Z" }] })).is_err());
    }

    #[test]
    fn substituir_no_intervalo_e_na_aba() {
        let mut h = pasta_exemplo();
        let t = ok(&mut h, "sheet_substituir", json!({ "texto": "SP", "por": "São Paulo", "intervalo": "B1:B3" }));
        assert!(t.starts_with("1 célula"), "{t}");
        assert_eq!(coluna(&h, 1), ["UF", "São Paulo", "RJ", "SP", "MG"]);
        ok(&mut h, "sheet_substituir", json!({ "texto": "SP", "por": "São Paulo", "inteira": true }));
        assert_eq!(coluna(&h, 1)[3], "São Paulo");
        assert!(ok(&mut h, "sheet_substituir", json!({ "texto": "XYZ", "por": "" })).starts_with("Nada"));
    }

    #[test]
    fn mover_ajusta_formulas_e_copiar_com_colar_especial() {
        let mut h = pasta_exemplo();
        ok(&mut h, "sheet_escrever", json!({ "celula": "E1", "valores": [["=SOMA(C2:C5)"]] }));
        ok(&mut h, "sheet_mover", json!({ "origem": "C1:C5", "destino": "D1" }));
        assert_eq!(h.pasta.aba(0).unwrap().sheet.get_raw(0, 4), "=SOMA(D2:D5)", "a fórmula segue o bloco");
        assert_eq!(h.pasta.valor(0, 0, 4), Value::Number(100.0));
        // Copiar tudo para outra aba, transposto.
        ok(&mut h, "sheet_aba", json!({ "acao": "criar", "nome": "Resumo" }));
        ok(&mut h, "sheet_copiar", json!({ "origem": "Planilha1!A1:A3", "destino": "Resumo!B2", "transpor": true }));
        let resumo = h.pasta.aba(1).unwrap();
        assert_eq!((resumo.sheet.get_raw(1, 1), resumo.sheet.get_raw(1, 3)), ("Nome".into(), "Ana".into()));
        // Só valores: a fórmula vira o resultado.
        ok(&mut h, "sheet_copiar", json!({ "origem": "Planilha1!E1", "destino": "Resumo!A1", "colar": "valores" }));
        assert_eq!(h.pasta.aba(1).unwrap().sheet.get_raw(0, 0), "100");
        // Operação: somar 100 em cima de 100.
        ok(&mut h, "sheet_copiar", json!({ "origem": "Planilha1!E1", "destino": "Resumo!A1", "colar": "valores", "operacao": "somar" }));
        assert_eq!(h.pasta.valor(1, 0, 0), Value::Number(200.0));
        // Pular vazias não apaga o destino.
        ok(&mut h, "sheet_copiar", json!({ "origem": "Planilha1!G1", "destino": "Resumo!A1", "pular_vazias": true }));
        assert_eq!(h.pasta.valor(1, 0, 0), Value::Number(200.0));
        assert!(chamar(&mut h, "sheet_mover", &json!({ "origem": "Planilha1!A1", "destino": "Resumo!A1" })).is_err());
    }

    #[test]
    fn filtrar_sem_o_aplicativo_explica() {
        let mut h = pasta_exemplo();
        let erro = chamar(&mut h, "sheet_filtrar", &json!({ "intervalo": "A1:C5", "coluna": "UF", "criterio": ["SP"] })).unwrap_err();
        assert!(erro.contains("aplicativo") && erro.contains("sheet_consulta"), "{erro}");
        assert!(crate::permissoes::so_le("sheet_filtrar", &json!({ "acao": "listar" })));
        assert!(!crate::permissoes::so_le("sheet_filtrar", &json!({ "acao": "aplicar" })));
    }

    #[test]
    fn duplicados_conta_e_remove_pela_coluna_escolhida() {
        let mut h = pasta_exemplo();
        let t = ok(&mut h, "sheet_duplicados", json!({ "intervalo": "A:C", "colunas": ["UF"], "contar": true }));
        assert!(t.contains("1 linha(s)") && t.contains("nada mudou"), "{t}");
        assert_eq!(coluna(&h, 0), ["Nome", "Bia", "Ana", "Caio", "Davi"]);
        let t = ok(&mut h, "sheet_duplicados", json!({ "intervalo": "A:C", "colunas": ["B"] }));
        assert!(t.contains("removida"), "{t}");
        assert_eq!(coluna(&h, 0), ["Nome", "Bia", "Ana", "Davi", ""], "o segundo SP (Caio) sai");
        let t = ok(&mut h, "sheet_duplicados", json!({ "intervalo": "A1:C5" }));
        assert!(t.contains("Nenhuma"), "{t}");
        assert!(crate::permissoes::so_le("sheet_duplicados", &json!({ "contar": true })));
    }

    #[test]
    fn atingir_meta_acha_o_valor_e_grava() {
        let mut h = Avulso::default();
        ok(&mut h, "sheet_escrever", json!({ "celula": "A1", "valores": [[10], ["=A1*3+5"]] }));
        let t = ok(&mut h, "sheet_atingir_meta", json!({ "definir": "A2", "para": 50, "alterando": "A1" }));
        assert!(t.contains("A1 = 15"), "{t}");
        let Value::Number(y) = h.pasta.valor(0, 1, 0) else { panic!("A2 sem número") };
        assert!((y - 50.0).abs() <= 0.001, "{y}");
        // Impossível (o quadrado nunca dá negativo): nada muda.
        ok(&mut h, "sheet_escrever", json!({ "celula": "B1", "valores": [[2], ["=B1*B1"]] }));
        let t = ok(&mut h, "sheet_atingir_meta", json!({ "definir": "B2", "para": -4, "alterando": "B1" }));
        assert!(t.contains("Nada mudou"), "{t}");
        assert_eq!(crate::texto::valor_texto(&h.pasta.valor(0, 0, 1)), "2");
        assert!(chamar(&mut h, "sheet_atingir_meta", &json!({ "definir": "A1", "para": 1, "alterando": "A2" })).is_err());
    }
}
