//! Macros JavaScript (API `sheet`, a mesma do Alt+F11): listar, ler,
//! escrever, excluir e executar. O catálogo é o da própria pasta
//! (`Workbook::macros`, gravado no .fshz), o mesmo que o editor do aplicativo
//! mostra: cada arquivo leva as suas macros.
//!
//! Executar roda o código na pasta do hospedeiro; um erro reverte as escritas
//! da execução (o motor de macros já faz isso).
use feather_macros::{Catalogo, Macro};
use serde_json::Value as Json;

use crate::ferramentas::Saida;
use crate::hospede::Hospede;
use crate::texto::*;

fn carregar(h: &dyn Hospede) -> Result<Catalogo, String> {
    match h.pasta().macros.as_deref() {
        Some(texto) => Catalogo::importar_json(texto),
        None => Ok(Catalogo::default()),
    }
}

fn gravar(h: &mut dyn Hospede, catalogo: &Catalogo) -> Result<(), String> {
    let json = catalogo.exportar_json()?;
    h.pasta_mut().macros = (!catalogo.macros.is_empty()).then_some(json);
    Ok(())
}

pub fn macro_(h: &mut dyn Hospede, args: &Json) -> Saida {
    let acao = arg_texto(args, "acao").unwrap_or("listar");
    let nome = arg_texto(args, "nome");
    let mut catalogo = carregar(h)?;
    let texto = match acao {
        "listar" => {
            if catalogo.macros.is_empty() { "Nenhuma macro guardada.".into() } else {
                catalogo.macros.iter().map(|m| format!("{} ({} linha(s))", m.name, m.code.lines().count()))
                    .collect::<Vec<_>>().join("\n")
            }
        }
        "ler" => {
            let nome = nome.ok_or("Falta `nome`.")?;
            catalogo.obter(nome).map(|m| m.code.clone()).ok_or_else(|| format!("Não existe a macro \"{nome}\"."))?
        }
        "escrever" => {
            let nome = nome.ok_or("Falta `nome`.")?;
            let codigo = args.get("codigo").and_then(Json::as_str).ok_or("Falta `codigo`.")?;
            match catalogo.macros.iter_mut().find(|m| m.name == nome) {
                Some(m) => m.code = codigo.to_string(),
                None => catalogo.macros.push(Macro { name: nome.to_string(), code: codigo.to_string(), auto: false }),
            }
            // Passa pela mesma validação do editor (nome, tamanho, duplicata).
            let validado = Catalogo::importar_json(&catalogo.exportar_json()?)?;
            gravar(h, &validado)?;
            h.macros_mudaram();
            format!("Macro \"{nome}\" guardada. Ela não roda sozinha: use acao \"executar\".")
        }
        "excluir" => {
            let nome = nome.ok_or("Falta `nome`.")?;
            let antes = catalogo.macros.len();
            catalogo.macros.retain(|m| m.name != nome);
            if catalogo.macros.len() == antes { return Err(format!("Não existe a macro \"{nome}\".")); }
            gravar(h, &catalogo)?;
            h.macros_mudaram();
            format!("Macro \"{nome}\" excluída.")
        }
        "executar" => {
            // Um texto numa célula podia levar a IA a rodar o que a pessoa não liberou (#127).
            if !h.macros_liberadas() {
                return Err("As macros deste arquivo estão desativadas. A pessoa precisa ativá-las em \"Ativar macros?\", na tela, antes de qualquer macro rodar.".into());
            }
            let codigo = match (args.get("codigo").and_then(Json::as_str), nome) {
                (Some(c), _) => c.to_string(),
                // `Módulo.função` chama só aquela função do módulo.
                (None, Some(n)) => feathersheet_macros::procedimentos::codigo_da_macro(&catalogo, n).map(|(_, c)| c)
                    .ok_or_else(|| format!("Não existe a macro \"{n}\"."))?,
                _ => return Err("Informe `nome` (macro guardada) ou `codigo`.".into()),
            };
            let aba = aba_do_arg(h, args)?;
            let r = feathersheet_macros::executar(h.pasta_mut(), aba, &codigo);
            if r.alterou { h.marcar_alterado(); }
            let mut s = r.saida.join("\n");
            match r.erro {
                Some(e) => s.push_str(&format!("\nErro: {e}\nAs escritas desta execução foram revertidas.")),
                None => s.push_str("\nMacro concluída."),
            }
            if let Some(st) = r.status { s.push_str(&format!("\nStatus: {st}")); }
            let pedido = r.formulario.as_ref().map(|(f, _)| f.clone());
            if r.fechar_formulario || pedido.is_some() || !r.campos.is_empty() {
                let aberto = h.formulario(r.fechar_formulario, r.formulario, r.campos);
                match pedido {
                    Some(form) if aberto => s.push_str(&format!("\nFormulário \"{form}\" aberto na tela.")),
                    Some(form) => s.push_str(&format!("\nA macro pediu o formulário \"{form}\", que só abre com o app.")),
                    None if r.fechar_formulario => s.push_str("\nFormulário fechado."),
                    None => {}
                }
            }
            s.trim_start().to_string()
        }
        "converter_vba" => converter_vba(h, &mut catalogo, args)?,
        outra => return Err(format!("Ação desconhecida: {outra}. Use listar, ler, escrever, excluir, executar ou converter_vba.")),
    };
    Ok(conteudo(texto))
}

/// O código de cada macro sai inteiro até este total; depois, só os nomes.
const MAX_CODIGO: usize = 30_000;

/// `converter_vba`: o VBA de um `.xlsm` (`arquivo`) ou colado (`vba`, módulo `nome`) vira as
/// macros JavaScript, com a lista do que não converteu. `guardar` põe as macros na pasta.
fn converter_vba(h: &mut dyn Hospede, catalogo: &mut Catalogo, args: &Json) -> Result<String, String> {
    let conversao = match (arg_texto(args, "arquivo"), arg_texto(args, "vba")) {
        (Some(caminho), _) => {
            let bytes = std::fs::read(caminho).map_err(|e| format!("Não consegui ler {caminho}: {e}"))?;
            let nome = std::path::Path::new(caminho).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            feathersheet_vba::converter_xlsm(&bytes, &nome)?
        }
        (None, Some(vba)) => feathersheet_vba::converter_texto(&[(arg_texto(args, "nome").unwrap_or("Module1"), vba)], &[]),
        _ => return Err("Informe `arquivo` (um .xlsm) ou `vba` (o código VBA).".into()),
    };
    let mut s = format!("{} linha(s) de VBA, {} instrução(ões) convertida(s), {} macro(s).\n",
        conversao.linhas_vba, conversao.instrucoes_convertidas, conversao.macros.len());
    let mut usado = 0;
    for m in &conversao.macros {
        if usado + m.codigo.len() <= MAX_CODIGO {
            s.push_str(&format!("\n// ===== {} (módulo {}) =====\n{}\n", m.nome, m.modulo, m.codigo));
            usado += m.codigo.len();
        } else {
            s.push_str(&format!("\n// ===== {} (módulo {}): código longo, leia com acao ler depois de guardar =====\n", m.nome, m.modulo));
        }
    }
    for e in &conversao.eventos {
        s.push_str(&format!("\nEvento {} {} (de {}) — o app liga pelo Importar VBA.", e.evento, e.aba.as_deref().unwrap_or("da pasta"), e.origem));
    }
    let (avisos, faltas): (Vec<_>, Vec<_>) = conversao.pendencias.iter().partition(|p| p.aviso);
    if !faltas.is_empty() {
        s.push_str(&format!("\n\nNão convertido ({}):", faltas.len()));
        for p in faltas.iter().take(40) { s.push_str(&format!("\n- {} linha {}: {} — {}", p.modulo, p.linha, p.vba.trim(), p.motivo)); }
    }
    if !avisos.is_empty() {
        s.push_str(&format!("\n\nConvertido, mas pode agir diferente ({}):", avisos.len()));
        for p in avisos.iter().take(20) { s.push_str(&format!("\n- {} linha {}: {} — {}", p.modulo, p.linha, p.vba.trim(), p.motivo)); }
    }
    if args.get("guardar").and_then(Json::as_bool).unwrap_or(false) {
        for m in &conversao.macros {
            match catalogo.macros.iter_mut().find(|x| x.name == m.nome) {
                Some(x) => x.code = m.codigo.clone(),
                None => catalogo.macros.push(Macro { name: m.nome.clone(), code: m.codigo.clone(), auto: false }),
            }
        }
        let validado = Catalogo::importar_json(&catalogo.exportar_json()?)?;
        gravar(h, &validado)?;
        h.macros_mudaram();
        s.push_str(&format!("\n\n{} macro(s) guardada(s) na pasta (não rodam sozinhas).", conversao.macros.len()));
    }
    Ok(s)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::hospede::Avulso;
    use serde_json::json;

    #[test]
    fn escrever_ler_executar_e_excluir() {
        let mut h = Avulso::default();
        let texto = |r: Saida| r.unwrap()[0]["text"].as_str().unwrap().to_string();
        texto(macro_(&mut h, &json!({ "acao": "escrever", "nome": "Dobro", "codigo": "sheet.range(\"B1\").value = 42; log(\"ok\");" })));
        // A macro fica na própria pasta (vai junto no .fshz).
        assert!(h.pasta.macros.as_deref().is_some_and(|t| t.contains("Dobro")));
        assert!(texto(macro_(&mut h, &json!({ "acao": "listar" }))).contains("Dobro"));
        let r = texto(macro_(&mut h, &json!({ "acao": "executar", "nome": "Dobro" })));
        assert!(r.contains("ok") && r.contains("concluída"), "{r}");
        assert_eq!(crate::texto::valor_texto(&h.pasta.valor(0, 0, 1)), "42");
        texto(macro_(&mut h, &json!({ "acao": "excluir", "nome": "Dobro" })));
        assert!(h.pasta.macros.is_none());
    }

    #[test]
    fn converter_vba_devolve_o_js_e_guarda() {
        let mut h = Avulso::default();
        let vba = "Sub Preencher()\n    Range(\"A1\").Value = 7\n    Set o = CreateObject(\"Outlook.Application\")\nEnd Sub\n";
        let r = macro_(&mut h, &json!({ "acao": "converter_vba", "vba": vba, "guardar": true })).unwrap();
        let r = r[0]["text"].as_str().unwrap();
        assert!(r.contains("Preencher") && r.contains("sheet."), "{r}");
        assert!(r.contains("Não convertido") && r.contains("CreateObject"), "o que não converte aparece: {r}");
        assert!(h.pasta.macros.as_deref().is_some_and(|t| t.contains("Preencher")), "guardada na pasta");
        let r = macro_(&mut h, &json!({ "acao": "executar", "nome": "Preencher" })).unwrap();
        assert!(r[0]["text"].as_str().unwrap().contains("concluída"), "{r:?}");
        assert_eq!(crate::texto::valor_texto(&h.pasta.valor(0, 0, 0)), "7");
        assert!(macro_(&mut h, &json!({ "acao": "converter_vba" })).is_err());
    }
}
