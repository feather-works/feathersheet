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
///
/// Portão da proteção, como o da tela (`protecao_ui::vigiar_protecao`): o que a chamada deixou no desfazer de uma
/// aba protegida é examinado pelo motor e, se a proteção não deixa, desfeito, e a IA recebe o erro (antes escrevia
/// em célula bloqueada no modo avulso, e ao vivo ouvia "feito" de algo que a tela desfazia).
pub fn chamar(h: &mut dyn Hospede, nome: &str, args: &Json) -> Saida {
    if crate::permissoes::so_le(nome, args) || matches!(nome, "sheet_proteger" | "sheet_desfazer") {
        return despachar(h, nome, args);
    }
    let marcas: Vec<(u32, usize)> = (0..h.pasta().len()).filter_map(|i| h.pasta().aba(i))
        .filter(|a| a.sheet.protegida()).map(|a| (a.id, a.sheet.profundidade_do_historico())).collect();
    let estrutura = h.pasta().propriedades.protecao_estrutura.is_some().then(|| {
        (0..h.pasta().len()).filter_map(|i| h.pasta().aba(i)).map(|a| (a.id, a.sheet.name.clone())).collect::<Vec<_>>()
    });
    let saida = despachar(h, nome, args);
    // Pasta protegida: aba nova (consulta com destino novo, dinâmica…) sai e nome trocado volta.
    if let Some(antes) = estrutura {
        let depois: Vec<(u32, String)> = (0..h.pasta().len()).filter_map(|i| h.pasta().aba(i)).map(|a| (a.id, a.sheet.name.clone())).collect();
        if depois != antes {
            for (id, _) in depois.iter().filter(|(id, _)| !antes.iter().any(|(a, _)| a == id)) {
                if let Some(i) = h.pasta().indice_do_id(*id) { h.pasta_mut().remover_aba(i); }
            }
            for (id, nome_antes) in &antes {
                if let Some(i) = h.pasta().indice_do_id(*id) {
                    if h.pasta().nomes()[i] != *nome_antes { h.pasta_mut().renomear_aba(i, nome_antes); }
                }
            }
            h.definir_aba_ativa(h.aba_ativa().min(h.pasta().len().saturating_sub(1)));
            return Err("A pasta de trabalho está protegida (estrutura): a chamada criaria ou renomearia abas e foi desfeita.".into());
        }
    }
    for (id, marca) in marcas {
        let Some(i) = h.pasta().indice_do_id(id) else { continue };
        let Some(aba) = h.pasta().aba(i) else { continue };
        if let Some(recusa) = aba.sheet.examinar_protecao(marca) {
            let nome_aba = aba.sheet.name.clone();
            h.pasta_mut().editar(i, |s| s.desfazer_ate(marca));
            return Err(format!("{} Nada foi alterado na aba {nome_aba} (protegida; desproteja com sheet_proteger).", recusa.0));
        }
    }
    saida
}

fn despachar(h: &mut dyn Hospede, nome: &str, args: &Json) -> Saida {
    match nome {
        "sheet_resumo" => resumo(h),
        "sheet_ler" => ler(h, args),
        "sheet_escrever" => escrever(h, args),
        "sheet_limpar" => limpar(h, args),
        "sheet_aba" => aba(h, args),
        "sheet_nomes" => nomes(h, args),
        "sheet_proteger" => proteger(h, args),
        "sheet_link" => link(h, args),
        "sheet_impressao" => impressao(h, args),
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
        "sheet_duplicados" => crate::edicao::duplicados(h, args),
        "sheet_atingir_meta" => crate::edicao::atingir_meta(h, args),
        "sheet_consolidar" => crate::edicao::consolidar(h, args),
        "sheet_filtrar" => crate::edicao::filtrar(h, args),
        "sheet_substituir" => crate::edicao::substituir(h, args),
        "sheet_mover" => crate::edicao::mover(h, args),
        "sheet_copiar" => crate::edicao::copiar(h, args),
        "sheet_condicional" => crate::condicional::condicional(h, args),
        "sheet_validacao" => crate::validacao::validacao(h, args),
        "sheet_grafico" => crate::grafico::grafico(h, args),
        "sheet_botao" => crate::botao::botao(h, args),
        "sheet_formulario" => crate::formulario::formulario(h, args),
        "sheet_tabela" => {
            let texto = h.tabela(args)?;
            if !crate::permissoes::so_le("sheet_tabela", args) { h.marcar_alterado(); }
            Ok(conteudo(texto))
        }
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
        if area.area() >= 200_000 { h.calcular_muitas(); }
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
    // Proteger pasta de trabalho (estrutura): como na tela, abas não são criadas, renomeadas nem excluídas.
    if matches!(acao, "criar" | "renomear" | "excluir") && h.pasta().propriedades.protecao_estrutura.is_some() {
        return Err("A pasta de trabalho está protegida (estrutura): abas não podem ser criadas, renomeadas nem excluídas.".into());
    }
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
    // `escopo`: a aba onde o nome vale (o `localSheetId` do Excel); sem ele, a pasta inteira.
    let escopo = match arg_texto(args, "escopo") {
        Some(a) => Some(h.pasta().nomes()[indice_aba(h.pasta(), &Json::String(a.to_owned()))?].clone()),
        None => None,
    };
    let texto = match acao {
        "listar" => {
            let mut lista: Vec<String> = h.pasta().nomeados().into_iter().map(|n| format!("{} = {}{}", n.nome,
                n.area.sheet.as_deref().map(|a| format!("{a}!")).unwrap_or_default(), nome_area(&n.area))).collect();
            lista.extend(h.pasta().nomes_formula().into_iter().map(|n| format!("{} = {}{}", n.nome, formula_exibida(&n.formula),
                n.aba.map(|a| format!(" (só na aba {a})")).unwrap_or_default())));
            if lista.is_empty() { "Nenhum nome definido.".into() } else { lista.join("\n") }
        }
        // Com `formula` (`=0,1`, `=Vendas!$A$1*2`, em pt-BR) é nome de valor/fórmula; com `intervalo`, de intervalo.
        "definir" => {
            let nome = arg_texto(args, "nome").ok_or("Falta `nome`.")?;
            if let Some(f) = arg_texto(args, "formula") {
                let f = conteudo_da_ia(&Json::String(if f.starts_with('=') { f.to_owned() } else { format!("={f}") }));
                feathersheet_core::formula::parser::parse_formula(&f[1..]).map_err(|e| format!("Fórmula inválida: {}", e.message))?;
                h.pasta_mut().definir_nome_formula(nome, &f, escopo.as_deref());
                h.marcar_alterado();
                return Ok(conteudo(format!("{nome} = {}{}", formula_exibida(&f),
                    escopo.map(|a| format!(" (só na aba {a})")).unwrap_or_default())));
            }
            let (aba, mut area) = intervalo(h, args, "intervalo")?;
            area.sheet = Some(h.pasta().nomes()[aba].clone());
            let pasta = h.pasta_mut();
            if let Some(a) = &escopo {
                let texto = format!("={}!{}", feathersheet_core::celulas::refs::quote_sheet(area.sheet.as_deref().unwrap_or_default()), nome_area(&area));
                pasta.definir_nome_formula(nome, &texto, Some(a));
            } else {
                pasta.remover_nome(nome);
                pasta.definir_nome(nome, area.clone());
            }
            h.marcar_alterado();
            format!("{nome} = {}!{}", h.pasta().nomes()[aba], nome_area(&area))
        }
        "remover" => {
            let nome = arg_texto(args, "nome").ok_or("Falta `nome`.")?;
            let saiu = match &escopo {
                Some(a) => h.pasta_mut().remover_nome_formula(nome, Some(a)),
                None => h.pasta_mut().remover_nome(nome),
            };
            if !saiu { return Err(format!("Não existe o nome \"{nome}\".")); }
            h.marcar_alterado();
            format!("Nome \"{nome}\" removido")
        }
        outra => return Err(format!("Ação desconhecida: {outra}. Use listar, definir ou remover.")),
    };
    Ok(conteudo(texto))
}

/// Proteger planilha (como Página Inicial ▸ Formatar ▸ Proteger planilha): senha opcional no formato do Excel e as
/// permissões pelo nome do `<sheetProtection>` (`sort`, `autoFilter`, `formatCells`…). Desproteger confere a senha.
fn proteger(h: &mut dyn Hospede, args: &Json) -> Saida {
    use feathersheet_core::planilha::sheet::protecao::{Permissao, ProtecaoPlanilha};
    let acao = arg_texto(args, "acao").unwrap_or("listar");
    if acao == "listar" {
        let pasta = h.pasta();
        let linhas: Vec<String> = (0..pasta.len()).filter_map(|i| pasta.aba(i)).map(|a| match &a.sheet.protecao {
            None => format!("{}: sem proteção", a.sheet.name),
            Some(p) => format!("{}: protegida {} · permite: {}", a.sheet.name,
                if p.senha.is_some() { "com senha" } else { "sem senha" },
                p.permitidas.iter().map(|x| x.chave()).collect::<Vec<_>>().join(", ")),
        }).collect();
        let pasta = match &h.pasta().propriedades.protecao_estrutura {
            None => "Pasta (estrutura): sem proteção".to_string(),
            Some(p) => format!("Pasta (estrutura): protegida {}", if p.senha.is_some() { "com senha" } else { "sem senha" }),
        };
        return Ok(conteudo(format!("{pasta}\n{}", linhas.join("\n"))));
    }
    // Bloquear/desbloquear células (o "Bloqueada" de Formatar células ▸ Proteção): só vale com a aba protegida.
    if let a @ ("bloquear" | "desbloquear") = acao {
        let (aba, area) = intervalo(h, args, "intervalo")?;
        if h.pasta().aba(aba).is_some_and(|x| x.sheet.protegida()) {
            return Err("A aba está protegida: desproteja antes de mudar o que fica bloqueado (como no Excel).".into());
        }
        let patch = feathersheet_core::celulas::estilo_parcial::EstiloParcial::from([("locked".to_string(),
            feathersheet_core::celulas::estilo_parcial::Campo::Logico(a == "bloquear"))]);
        h.pasta_mut().editar(aba, |s| s.set_style(&area, &patch));
        h.marcar_alterado();
        return Ok(conteudo(format!("{} {} (vale quando a aba for protegida)", nome_area(&area),
            if a == "bloquear" { "bloqueada" } else { "desbloqueada" })));
    }
    if arg_texto(args, "alvo") == Some("pasta") {
        return proteger_pasta(h, acao, arg_texto(args, "senha").unwrap_or(""));
    }
    let aba = aba_do_arg(h, args)?;
    let nome = h.pasta().nomes()[aba].clone();
    let guardada = h.pasta().aba(aba).and_then(|a| a.sheet.protecao.clone());
    let senha = arg_texto(args, "senha").unwrap_or("");
    let texto = match acao {
        "proteger" => {
            if guardada.is_some() { return Err(format!("A aba {nome} já está protegida: desproteja antes (como no Excel).")); }
            let mut permitidas = Permissao::padrao();
            for p in args.get("permitir").and_then(Json::as_array).into_iter().flatten() {
                let chave = p.as_str().unwrap_or_default();
                let todas: Vec<&str> = Permissao::TODAS.iter().map(|(_, _, a)| *a).collect();
                permitidas.insert(Permissao::da_chave(chave)
                    .ok_or_else(|| format!("Permissão desconhecida: {chave}. Use: {}.", todas.join(", ")))?);
            }
            let senha = (!senha.is_empty()).then(|| feathersheet_io::senha_planilha::criar_senha(senha)).transpose()?;
            let com_senha = senha.is_some();
            h.pasta_mut().editar(aba, |s| { s.proteger(ProtecaoPlanilha { senha, permitidas }); });
            format!("Aba {nome} protegida{}.", if com_senha { " com senha" } else { " sem senha" })
        }
        "desproteger" => {
            let Some(p) = guardada else { return Err(format!("A aba {nome} não está protegida.")) };
            if let Some(g) = &p.senha {
                if !feathersheet_io::senha_planilha::conferir(senha, g)? {
                    return Err(format!("Senha errada: a aba {nome} continua protegida."));
                }
            }
            h.pasta_mut().editar(aba, |s| { s.desproteger(); });
            format!("Aba {nome} desprotegida.")
        }
        outra => return Err(format!("Ação desconhecida: {outra}. Use listar, proteger, desproteger, bloquear ou desbloquear.")),
    };
    h.marcar_alterado();
    Ok(conteudo(texto))
}

/// Proteger pasta de trabalho (estrutura): abas não são criadas, excluídas, renomeadas, movidas nem ocultadas.
fn proteger_pasta(h: &mut dyn Hospede, acao: &str, senha: &str) -> Saida {
    let guardada = h.pasta().propriedades.protecao_estrutura.clone();
    let texto = match acao {
        "proteger" => {
            if guardada.is_some() { return Err("A pasta já está protegida: desproteja antes.".into()); }
            let senha = (!senha.is_empty()).then(|| feathersheet_io::senha_planilha::criar_senha(senha)).transpose()?;
            h.pasta_mut().propriedades.protecao_estrutura = Some(feathersheet_core::planilha::workbook::ProtecaoPasta { senha });
            "Pasta protegida: as abas não podem ser criadas, excluídas, renomeadas, movidas nem ocultadas."
        }
        "desproteger" => {
            let Some(p) = guardada else { return Err("A pasta não está protegida.".into()) };
            if let Some(g) = &p.senha {
                if !feathersheet_io::senha_planilha::conferir(senha, g)? { return Err("Senha errada: a pasta continua protegida.".into()); }
            }
            h.pasta_mut().propriedades.protecao_estrutura = None;
            "Pasta desprotegida."
        }
        outra => return Err(format!("Ação desconhecida: {outra}. Use listar, proteger ou desproteger.")),
    };
    h.marcar_alterado();
    Ok(conteudo(texto.to_string()))
}

/// Hiperlinks de célula (o campo `link` do estilo, como Inserir ▸ Link): listar, definir (com texto opcional) e
/// remover (o texto fica). Só web, `mailto:` e `#Aba!A1`, pela mesma regra do app (`feathersheet_io::links`).
fn link(h: &mut dyn Hospede, args: &Json) -> Saida {
    use feathersheet_core::celulas::estilo_parcial::{Campo, EstiloParcial};
    let acao = arg_texto(args, "acao").unwrap_or("listar");
    let (aba, area) = match args.get("intervalo") {
        Some(_) => intervalo(h, args, "intervalo")?,
        None => { let a = aba_do_arg(h, args)?; (a, h.pasta().aba(a).ok_or("Aba inexistente")?.sheet.used_rect()) }
    };
    let texto = match acao {
        "listar" => {
            let sheet = &h.pasta().aba(aba).ok_or("Aba inexistente")?.sheet;
            let mut achados = Vec::new();
            'fora: for r in area.r0..=area.r1.min(sheet.used_rect().r1) {
                for c in area.c0..=area.c1.min(sheet.used_rect().c1) {
                    if let Some(l) = sheet.estilo_resolvido(r, c).and_then(|e| e.link) {
                        achados.push(format!("{}: {l}", cell_name(r, c)));
                        if achados.len() >= 200 { achados.push("… (mais de 200)".into()); break 'fora; }
                    }
                }
            }
            if achados.is_empty() { "Nenhum hiperlink.".into() } else { achados.join("\n") }
        }
        "definir" => {
            let destino = arg_texto(args, "endereco").ok_or("Falta `endereco` (https://…, mailto:… ou #Aba!A1).")?;
            let link = feathersheet_io::links::link_do_estilo(destino)
                .ok_or("Endereço recusado: use http://, https://, mailto:, www. ou #Aba!A1 (sem espaços).")?;
            if (area.r1 - area.r0 + 1) as u64 * (area.c1 - area.c0 + 1) as u64 > MAX_ESCREVER {
                return Err("Intervalo grande demais para links.".into());
            }
            let texto = arg_texto(args, "texto").map(str::to_owned);
            let patch = EstiloParcial::from([("link".to_string(), Campo::Texto(link.clone()))]);
            h.pasta_mut().editar(aba, |s| {
                // Um Ctrl+Z: texto e link juntos, como no diálogo do app.
                s.abrir_transacao();
                if let Some(t) = &texto { s.set_cell(area.r0, area.c0, t); }
                s.set_style(&area, &patch);
                s.fechar_transacao();
            });
            format!("Link {link} em {}", nome_area(&area))
        }
        "remover" => {
            let patch = EstiloParcial::from([("link".to_string(), Campo::Nulo)]);
            h.pasta_mut().editar(aba, |s| s.set_style(&area, &patch));
            format!("Links removidos de {} (o texto ficou)", nome_area(&area))
        }
        outra => return Err(format!("Ação desconhecida: {outra}. Use listar, definir ou remover.")),
    };
    if acao != "listar" { h.marcar_alterado(); }
    Ok(conteudo(texto))
}

/// Configurar página da aba (o `pg`, como Layout da Página): área de impressão, títulos que repetem, orientação,
/// papel e ajustar a N páginas. Sem desfazer, como no Excel.
fn impressao(h: &mut dyn Hospede, args: &Json) -> Saida {
    use feathersheet_io::pagina_aba;
    const PAPEIS: [(&str, u16); 6] = [("A4", 9), ("Carta", 1), ("Oficio", 5), ("A3", 8), ("A5", 11), ("Tabloide", 3)];
    let aba = aba_do_arg(h, args)?;
    let nome = h.pasta().nomes()[aba].clone();
    let mut p = pagina_aba::ler(h.pasta().aba(aba).and_then(|a| a.sheet.pg.as_deref()));
    let acao = arg_texto(args, "acao").unwrap_or("listar");
    if acao == "definir" {
        if let Some(t) = args.get("area").and_then(Json::as_str) {
            pagina_aba::ler_area(&mut p, t);
            // Coluna além da XFD (16384) ou linha além da 1.048.576 não existe no Excel.
            let fora = p.areas.iter().any(|a| a[3] > 16_383 || a[2] > 1_048_575);
            if !t.trim().is_empty() && (fora || p.areas.len() != t.split(',').count()) {
                return Err(format!("Área de impressão inválida: {t} (ex.: A1:F40 ou A1:B9,D1:E9)."));
            }
        }
        if let Some(t) = args.get("titulos").and_then(Json::as_str) {
            p.titulos_linhas = None;
            p.titulos_colunas = None;
            pagina_aba::ler_titulos(&mut p, t);
            if !t.trim().is_empty() && p.titulos_linhas.is_none() && p.titulos_colunas.is_none() {
                return Err(format!("Títulos inválidos: {t} (linhas 1:2, colunas A:B ou os dois: 1:2,A:A)."));
            }
        }
        if let Some(o) = arg_texto(args, "orientacao") {
            p.paisagem = match o { "paisagem" => true, "retrato" => false, _ => return Err("orientacao: retrato ou paisagem.".into()) };
        }
        if let Some(t) = arg_texto(args, "papel") {
            let sem_acento = t.replace(['í', 'Í'], "i");
            p.papel = PAPEIS.iter().find(|(n, _)| n.eq_ignore_ascii_case(&sem_acento)).map(|(_, c)| *c)
                .ok_or_else(|| format!("Papel desconhecido: {t}. Use {}.", PAPEIS.map(|(n, _)| n).join(", ")))?;
        }
        if let Some(v) = args.get("ajustar") {
            p.ajustar = match v.as_array().map(|a| a.iter().map(|x| x.as_u64().and_then(|n| u16::try_from(n).ok())).collect::<Option<Vec<_>>>()) {
                Some(Some(l)) if l.len() == 2 => Some([l[0], l[1]]),
                _ if v.is_null() => None,
                _ => return Err("ajustar: [largura, altura] em páginas (0 = automático), ou null.".into()),
            };
        }
        let novo = pagina_aba::gravar(&p);
        h.pasta_mut().editar(aba, |s| s.pg = novo);
        h.marcar_alterado();
    } else if acao != "listar" {
        return Err(format!("Ação desconhecida: {acao}. Use listar ou definir."));
    }
    let papel = PAPEIS.iter().find(|(_, c)| *c == p.papel).map_or_else(|| format!("código {}", p.papel), |(n, _)| (*n).to_string());
    Ok(conteudo(format!("{nome}: área {}; títulos {}; {}; papel {papel}; {}",
        pagina_aba::texto_area(&p, &nome).unwrap_or_else(|| "a parte usada".into()),
        pagina_aba::texto_titulos(&p, &nome).unwrap_or_else(|| "nenhum".into()),
        if p.paisagem { "paisagem" } else { "retrato" },
        match p.ajustar { Some([l, a]) => format!("ajustar a {l} × {a} páginas (0 = auto)"), None => format!("escala {}%", p.escala) })))
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

    /// `sheet_nomes` com fórmula e escopo: a IA cria `Taxa = 0,1` e um nome de uma aba só, lista e remove.
    #[test]
    fn nomes_de_formula_e_de_aba_pelo_mcp() {
        let mut h = Avulso::default();
        h.pasta.criar_aba("Plan2");
        h.pasta.escrever_celula(0, 0, 0, "1000".into());
        texto(chamar(&mut h, "sheet_nomes", &json!({ "acao": "definir", "nome": "Taxa", "formula": "=0,1" })));
        texto(chamar(&mut h, "sheet_nomes", &json!({ "acao": "definir", "nome": "Meta", "formula": "20", "escopo": "Plan2" })));
        h.pasta.escrever_celula(0, 0, 1, "=A1*Taxa".into());
        h.pasta.escrever_celula(1, 0, 0, "=Meta".into());
        assert_eq!(h.pasta.valor(0, 0, 1), feathersheet_core::Value::Number(100.0));
        assert_eq!(h.pasta.valor(1, 0, 0), feathersheet_core::Value::Number(20.0));
        let lista = texto(chamar(&mut h, "sheet_nomes", &json!({ "acao": "listar" })));
        assert!(lista.contains("Taxa = =0,1") && lista.contains("Meta = =20 (só na aba Plan2)"), "{lista}");
        assert!(chamar(&mut h, "sheet_nomes", &json!({ "acao": "definir", "nome": "X", "formula": "=1+" })).is_err());
        assert!(chamar(&mut h, "sheet_nomes", &json!({ "acao": "remover", "nome": "Meta" })).is_err(), "o da aba pede o escopo");
        texto(chamar(&mut h, "sheet_nomes", &json!({ "acao": "remover", "nome": "Meta", "escopo": "Plan2" })));
        texto(chamar(&mut h, "sheet_nomes", &json!({ "acao": "remover", "nome": "Taxa" })));
        assert!(h.pasta.nomes_formula().is_empty());
    }

    /// `sheet_proteger`: protege com senha e permissões, recusa proteger de novo e desproteger com a senha errada.
    #[test]
    fn proteger_e_desproteger_pelo_mcp() {
        let mut h = Avulso::default();
        assert!(chamar(&mut h, "sheet_proteger", &json!({ "acao": "proteger", "permitir": ["voar"] })).unwrap_err().contains("desconhecida"));
        texto(chamar(&mut h, "sheet_proteger", &json!({ "acao": "proteger", "senha": "s3gr3d0", "permitir": ["sort"] })));
        let p = h.pasta.aba(0).unwrap().sheet.protecao.clone().unwrap();
        assert!(p.senha.is_some() && p.permite(feathersheet_core::planilha::sheet::protecao::Permissao::Classificar));
        assert!(texto(chamar(&mut h, "sheet_proteger", &json!({ "acao": "listar" }))).contains("protegida com senha"));
        assert!(chamar(&mut h, "sheet_proteger", &json!({ "acao": "proteger" })).is_err(), "já protegida");
        assert!(chamar(&mut h, "sheet_proteger", &json!({ "acao": "desproteger", "senha": "errada" })).unwrap_err().contains("Senha errada"));
        assert!(h.pasta.aba(0).unwrap().sheet.protegida());
        texto(chamar(&mut h, "sheet_proteger", &json!({ "acao": "desproteger", "senha": "s3gr3d0" })));
        assert!(!h.pasta.aba(0).unwrap().sheet.protegida());
    }

    /// `sheet_link`: define com texto (um Ctrl+Z), recusa esquema perigoso, lista e remove mantendo o texto.
    #[test]
    fn links_pelo_mcp() {
        let mut h = Avulso::default();
        assert!(chamar(&mut h, "sheet_link", &json!({ "acao": "definir", "intervalo": "A1", "endereco": "javascript:alert(1)" })).is_err());
        texto(chamar(&mut h, "sheet_link", &json!({ "acao": "definir", "intervalo": "A1", "endereco": "www.exemplo.com", "texto": "Site" })));
        texto(chamar(&mut h, "sheet_link", &json!({ "acao": "definir", "intervalo": "B2", "endereco": "#Planilha1!C3" })));
        let lista = texto(chamar(&mut h, "sheet_link", &json!({ "acao": "listar" })));
        assert!(lista.contains("A1: https://www.exemplo.com") && lista.contains("B2: #Planilha1!C3"), "{lista}");
        assert_eq!(h.pasta.aba(0).unwrap().sheet.get_raw(0, 0), "Site");
        texto(chamar(&mut h, "sheet_link", &json!({ "acao": "remover", "intervalo": "A1" })));
        assert!(h.pasta.aba(0).unwrap().sheet.estilo_resolvido(0, 0).and_then(|e| e.link).is_none());
        assert_eq!(h.pasta.aba(0).unwrap().sheet.get_raw(0, 0), "Site", "o texto fica");
        texto(chamar(&mut h, "sheet_desfazer", &json!({})));
        assert!(h.pasta.aba(0).unwrap().sheet.estilo_resolvido(0, 0).and_then(|e| e.link).is_some(), "Ctrl+Z volta o link");
    }

    /// `sheet_impressao`: área, títulos, paisagem, papel e ajustar vão para o `pg` (que o `.xlsx` grava); entrada ruim é recusada.
    #[test]
    fn configurar_pagina_pelo_mcp() {
        let mut h = Avulso::default();
        let t = texto(chamar(&mut h, "sheet_impressao", &json!({ "acao": "definir", "area": "A1:F40", "titulos": "1:1",
            "orientacao": "paisagem", "papel": "Ofício", "ajustar": [1, 0] })));
        assert!(t.contains("$A$1:$F$40") && t.contains("$1:$1") && t.contains("paisagem") && t.contains("Oficio") && t.contains("1 × 0"), "{t}");
        let p = feathersheet_io::pagina_aba::ler(h.pasta.aba(0).unwrap().sheet.pg.as_deref());
        assert_eq!((p.areas.clone(), p.titulos_linhas, p.paisagem, p.papel, p.ajustar), (vec![[0, 0, 39, 5]], Some([0, 0]), true, 5, Some([1, 0])));
        for ruim in [json!({ "acao": "definir", "area": "zzz" }), json!({ "acao": "definir", "papel": "A9" }),
                     json!({ "acao": "definir", "ajustar": [1] }), json!({ "acao": "definir", "orientacao": "torto" })] {
            assert!(chamar(&mut h, "sheet_impressao", &ruim).is_err(), "{ruim}");
        }
        texto(chamar(&mut h, "sheet_impressao", &json!({ "acao": "definir", "area": "", "titulos": "" })));
        assert!(feathersheet_io::pagina_aba::ler(h.pasta.aba(0).unwrap().sheet.pg.as_deref()).areas.is_empty());
    }

    /// Aba protegida: escrever em célula bloqueada e inserir linha são recusados (nada muda); desbloqueada passa;
    /// pasta protegida não deixa criar aba.
    #[test]
    fn protecao_vale_para_a_ia() {
        let mut h = Avulso::default();
        texto(chamar(&mut h, "sheet_escrever", &json!({ "celula": "A1", "valores": [["antes"]] })));
        texto(chamar(&mut h, "sheet_proteger", &json!({ "acao": "proteger" })));
        let e = chamar(&mut h, "sheet_escrever", &json!({ "celula": "A1", "valores": [["depois"]] })).unwrap_err();
        assert!(e.contains("protegida"), "{e}");
        assert_eq!(h.pasta.aba(0).unwrap().sheet.get_raw(0, 0), "antes");
        assert!(chamar(&mut h, "sheet_estrutura", &json!({ "acao": "inserir", "eixo": "linhas", "intervalo": "2:2" })).is_err());
        // Célula desbloqueada antes de proteger: a IA escreve.
        assert!(chamar(&mut h, "sheet_proteger", &json!({ "acao": "desbloquear", "intervalo": "B2" })).is_err(), "protegida: desproteger antes");
        texto(chamar(&mut h, "sheet_proteger", &json!({ "acao": "desproteger" })));
        texto(chamar(&mut h, "sheet_proteger", &json!({ "acao": "desbloquear", "intervalo": "B2" })));
        texto(chamar(&mut h, "sheet_proteger", &json!({ "acao": "proteger" })));
        texto(chamar(&mut h, "sheet_escrever", &json!({ "celula": "B2", "valores": [["livre"]] })));
        assert_eq!(h.pasta.aba(0).unwrap().sheet.get_raw(1, 1), "livre");
        texto(chamar(&mut h, "sheet_proteger", &json!({ "acao": "proteger", "alvo": "pasta", "senha": "x1" })));
        assert!(chamar(&mut h, "sheet_aba", &json!({ "acao": "criar", "nome": "Nova" })).unwrap_err().contains("protegida"));
        assert_eq!(h.pasta.len(), 1);
        assert!(chamar(&mut h, "sheet_proteger", &json!({ "acao": "desproteger", "alvo": "pasta", "senha": "y" })).is_err());
        // Outra ferramenta que cria aba (consulta com destino novo) também é barrada.
        texto(chamar(&mut h, "sheet_proteger", &json!({ "acao": "desproteger" })));
        texto(chamar(&mut h, "sheet_escrever", &json!({ "celula": "D1", "valores": [["n"], [1], [2]] })));
        let e = chamar(&mut h, "sheet_consulta", &json!({ "fonte": "D1:D3", "destino": "NovaAba" }));
        assert!(e.as_ref().is_err_and(|e| e.contains("protegida")), "{e:?}");
        assert_eq!(h.pasta.len(), 1);
        texto(chamar(&mut h, "sheet_proteger", &json!({ "acao": "desproteger", "alvo": "pasta", "senha": "x1" })));
        texto(chamar(&mut h, "sheet_aba", &json!({ "acao": "criar", "nome": "Nova" })));
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
