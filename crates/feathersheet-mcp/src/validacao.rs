//! `sheet_validacao`: listar, criar, remover e limpar regras de validação de
//! dados — as mesmas `Validacao` do diálogo da faixa (`operacoes::validation`),
//! gravadas no `va` da aba com Ctrl+Z. Funciona com e sem o aplicativo.
use feathersheet_core::celulas::refs::Rect;
use feathersheet_core::operacoes::validation::{MensagensValidacao, QuandoValidacao, Regra, Validacao};
use serde_json::Value as Json;

use crate::ferramentas::Saida;
use crate::hospede::Hospede;
use crate::texto::*;

fn regras(h: &dyn Hospede, aba: usize) -> Result<Vec<Validacao>, String> {
    let sheet = &h.pasta().aba(aba).ok_or("Aba inexistente")?.sheet;
    match sheet.va.as_deref() {
        None => Ok(Vec::new()),
        Some(t) => serde_json::from_str(t).map_err(|_| "As validações desta aba estão ilegíveis; nada foi alterado.".to_string()),
    }
}

fn gravar(h: &mut dyn Hospede, aba: usize, lista: &[Validacao], area: &Rect) {
    let texto = (!lista.is_empty()).then(|| serde_json::to_string(lista).expect("regras em memória"));
    h.pasta_mut().editar(aba, |sheet| { sheet.alterar_validacoes(texto, area); });
    h.marcar_alterado();
}

fn descrever(i: usize, v: &Validacao) -> String {
    let onde: Vec<String> = v.ranges.iter().map(nome_area).collect();
    let regra = serde_json::to_string(&v.regra).unwrap_or_default();
    let quando = match v.quando { QuandoValidacao::Recusar => "recusar", QuandoValidacao::Avisar => "avisar", QuandoValidacao::Informar => "informar" };
    format!("{} em {}: {regra} ({quando})", i + 1, onde.join(", "))
}

/// A regra pedida: `lista`, `origem` (fórmula/intervalo/nome) ou `regra` no formato do motor.
fn regra_de(args: &Json) -> Result<Regra, String> {
    if let Some(l) = args["lista"].as_array() {
        let valores: Vec<String> = l.iter().map(conteudo_da_ia).filter(|s| !s.trim().is_empty()).collect();
        if valores.is_empty() { return Err("`lista` vazia.".into()); }
        return Ok(Regra::Lista { valores });
    }
    if let Some(o) = arg_texto(args, "origem") {
        let formula = if o.starts_with('=') { o.to_string() } else { format!("={o}") };
        return Ok(Regra::Formula { formula });
    }
    if !args["regra"].is_object() {
        return Err("Informe `lista` ([\"Sim\", \"Não\"]), `origem` (\"=Dados!$A$2:$A$9\" ou um nome) ou `regra` (ex.: {\"tipo\": \"numero\", \"op\": \"entre\", \"a\": 1, \"b\": 10}).".into());
    }
    serde_json::from_value(args["regra"].clone()).map_err(|e| format!("`regra` inválida: {e}. Tipos: lista {{valores}}, numero {{op, a, b?, inteiro?}}, data {{op: entre|antes|depois, a, b?}}, hora {{op, a, b?}}, texto {{op: contem|comeca|email|url, valor?}}, caixa {{marcado, desmarcado}}, formula {{formula}}. op do número: entre, fora, maior, menor, igual, diferente, maiorigual, menorigual."))
}

pub fn validacao(h: &mut dyn Hospede, args: &Json) -> Saida {
    match arg_texto(args, "acao").unwrap_or("listar") {
        "listar" => {
            let aba = aba_do_arg(h, args)?;
            let lista = regras(h, aba)?;
            let nome = &h.pasta().nomes()[aba];
            Ok(conteudo(if lista.is_empty() {
                format!("Nenhuma validação em {nome}.")
            } else {
                format!("{} validação(ões) em {nome}:\n{}", lista.len(),
                    lista.iter().enumerate().map(|(i, v)| descrever(i, v)).collect::<Vec<_>>().join("\n"))
            }))
        }
        "criar" => {
            let (aba, area) = intervalo(h, args, "intervalo")?;
            let quando = match arg_texto(args, "quando").unwrap_or("recusar") {
                "recusar" => QuandoValidacao::Recusar,
                "avisar" => QuandoValidacao::Avisar,
                "informar" => QuandoValidacao::Informar,
                outro => return Err(format!("`quando` \"{outro}\": use recusar, avisar ou informar.")),
            };
            let texto = |k: &str| arg_texto(args, k).map(str::to_owned);
            let nova = Validacao {
                ranges: vec![Rect { sheet: None, ..area.clone() }],
                regra: regra_de(args)?,
                quando,
                mensagem: texto("mensagem"),
                mensagens: MensagensValidacao { entrada_titulo: texto("entrada_titulo"), entrada_texto: texto("entrada_texto"), erro_titulo: texto("erro_titulo") },
            };
            let mut lista = regras(h, aba)?;
            let descricao = descrever(lista.len(), &nova);
            lista.push(nova);
            gravar(h, aba, &lista, &area);
            Ok(conteudo(format!("Validação criada: {descricao}")))
        }
        "remover" => {
            let aba = aba_do_arg(h, args)?;
            let mut lista = regras(h, aba)?;
            let n = args["numero"].as_u64().filter(|n| (1..=lista.len() as u64).contains(n))
                .ok_or_else(|| format!("`numero`: de 1 a {} (veja com acao listar).", lista.len()))? as usize;
            let removida = lista.remove(n - 1);
            let area = removida.ranges.first().cloned().unwrap_or_else(|| Rect::new(0, 0, 0, 0));
            gravar(h, aba, &lista, &area);
            Ok(conteudo(format!("Validação {n} removida ({} restante(s)).", lista.len())))
        }
        "limpar" => {
            // Com intervalo, só as regras que passam por ele; sem, a aba toda.
            let (aba, area) = match arg_texto(args, "intervalo") {
                Some(_) => { let (a, r) = intervalo(h, args, "intervalo")?; (a, Some(r)) }
                None => (aba_do_arg(h, args)?, None),
            };
            let mut lista = regras(h, aba)?;
            let antes = lista.len();
            lista.retain(|v| area.as_ref().is_some_and(|a| !v.ranges.iter()
                .any(|f| f.r0 <= a.r1 && a.r0 <= f.r1 && f.c0 <= a.c1 && a.c0 <= f.c1)));
            gravar(h, aba, &lista, area.as_ref().unwrap_or(&Rect::new(0, 0, 0, 0)));
            Ok(conteudo(format!("{} validação(ões) removida(s).", antes - lista.len())))
        }
        outra => Err(format!("Ação \"{outra}\" desconhecida: listar, criar, remover ou limpar.")),
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
    fn criar_listar_validar_e_desfazer() {
        let mut h = Avulso::default();
        texto(chamar(&mut h, "sheet_validacao", &json!({ "acao": "criar", "intervalo": "B2:B9", "lista": ["Sim", "Não"] })));
        let t = texto(chamar(&mut h, "sheet_validacao", &json!({ "acao": "criar", "intervalo": "C2",
            "regra": { "tipo": "numero", "op": "entre", "a": 1, "b": 10 }, "quando": "avisar", "mensagem": "De 1 a 10" })));
        assert!(t.contains("numero"), "{t}");
        let t = texto(chamar(&mut h, "sheet_validacao", &json!({ "acao": "listar" })));
        assert!(t.contains("2 validação") && t.contains("B2:B9") && t.contains("Sim") && t.contains("(avisar)"), "{t}");
        // A regra gravada é a que o motor entende: a lista vira as opções da célula.
        let regras: Vec<feathersheet_core::operacoes::validation::Validacao> =
            serde_json::from_str(h.pasta().aba(0).unwrap().sheet.va.as_deref().unwrap()).unwrap();
        assert_eq!(h.pasta().opcoes_da_lista(0, 1, 1, &regras[0].regra), Some(vec!["Sim".to_string(), "Não".to_string()]));
        assert!(chamar(&mut h, "sheet_validacao", &json!({ "acao": "criar", "intervalo": "D1", "regra": { "tipo": "xyz" } })).is_err());
        chamar(&mut h, "sheet_desfazer", &json!({})).unwrap();
        assert!(texto(chamar(&mut h, "sheet_validacao", &json!({ "acao": "listar" }))).contains("1 validação"));
        texto(chamar(&mut h, "sheet_validacao", &json!({ "acao": "limpar" })));
        assert!(h.pasta().aba(0).unwrap().sheet.va.is_none());
    }
}
