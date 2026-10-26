//! `sheet_botao`: botões de macro na grade (os mesmos do Inserir > Botão de
//! macro), no `bt` da aba com Ctrl+Z. Clicar no botão roda a macro; criar o
//! botão nunca roda nada.
use serde_json::{json, Value as Json};

use crate::ferramentas::Saida;
use crate::hospede::Hospede;
use crate::texto::*;

/// O mesmo teto do aplicativo (`botoes_macro.rs`).
const MAX_BOTOES: usize = 128;

fn listar(h: &dyn Hospede, aba: usize) -> Result<Vec<Json>, String> {
    match h.pasta().aba(aba).ok_or("Aba inexistente")?.sheet.bt.as_deref() {
        None => Ok(Vec::new()),
        Some(t) => serde_json::from_str(t).map_err(|_| "Os botões desta aba estão ilegíveis; nada foi alterado.".to_string()),
    }
}

fn gravar(h: &mut dyn Hospede, aba: usize, itens: &[Json]) {
    let texto = (!itens.is_empty()).then(|| serde_json::to_string(itens).expect("botões em memória"));
    h.pasta_mut().editar(aba, |sheet| { sheet.alterar_botoes(texto); });
    h.marcar_alterado();
}

fn descrever(v: &Json) -> String {
    let p = &v["position"];
    format!("{} \"{}\" → macro {} · em x {} y {}, {}×{} px", v["id"].as_str().unwrap_or("?"), v["label"].as_str().unwrap_or(""),
        v["macroName"].as_str().unwrap_or("?"), p["x"], p["y"], p["width"], p["height"])
}

/// A macro precisa existir no catálogo da pasta (o mesmo do `sheet_macro`).
fn conferir_macro(h: &dyn Hospede, nome: &str) -> Result<(), String> {
    let existe = h.pasta().macros.as_deref()
        .map(feather_macros::Catalogo::importar_json).transpose()?
        .is_some_and(|c| feathersheet_macros::procedimentos::codigo_da_macro(&c, nome).is_some());
    if existe { Ok(()) } else { Err(format!("Não existe a macro \"{nome}\": crie antes com sheet_macro acao escrever.")) }
}

pub fn botao(h: &mut dyn Hospede, args: &Json) -> Saida {
    let aba = aba_do_arg(h, args)?;
    let mut itens = listar(h, aba)?;
    match arg_texto(args, "acao").unwrap_or("listar") {
        "listar" => {
            let nome = &h.pasta().nomes()[aba];
            Ok(conteudo(if itens.is_empty() { format!("Nenhum botão de macro em {nome}.") } else {
                format!("{} botão(ões) em {nome}:\n{}", itens.len(), itens.iter().map(descrever).collect::<Vec<_>>().join("\n"))
            }))
        }
        acao @ ("criar" | "editar") => {
            let criar = acao == "criar";
            let indice = if criar {
                if itens.len() >= MAX_BOTOES { return Err("Limite de 128 botões por aba.".into()); }
                let id = (1..).map(|n| format!("mcp-btn-{n}")).find(|c| !itens.iter().any(|v| v["id"] == c.as_str())).expect("sempre há id livre");
                let macro_nome = arg_texto(args, "macro").ok_or("Falta `macro` (o nome de uma macro da pasta).")?;
                // Na célula pedida (padrão A1), com a folga que o app usa ao inserir.
                let celula = arg_texto(args, "celula").unwrap_or("A1");
                let (linha, coluna) = feathersheet_core::celulas::refs::parse_cell_ref(celula)
                    .ok_or_else(|| format!("`celula` \"{celula}\" inválida (ex.: B2)."))?;
                let (x, y) = crate::grafico::pixels(&h.pasta().aba(aba).ok_or("Aba inexistente")?.sheet, linha, coluna);
                itens.push(json!({ "id": id, "label": "Executar macro", "macroName": macro_nome,
                    "position": { "x": x + 12.0, "y": y + 4.0, "width": 145.0, "height": 31.0 } }));
                itens.len() - 1
            } else {
                let id = arg_texto(args, "id").ok_or("Falta `id` (veja com acao listar).")?;
                itens.iter().position(|v| v["id"] == id).ok_or_else(|| format!("Nenhum botão \"{id}\" nesta aba."))?
            };
            if let Some(m) = arg_texto(args, "macro") {
                conferir_macro(h, m)?;
                itens[indice]["macroName"] = json!(m);
            }
            if let Some(r) = arg_texto(args, "rotulo") {
                itens[indice]["label"] = json!(if r.trim().is_empty() { "Botão" } else { r.trim() });
            }
            // Os limites do app (`Posicao::ler`): fora deles o botão seria descartado ao abrir.
            for (arg, campo, min) in [("largura", "width", 40.0), ("altura", "height", 24.0)] {
                if let Some(v) = args[arg].as_f64() { itens[indice]["position"][campo] = json!(v.clamp(min, 4096.0)); }
            }
            if !criar {
                if let Some(celula) = arg_texto(args, "celula") {
                    let (linha, coluna) = feathersheet_core::celulas::refs::parse_cell_ref(celula)
                        .ok_or_else(|| format!("`celula` \"{celula}\" inválida (ex.: B2)."))?;
                    let (x, y) = crate::grafico::pixels(&h.pasta().aba(aba).ok_or("Aba inexistente")?.sheet, linha, coluna);
                    itens[indice]["position"]["x"] = json!(x + 12.0);
                    itens[indice]["position"]["y"] = json!(y + 4.0);
                }
            }
            let texto = descrever(&itens[indice]);
            gravar(h, aba, &itens);
            Ok(conteudo(format!("Botão {}: {texto}", if criar { "criado" } else { "alterado" })))
        }
        "remover" => {
            let id = arg_texto(args, "id").ok_or("Falta `id` (veja com acao listar).")?;
            let antes = itens.len();
            itens.retain(|v| v["id"] != id);
            if itens.len() == antes { return Err(format!("Nenhum botão \"{id}\" nesta aba.")); }
            gravar(h, aba, &itens);
            Ok(conteudo(format!("Botão {id} removido.")))
        }
        outra => Err(format!("Ação \"{outra}\" desconhecida: listar, criar, editar ou remover.")),
    }
}

#[cfg(test)]
mod testes {
    use crate::ferramentas::chamar;
    use crate::hospede::{Avulso, Hospede};
    use serde_json::json;

    fn texto(r: crate::ferramentas::Saida) -> String {
        r.unwrap().iter().filter_map(|c| c["text"].as_str()).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn criar_editar_remover_e_desfazer() {
        let mut h = Avulso::default();
        assert!(chamar(&mut h, "sheet_botao", &json!({ "acao": "criar", "macro": "Relatorio" })).unwrap_err().contains("Não existe a macro"));
        chamar(&mut h, "sheet_macro", &json!({ "acao": "escrever", "nome": "Relatorio", "codigo": "sheet.set('A1', 1);" })).unwrap();
        let t = texto(chamar(&mut h, "sheet_botao", &json!({ "acao": "criar", "macro": "Relatorio", "rotulo": "Gerar", "celula": "C2" })));
        // C2: duas colunas de 122 px e uma linha de 28, mais a folga do app.
        assert!(t.contains("mcp-btn-1 \"Gerar\" → macro Relatorio · em x 256.0 y 32.0, 145.0×31.0"), "{t}");
        let t = texto(chamar(&mut h, "sheet_botao", &json!({ "acao": "editar", "id": "mcp-btn-1", "largura": 10 })));
        assert!(t.contains("40.0×31.0"), "largura mínima do app: {t}");
        chamar(&mut h, "sheet_desfazer", &json!({})).unwrap();
        assert!(texto(chamar(&mut h, "sheet_botao", &json!({}))).contains("145.0×31.0"));
        texto(chamar(&mut h, "sheet_botao", &json!({ "acao": "remover", "id": "mcp-btn-1" })));
        assert!(h.pasta().aba(0).unwrap().sheet.bt.is_none());
    }
}
