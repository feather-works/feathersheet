//! `sheet_dinamica` do MCP ao vivo: listar, criar, editar, atualizar e
//! excluir tabelas dinâmicas pelo mesmo caminho do painel (`Estado`,
//! `criar_dinamica_nativa`, `aplicar_estado_na_dinamica`) — o que a IA cria
//! é igual ao que a pessoa criaria com o mouse, e cada chamada é um Ctrl+Z.
use super::*;
use serde_json::Value as Json;
use feathersheet_core::dinamica::exibicao::{CampoBase, ItemBase};
use feathersheet_core::dinamica::dinamica_layout::SEPARADOR;

/// As dinâmicas de todas as abas: (aba, definição).
fn todas(app: &App) -> Vec<(usize, Json)> {
    (0..app.workbook.len()).flat_map(|i| {
        app.workbook.aba(i).and_then(|a| a.sheet.dn.as_deref())
            .and_then(|txt| serde_json::from_str::<Vec<Json>>(txt).ok())
            .unwrap_or_default().into_iter().map(move |d| (i, d))
    }).collect()
}

/// A coluna (absoluta) de um campo pelo título do cabeçalho ou pela letra.
fn coluna_do_campo(e: &Estado, campo: &str) -> Result<usize, String> {
    let alvo = campo.trim();
    if let Some(i) = e.nomes.iter().position(|n| n.eq_ignore_ascii_case(alvo)) {
        return Ok(e.origem.c0 as usize + i);
    }
    if let Some(c) = feathersheet_core::celulas::refs::col_index(&alvo.to_ascii_uppercase()) {
        if c >= e.origem.c0 && (c as usize) < e.origem.c0 as usize + e.nomes.len() {
            return Ok(c as usize);
        }
    }
    Err(format!("Campo \"{alvo}\" não está na origem. Campos: {}.", e.nomes.join(", ")))
}

fn lista(args: &Json, chave: &str) -> Vec<String> {
    args[chave].as_array().map(|l| l.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect()).unwrap_or_default()
}

/// Um campo calculado novo (a regra do painel); com o nome já existente, nada muda.
fn acrescentar_calculado(e: &mut Estado, nome: &str, formula: &str) -> Result<(), String> {
    use super::painel_avancado::{acrescentar_calculado as acrescentar, ErroCalculado};
    let real = e.nomes.len().saturating_sub(e.calculados.len());
    let campos = e.nomes[..real].join(", ");
    match acrescentar(e, nome, formula) {
        Ok(()) | Err(ErroCalculado::JaExiste) => Ok(()),
        Err(ErroCalculado::SemColunas) => Err("Não há mais colunas para campos calculados.".into()),
        Err(ErroCalculado::Formula(m)) => Err(format!("Campo calculado \"{nome}\": {m}. Campos: {campos}.")),
    }
}

/// O campo de Linhas ou Colunas de um item calculado ou grupo.
fn campo_dos_eixos(e: &Estado, v: &Json) -> Result<usize, String> {
    let campo = v["campo"].as_str().ok_or("Cada item calculado ou grupo precisa de `campo`.")?;
    let c = coluna_do_campo(e, campo)?;
    if !e.linhas.contains(&c) && !e.colunas.contains(&c) {
        return Err(format!("O campo \"{campo}\" precisa estar em linhas ou colunas."));
    }
    Ok(c)
}

/// Põe em `e` o que veio nos argumentos (só o que veio: editar mexe no resto).
fn aplicar_args(app: &App, e: &mut Estado, args: &Json) -> Result<(), String> {
    use feathersheet_core::dinamica::exibicao::{GrupoDeItens, ItemCalculado};
    // Primeiro os campos calculados: linhas, colunas e valores podem citá-los.
    for c in args["campos_calculados"].as_array().into_iter().flatten() {
        let (Some(nome), Some(formula)) = (c["nome"].as_str(), c["formula"].as_str()) else {
            return Err("Cada campo calculado precisa de `nome` e `formula`.".into());
        };
        acrescentar_calculado(e, nome.trim(), formula.trim())?;
    }
    if args.get("linhas").is_some() {
        e.linhas = lista(args, "linhas").iter().map(|c| coluna_do_campo(e, c)).collect::<Result<_, _>>()?;
    }
    if args.get("colunas").is_some() {
        e.colunas = lista(args, "colunas").iter().map(|c| coluna_do_campo(e, c)).collect::<Result<_, _>>()?;
    }
    if let Some(valores) = args.get("valores").and_then(Json::as_array) {
        e.valores = valores.iter().map(|v| {
            let (campo, como) = match v {
                Json::String(campo) => (campo.as_str(), "soma"),
                _ => (v["campo"].as_str().ok_or("Cada valor precisa de `campo`.")?, v["como"].as_str().unwrap_or("soma")),
            };
            let op = edicao::agregacao(como)
                .ok_or_else(|| format!("Agregação \"{como}\" desconhecida: soma, contagem, media, min, max, desviopadrao, moda."))?;
            Ok(MedidaDinamica { coluna: coluna_do_campo(e, campo)?, op })
        }).collect::<Result<_, String>>()?;
    }
    match args["layout"].as_str() {
        Some("compacto") => e.layout = Layout::Compacto,
        Some("tabular") => e.layout = Layout::Tabular,
        Some(outro) => return Err(format!("Layout \"{outro}\" desconhecido: tabular ou compacto.")),
        None => {}
    }
    if let Some(tema) = args["tema"].as_str() {
        use feathersheet_core::dinamica::exibicao::TEMAS_DA_DINAMICA as TEMAS;
        if !TEMAS.contains(&tema) { return Err(format!("Tema \"{tema}\" desconhecido: {}.", TEMAS.join(", "))); }
        e.tema = tema.to_owned();
    }
    match args["subtotais"].as_str() {
        Some("sem") => (e.subtotais, e.subtotal_embaixo) = (false, false),
        // No tabular o subtotal vai sempre embaixo (como o painel).
        Some("em_cima") => (e.subtotais, e.subtotal_embaixo) = (true, e.layout == Layout::Tabular),
        Some("embaixo") => (e.subtotais, e.subtotal_embaixo) = (true, true),
        Some(outro) => return Err(format!("Subtotais \"{outro}\" desconhecido: sem, em_cima ou embaixo.")),
        None => {}
    }
    if let Some(t) = args.get("totais_gerais").filter(|t| t.is_object()) {
        let extras = config_campo::extras_editaveis(e);
        // linha_embaixo = "Total Geral" embaixo (colGrandTotals); coluna_direita = à direita (rowGrandTotals).
        if let Some(v) = t["linha_embaixo"].as_bool() { extras.totais_gerais_colunas = v; }
        if let Some(v) = t["coluna_direita"].as_bool() { extras.totais_gerais_linhas = v; }
    }
    if let Some(lista) = args["itens_calculados"].as_array() {
        let mut novos = Vec::new();
        for v in lista {
            let coluna = campo_dos_eixos(e, v)?;
            let (Some(nome), Some(formula)) = (v["nome"].as_str(), v["formula"].as_str()) else {
                return Err("Cada item calculado precisa de `campo`, `nome` e `formula`.".into());
            };
            let itens = app.itens_do_campo(e, coluna);
            feathersheet_core::dinamica::campo_calculado::compilar(formula, &itens)
                .map_err(|m| format!("Item calculado \"{nome}\": {m}. Itens: {}.", itens.join(", ")))?;
            novos.push(ItemCalculado { coluna, nome: nome.trim().to_owned(), formula: formula.trim().to_owned() });
        }
        config_campo::extras_editaveis(e).itens_calculados = novos;
    }
    if let Some(lista) = args["filtros"].as_array() {
        // Substitui os filtros de itens; [] tira todos.
        let mut novos = Vec::new();
        for v in lista {
            let campo = v["campo"].as_str().ok_or("Cada filtro precisa de `campo`.")?;
            let coluna = coluna_do_campo(e, campo)?;
            let (itens, excluir) = match (v.get("mostrar"), v.get("ocultar")) {
                (Some(m), None) => (lista_de(m), false),
                (None, Some(o)) => (lista_de(o), true),
                _ => return Err(format!("Filtro de \"{campo}\": use `mostrar` ou `ocultar` com a lista de itens.")),
            };
            // O filtro compara a chave interna (data = número de série, vazio = "(Vazio)"): o
            // item vem pelo que a lista do painel mostra e vira a chave; o que não existe é
            // recusado (antes, `mostrar` com uma data escrita escondia a tabela inteira).
            let (opcoes, cortada) = avancado::opcoes_de_filtro(app, e, coluna);
            let mut chaves = std::collections::HashSet::new();
            for item in &itens {
                let achada = opcoes.iter().find(|(k, r)| k.eq_ignore_ascii_case(item) || r.eq_ignore_ascii_case(item));
                match achada {
                    Some((k, _)) => { chaves.insert(k.clone()); }
                    None if cortada => { chaves.insert(item.clone()); }
                    None => {
                        let existentes: Vec<&str> = opcoes.iter().take(20).map(|(_, r)| r.as_str()).collect();
                        return Err(format!("O campo \"{campo}\" não tem o item \"{item}\". Itens: {}.", existentes.join(", ")));
                    }
                }
            }
            novos.push(Filtro { coluna, valores: chaves, excluir });
        }
        e.filtros = novos;
    }
    if let Some(lista) = args["ordenar"].as_array() {
        for v in lista {
            let campo = v["campo"].as_str().ok_or("Cada ordenação precisa de `campo`.")?;
            let coluna = coluna_do_campo(e, campo)?;
            let medida = v["medida"].as_u64().unwrap_or(0) as usize;
            if medida >= e.valores.len().max(1) { return Err(format!("`medida` {medida} não existe: há {} valor(es).", e.valores.len())); }
            let ordem = match v["ordem"].as_str().unwrap_or("crescente") {
                "padrao" => Ordenacao::Padrao,
                "crescente" => Ordenacao::Crescente,
                "decrescente" => Ordenacao::Decrescente,
                "maior_valor" => Ordenacao::PorValor { medida, crescente: false },
                "menor_valor" => Ordenacao::PorValor { medida, crescente: true },
                outra => return Err(format!("Ordem \"{outra}\" desconhecida: padrao, crescente, decrescente, maior_valor ou menor_valor.")),
            };
            let (linhas, colunas) = (e.linhas.iter().position(|&c| c == coluna), e.colunas.iter().position(|&c| c == coluna));
            let extras = config_campo::extras_editaveis(e);
            match (linhas, colunas) {
                (Some(p), _) => extras.linhas[p].ordenacao = ordem,
                (None, Some(p)) => extras.colunas[p].ordenacao = ordem,
                _ => return Err(format!("O campo \"{campo}\" precisa estar em linhas ou colunas para ordenar.")),
            }
        }
    }
    if let Some(lista) = args["agrupar"].as_array() {
        use feathersheet_core::dinamica::exibicao::{Faixas, ParteData};
        for v in lista {
            let campo = v["campo"].as_str().ok_or("Cada agrupamento precisa de `campo`.")?;
            let coluna = coluna_do_campo(e, campo)?;
            let agrupamento = if let Some(f) = v["faixas"].as_array() {
                let n: Vec<f64> = f.iter().filter_map(Json::as_f64).collect();
                match n[..] {
                    [inicio, fim, intervalo] if intervalo > 0.0 && fim > inicio => Some(Agrupamento::Faixas(Faixas { inicio, fim, intervalo })),
                    _ => return Err(format!("`faixas` de \"{campo}\" é [início, fim, intervalo], com fim > início e intervalo > 0.")),
                }
            } else {
                match v["por"].as_str().unwrap_or("") {
                    "ano" => Some(Agrupamento::Data { parte: ParteData::Ano }),
                    "trimestre" => Some(Agrupamento::Data { parte: ParteData::Trimestre }),
                    "mes" => Some(Agrupamento::Data { parte: ParteData::Mes }),
                    "dia" => Some(Agrupamento::Data { parte: ParteData::Dia }),
                    "nada" => None,
                    outro => return Err(format!("Agrupar \"{campo}\" por \"{outro}\": use por = ano, trimestre, mes, dia ou nada, ou faixas = [início, fim, intervalo].")),
                }
            };
            let (linhas, colunas) = (e.linhas.iter().position(|&c| c == coluna), e.colunas.iter().position(|&c| c == coluna));
            let extras = config_campo::extras_editaveis(e);
            match (linhas, colunas) {
                (Some(p), _) => extras.linhas[p].agrupamento = agrupamento,
                (None, Some(p)) => extras.colunas[p].agrupamento = agrupamento,
                _ => return Err(format!("O campo \"{campo}\" precisa estar em linhas ou colunas para agrupar.")),
            }
        }
    }
    if let Some(lista) = args["mostrar_como"].as_array() {
        // Na ordem de `valores`: texto ("perc_total_geral") ou {"como": ..., "campo": ...}.
        let mut modos = Vec::new();
        for v in lista {
            let como = v.as_str().or(v["como"].as_str()).unwrap_or("normal");
            let base = || -> Result<CampoBase, String> {
                let campo = v["campo"].as_str().ok_or_else(|| format!("\"{como}\" precisa de `campo` (de linhas ou colunas)."))?;
                let c = coluna_do_campo(e, campo)?;
                match (e.linhas.iter().position(|&x| x == c), e.colunas.iter().position(|&x| x == c)) {
                    (Some(posicao), _) => Ok(CampoBase { eixo: EixoExtra::Linhas, posicao }),
                    (None, Some(posicao)) => Ok(CampoBase { eixo: EixoExtra::Colunas, posicao }),
                    _ => Err(format!("O campo \"{campo}\" precisa estar em linhas ou colunas.")),
                }
            };
            modos.push(match como {
                "normal" => MostrarComo::Normal,
                "perc_total_geral" => MostrarComo::PercTotalGeral,
                "perc_total_coluna" => MostrarComo::PercTotalColuna,
                "perc_total_linha" => MostrarComo::PercTotalLinha,
                "indice" => MostrarComo::Indice,
                "total_acumulado" => MostrarComo::TotalAcumuladoEm { campo: base()? },
                "perc_total_acumulado" => MostrarComo::PercTotalAcumuladoEm { campo: base()? },
                "classificar_maior_menor" => MostrarComo::ClassificarMaiorMenor { campo: base()? },
                "classificar_menor_maior" => MostrarComo::ClassificarMenorMaior { campo: base()? },
                "perc_de" | "diferenca_de" | "perc_diferenca_de" => {
                    let campo = base()?;
                    let item = match v["item"].as_str() {
                        Some("(anterior)") => ItemBase::Anterior,
                        Some("(seguinte)") => ItemBase::Seguinte,
                        Some(nome) => ItemBase::Item { nome: nome.to_owned() },
                        None => return Err(format!("\"{como}\" precisa de `item`: o rótulo do item base, \"(anterior)\" ou \"(seguinte)\".")),
                    };
                    match como {
                        "perc_de" => MostrarComo::PercDe { campo, item },
                        "diferenca_de" => MostrarComo::DiferencaDe { campo, item },
                        _ => MostrarComo::PercDiferencaDe { campo, item },
                    }
                }
                outro => return Err(format!("mostrar_como \"{outro}\" desconhecido: normal, perc_total_geral, perc_total_coluna, perc_total_linha, indice, total_acumulado, perc_total_acumulado, classificar_maior_menor, classificar_menor_maior, perc_de, diferenca_de, perc_diferenca_de.")),
            });
        }
        let extras = config_campo::extras_editaveis(e);
        for (i, m) in modos.into_iter().enumerate() {
            if let Some(v) = extras.valores.get_mut(i) { *v = m; }
        }
    }
    // Recolher: "tudo", "nada", "nivel" (mais um nível), "abrir_nivel" ou a lista dos itens
    // ("Norte", ou "Norte > Maçã" no segundo nível). Vale sobre os grupos do último resultado.
    match &args["recolher"] {
        Json::Null => {}
        Json::String(s) => match s.as_str() {
            "tudo" => e.recolhidos = e.grupos.clone(),
            "nada" => e.recolhidos.clear(),
            "nivel" => feathersheet_core::dinamica::dinamica_layout::passo_de_nivel(&e.grupos, &mut e.recolhidos, true),
            "abrir_nivel" => feathersheet_core::dinamica::dinamica_layout::passo_de_nivel(&e.grupos, &mut e.recolhidos, false),
            outro => return Err(format!("recolher \"{outro}\" desconhecido: tudo, nada, nivel, abrir_nivel ou a lista dos itens.")),
        },
        Json::Array(_) => {
            let mut novos = Vec::new();
            for item in lista_de(&args["recolher"]) {
                let caminho = item.split(" > ").map(str::trim).collect::<Vec<_>>().join(&SEPARADOR.to_string());
                if !e.grupos.contains(&caminho) {
                    let exemplos: Vec<String> = e.grupos.iter().take(8).map(|g| g.replace(SEPARADOR, " > ")).collect();
                    return Err(format!("\"{item}\" não é um grupo que se recolhe. Exemplos: {}.", exemplos.join(", ")));
                }
                novos.push(caminho);
            }
            e.recolhidos = novos;
        }
        _ => return Err("`recolher` é \"tudo\", \"nada\", \"nivel\", \"abrir_nivel\" ou uma lista de itens.".into()),
    }
    if let Some(lista) = args["grupos"].as_array() {
        let mut novos = Vec::new();
        for v in lista {
            let coluna = campo_dos_eixos(e, v)?;
            let nome = v["nome"].as_str().ok_or("Cada grupo precisa de `nome`.")?.trim().to_owned();
            let itens = lista_de(&v["itens"]);
            if itens.len() < 2 { return Err(format!("O grupo \"{nome}\" precisa de ao menos dois itens.")); }
            novos.push(GrupoDeItens { coluna, nome, itens });
        }
        config_campo::extras_editaveis(e).grupos_de_itens = novos;
    }
    Ok(())
}

fn lista_de(v: &Json) -> Vec<String> {
    v.as_array().map(|l| l.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default()
}

fn descrever(app: &App, aba: usize, d: &Json) -> String {
    let nome_aba = app.workbook.nomes().get(aba).cloned().unwrap_or_default();
    let area = retangulo_json(&d["areaResultado"])
        .map(|a| endereco_area(&a)).unwrap_or_default();
    let origem = retangulo_json(&d["fonte"]["area"])
        .map(|a| format!("{}!{}", d["fonte"]["aba"].as_str().unwrap_or("?"), endereco_area(&a)))
        .unwrap_or_default();
    let campos = edicao::ler_estado(app, aba, d).map(|e| {
        let nome = |c: &usize| e.nomes.get(c - e.origem.c0 as usize).cloned().unwrap_or_default();
        let valores: Vec<String> = e.valores.iter().map(|m| format!("{:?} de {}", m.op, nome(&m.coluna))).collect();
        let mut t = format!(" · linhas [{}] · colunas [{}] · valores [{}]",
            e.linhas.iter().map(nome).collect::<Vec<_>>().join(", "),
            e.colunas.iter().map(nome).collect::<Vec<_>>().join(", "), valores.join(", "));
        if e.tema != "padrao" { t += &format!(" · tema {}", e.tema); }
        if !e.recolhidos.is_empty() {
            t += &format!(" · recolhidos [{}]", e.recolhidos.iter().map(|r| r.replace(SEPARADOR, " > ")).collect::<Vec<_>>().join(", "));
        }
        if e.subtotais { t += if e.subtotal_embaixo { " · subtotais embaixo" } else { " · subtotais em cima" }; }
        for c in &e.calculados { t += &format!(" · campo calculado {} = {}", c.nome, c.formula); }
        for f in &e.filtros {
            let mut itens: Vec<&String> = f.valores.iter().collect();
            itens.sort();
            t += &format!(" · filtro {} {} [{}]", nome(&f.coluna), if f.excluir { "oculta" } else { "mostra" },
                itens.into_iter().map(String::as_str).collect::<Vec<_>>().join(", "));
        }
        let extras = avancado::extras_vigentes(&e);
        for i in &extras.itens_calculados { t += &format!(" · item calculado {}: {} = {}", nome(&i.coluna), i.nome, i.formula); }
        for g in &extras.grupos_de_itens { t += &format!(" · grupo {}: {} = [{}]", nome(&g.coluna), g.nome, g.itens.join(", ")); }
        t
    }).unwrap_or_default();
    format!("{} em {nome_aba}!{area} (origem {origem}){campos}", d["id"].as_str().unwrap_or("?"))
}

impl App {
    /// `sheet_dinamica`: `aba` e `origem` já resolvidos pelo MCP quando vieram.
    pub(in crate::app) fn dinamica_pelo_mcp(&mut self, args: &Json, origem: Option<(usize, Rect)>) -> Result<String, String> {
        let acao = args["acao"].as_str().unwrap_or("listar");
        let achar = |app: &App| -> Result<(usize, Json), String> {
            let id = args["id"].as_str().ok_or("Falta `id` (veja com acao listar).")?;
            todas(app).into_iter().find(|(_, d)| d["id"] == id)
                .ok_or_else(|| format!("Nenhuma dinâmica com id \"{id}\"."))
        };
        match acao {
            "listar" => {
                let lista: Vec<String> = todas(self).iter().map(|(aba, d)| descrever(self, *aba, d)).collect();
                Ok(if lista.is_empty() { "Nenhuma tabela dinâmica na pasta.".into() } else { lista.join("\n") })
            }
            "criar" => {
                let (aba, area) = origem.ok_or("Falta `origem`: a tabela com cabeçalho, ex.: A1:F500.")?;
                self.dinamica = None;
                self.folha = aba;
                self.linha = area.r0;
                self.coluna = area.c0;
                self.fim_selecao = (area.r1, area.c1);
                let _ = self.evento_dinamica(Evento::Abrir);
                let Some(mut e) = self.dinamica.take() else { return Err(self.status.clone()) };
                aplicar_args(self, &mut e, args)?;
                if e.linhas.is_empty() && e.colunas.is_empty() && e.valores.is_empty() {
                    return Err("Diga ao menos um campo em `linhas`, `colunas` ou `valores`.".into());
                }
                e.nova_aba = args["nova_aba"].as_bool().unwrap_or(true);
                e.fase = Fase::Campos;
                self.dinamica = Some(e);
                self.criar_dinamica_nativa();
                if self.dinamica.take().is_some() {
                    return Err(self.status.clone());
                }
                let (aba, d) = todas(self).into_iter().last().ok_or("A dinâmica não foi gravada.")?;
                Ok(format!("{} Criada: {}", self.status, descrever(self, aba, &d)))
            }
            "editar" | "atualizar" if acao == "atualizar" && args.get("id").is_none_or(Json::is_null) => {
                let mut feitas = Vec::new();
                for (aba, d) in todas(self) {
                    let e = edicao::ler_estado(self, aba, &d)?;
                    self.aplicar_com_resultado(aba, &e, None)?;
                    feitas.push(d["id"].as_str().unwrap_or("?").to_owned());
                }
                Ok(format!("{} dinâmica(s) atualizada(s): {}.", feitas.len(), feitas.join(", ")))
            }
            "editar" | "atualizar" => {
                let (aba, d) = achar(self)?;
                let mut e = edicao::ler_estado(self, aba, &d)?;
                if acao == "editar" {
                    aplicar_args(self, &mut e, args)?;
                }
                self.aplicar_com_resultado(aba, &e, None)?;
                let (aba, d) = achar(self)?;
                Ok(format!("{} {}", if acao == "editar" { "Editada:" } else { "Atualizada:" }, descrever(self, aba, &d)))
            }
            "excluir" => {
                let (aba, d) = achar(self)?;
                let area = retangulo_json(&d["areaResultado"]).ok_or("Área do resultado inválida.")?;
                let restantes: Vec<Json> = todas(self).into_iter()
                    .filter(|(i, x)| *i == aba && x["id"] != d["id"]).map(|(_, x)| x).collect();
                let novo = (!restantes.is_empty()).then(|| serde_json::to_string(&restantes).expect("JSON em memória"));
                self.workbook.editar(aba, |sheet| {
                    sheet.abrir_transacao();
                    sheet.clear(&area);
                    sheet.alterar_dinamicas(novo, &area);
                    sheet.fechar_transacao();
                });
                self.alterado = true;
                Ok(format!("Dinâmica {} excluída ({}:{} limpo).", d["id"].as_str().unwrap_or("?"),
                    cell_name(area.r0, area.c0), cell_name(area.r1, area.c1)))
            }
            outra => Err(format!("Ação \"{outra}\" desconhecida: listar, criar, editar, atualizar ou excluir.")),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn app_com_vendas() -> App {
        let mut app = App::default();
        app.workbook.editar(0, |s| {
            for (r, (regiao, produto, vendas)) in [("Região", "Produto", "Vendas"), ("Sul", "Maçã", "10"),
                ("Sul", "Pera", "20"), ("Norte", "Maçã", "5"), ("Norte", "Maçã", "")].into_iter().enumerate() {
                s.set_cell(r as u32, 0, regiao);
                s.set_cell(r as u32, 1, produto);
                s.set_cell(r as u32, 2, vendas);
            }
        });
        app
    }

    #[test]
    fn criar_editar_atualizar_e_excluir() {
        let mut app = app_com_vendas();
        let origem = Some((0, Rect::new(0, 0, 4, 2)));
        let t = app.dinamica_pelo_mcp(&json!({ "acao": "criar", "linhas": ["Região"],
            "valores": [{ "campo": "Vendas", "como": "media" }] }), origem).unwrap();
        assert!(t.contains("Criada") && t.contains("Região"), "{t}");
        let aba = app.workbook.len() - 1;
        let s = &app.workbook.aba(aba).unwrap().sheet;
        let linha = |s: &feathersheet_core::planilha::sheet::Sheet, rotulo: &str| (0..10).find(|&r| s.get_raw(r, 0) == rotulo).unwrap();
        let norte = linha(s, "Norte");
        assert_eq!(app.workbook.valor(aba, norte, 1), feathersheet_core::Value::Number(5.0), "vazio fica fora da média");

        let id = app.dinamica_pelo_mcp(&json!({ "acao": "listar" }), None).unwrap();
        let id = id.split_whitespace().next().unwrap().to_owned();
        let t = app.dinamica_pelo_mcp(&json!({ "acao": "editar", "id": id, "linhas": ["Produto"],
            "valores": [{ "campo": "C", "como": "soma" }] }), None).unwrap();
        assert!(t.contains("Produto"), "{t}");
        let s = &app.workbook.aba(aba).unwrap().sheet;
        assert_eq!(app.workbook.valor(aba, linha(s, "Maçã"), 1), feathersheet_core::Value::Number(15.0));

        app.workbook.editar(0, |s| s.set_cell(1, 2, "100"));
        app.dinamica_pelo_mcp(&json!({ "acao": "atualizar" }), None).unwrap();
        let s = &app.workbook.aba(aba).unwrap().sheet;
        assert_eq!(app.workbook.valor(aba, linha(s, "Maçã"), 1), feathersheet_core::Value::Number(105.0));

        let t = app.dinamica_pelo_mcp(&json!({ "acao": "excluir", "id": id }), None).unwrap();
        assert!(t.contains("excluída"), "{t}");
        assert!(app.dinamica_pelo_mcp(&json!({ "acao": "listar" }), None).unwrap().contains("Nenhuma"));
    }

    #[test]
    fn campo_que_nao_existe_explica_quais_existem() {
        let mut app = app_com_vendas();
        let erro = app.dinamica_pelo_mcp(&json!({ "acao": "criar", "linhas": ["Cidade"] }),
            Some((0, Rect::new(0, 0, 4, 2)))).unwrap_err();
        assert!(erro.contains("Região, Produto, Vendas"), "{erro}");
    }

    /// MCP-6: tema, subtotais, totais, campo calculado, item calculado e grupo pelo MCP.
    #[test]
    fn opcoes_da_dinamica_pelo_mcp() {
        let mut app = app_com_vendas();
        let origem = Some((0, Rect::new(0, 0, 4, 2)));
        let t = app.dinamica_pelo_mcp(&json!({ "acao": "criar", "linhas": ["Região", "Produto"], "tema": "verde",
            "subtotais": "embaixo", "totais_gerais": { "linha_embaixo": false },
            "campos_calculados": [{ "nome": "Dobro", "formula": "Vendas * 2" }],
            "valores": ["Vendas", "Dobro"],
            "itens_calculados": [{ "campo": "Região", "nome": "Sul menos Norte", "formula": "Sul - Norte" }] }), origem.clone()).unwrap();
        assert!(t.contains("tema verde") && t.contains("subtotais embaixo") && t.contains("campo calculado Dobro")
            && t.contains("item calculado Região: Sul menos Norte"), "{t}");
        let aba = app.workbook.len() - 1;
        let texto = |app: &App, r: u32, c: u32| feathersheet_core::celulas::values::to_text(&app.workbook.valor(aba, r, c));
        let achar = |app: &App, rotulo: &str| (0..20u32).find(|&r| texto(app, r, 0) == rotulo);
        let item = achar(&app, "Sul menos Norte").expect("linha do item calculado");
        // Sul 30, Norte 5; Dobro 60 e 10. No tabular com subtotal, o total do item vem na linha de subtotal.
        assert!((item..item + 4).any(|r| texto(&app, r, 2) == "25" && texto(&app, r, 3) == "50"), "{t}");
        assert!(achar(&app, "Total Geral").is_none(), "sem a linha de total embaixo");

        let id = t.split("Criada: ").nth(1).unwrap().split_whitespace().next().unwrap().to_owned();
        let t = app.dinamica_pelo_mcp(&json!({ "acao": "editar", "id": id, "itens_calculados": [],
            "grupos": [{ "campo": "Produto", "nome": "Frutas", "itens": ["Maçã", "Pera"] }] }), None).unwrap();
        assert!(t.contains("grupo Produto: Frutas") && !t.contains("item calculado"), "{t}");
        assert!(achar(&app, "Sul menos Norte").is_none());
        let erro = app.dinamica_pelo_mcp(&json!({ "acao": "editar", "id": id,
            "itens_calculados": [{ "campo": "Região", "nome": "X", "formula": "Leste - Sul" }] }), None).unwrap_err();
        assert!(erro.contains("Itens: Norte, Sul"), "{erro}");
        assert!(app.dinamica_pelo_mcp(&json!({ "acao": "editar", "id": id, "tema": "rosa" }), None).is_err());

        // Filtro, ordenação por valor e % do total geral numa dinâmica simples.
        let t = app.dinamica_pelo_mcp(&json!({ "acao": "criar", "linhas": ["Produto"], "valores": ["Vendas"],
            "ordenar": [{ "campo": "Produto", "ordem": "maior_valor" }], "mostrar_como": ["perc_total_geral"],
            "filtros": [{ "campo": "Região", "ocultar": ["Norte"] }] }), origem.clone()).unwrap();
        assert!(t.contains("filtro Região oculta [Norte]"), "{t}");
        let erro = app.dinamica_pelo_mcp(&json!({ "acao": "criar", "linhas": ["Produto"], "valores": ["Vendas"],
            "filtros": [{ "campo": "Região", "mostrar": ["Leste"] }] }), origem.clone()).unwrap_err();
        assert!(erro.contains("Itens: ") && erro.contains("Norte"), "item que não existe é recusado: {erro}");
        let aba = app.workbook.len() - 1;
        let texto = |r: u32, c: u32| feathersheet_core::celulas::values::to_text(&app.workbook.valor(aba, r, c));
        let primeira = (0..10u32).find(|&r| texto(r, 0) == "Pera" || texto(r, 0) == "Maçã").unwrap();
        // Sem o Norte: Pera 20 e Maçã 10 (de 30): Pera vem antes, com 2/3.
        assert_eq!(texto(primeira, 0), "Pera");
        assert!(texto(primeira, 1).starts_with("0.66") || texto(primeira, 1).starts_with("0,66"), "{}", texto(primeira, 1));
    }

    /// "Diferença de" com item base e recolher pelo MCP.
    #[test]
    fn diferenca_de_e_recolher_pelo_mcp() {
        let mut app = app_com_vendas();
        let origem = Some((0, Rect::new(0, 0, 4, 2)));
        let t = app.dinamica_pelo_mcp(&json!({ "acao": "criar", "linhas": ["Região", "Produto"], "valores": ["Vendas"], "layout": "compacto", "subtotais": "em_cima",
            "mostrar_como": [{ "como": "diferenca_de", "campo": "Região", "item": "Norte" }] }), origem).unwrap();
        let id = t.split("Criada: ").nth(1).unwrap().split_whitespace().next().unwrap().to_owned();
        let aba = app.workbook.len() - 1;
        let texto = |app: &App, r: u32, c: u32| feathersheet_core::celulas::values::to_text(&app.workbook.valor(aba, r, c));
        let sul = (0..12u32).find(|&r| texto(&app, r, 0) == "Sul").unwrap();
        assert_eq!(texto(&app, sul, 1), "25", "Sul 30 menos Norte 5");
        let t = app.dinamica_pelo_mcp(&json!({ "acao": "editar", "id": id, "recolher": ["Sul"] }), None).unwrap();
        assert!(t.contains("recolhidos [Sul]"), "{t}");
        let sul = (0..12u32).find(|&r| texto(&app, r, 0) == "Sul").unwrap();
        assert_ne!(texto(&app, sul + 1, 0), "Pera", "os itens do Sul somem");
        let erro = app.dinamica_pelo_mcp(&json!({ "acao": "editar", "id": id, "recolher": ["Leste"] }), None).unwrap_err();
        assert!(erro.contains("Exemplos: "), "{erro}");
        app.dinamica_pelo_mcp(&json!({ "acao": "editar", "id": id, "recolher": "nada" }), None).unwrap();
    }

    /// Agrupar em faixas pelo MCP.
    #[test]
    fn agrupar_em_faixas_pelo_mcp() {
        let mut app = app_com_vendas();
        app.dinamica_pelo_mcp(&json!({ "acao": "criar", "linhas": ["Vendas"], "valores": [{ "campo": "Vendas", "como": "contagem" }],
            "agrupar": [{ "campo": "Vendas", "faixas": [0, 30, 10] }] }), Some((0, Rect::new(0, 0, 4, 2)))).unwrap();
        let aba = app.workbook.len() - 1;
        let rotulos: Vec<String> = (0..10u32).map(|r| feathersheet_core::celulas::values::to_text(&app.workbook.valor(aba, r, 0))).collect();
        assert!(rotulos.iter().any(|r| r.starts_with("10")) && rotulos.iter().any(|r| r.starts_with("20")), "{rotulos:?}");
        assert!(app.dinamica_pelo_mcp(&json!({ "acao": "criar", "linhas": ["Vendas"],
            "agrupar": [{ "campo": "Vendas", "faixas": [10, 0, 5] }] }), Some((0, Rect::new(0, 0, 4, 2)))).is_err());
    }
}
