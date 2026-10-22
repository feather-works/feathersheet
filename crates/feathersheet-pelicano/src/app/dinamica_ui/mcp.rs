//! `sheet_dinamica` do MCP ao vivo: listar, criar, editar, atualizar e
//! excluir tabelas dinâmicas pelo mesmo caminho do painel (`Estado`,
//! `criar_dinamica_nativa`, `aplicar_estado_na_dinamica`) — o que a IA cria
//! é igual ao que a pessoa criaria com o mouse, e cada chamada é um Ctrl+Z.
use super::*;
use serde_json::Value as Json;

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

/// Põe em `e` o que veio nos argumentos (só o que veio: editar mexe no resto).
fn aplicar_args(e: &mut Estado, args: &Json) -> Result<(), String> {
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
    Ok(())
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
        format!(" · linhas [{}] · colunas [{}] · valores [{}]",
            e.linhas.iter().map(nome).collect::<Vec<_>>().join(", "),
            e.colunas.iter().map(nome).collect::<Vec<_>>().join(", "), valores.join(", "))
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
                aplicar_args(&mut e, args)?;
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
                    aplicar_args(&mut e, args)?;
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
}
