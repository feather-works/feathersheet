//! `sheet_formulario`: os formulários das macros (o UserForm do VBA), guardados
//! na pasta. A macro abre com `sheet.showComponent("nome")`; o botão do
//! formulário roda a macro dele com o preenchido em `valores`.
use feathersheet_macros::formularios::{self, Formulario};
use serde_json::Value as Json;

use crate::ferramentas::Saida;
use crate::hospede::Hospede;
use crate::texto::*;

fn gravar(h: &mut dyn Hospede, lista: &[Formulario]) -> Result<(), String> {
    h.pasta_mut().formularios = formularios::gravar(lista)?;
    h.marcar_alterado();
    Ok(())
}

pub fn formulario(h: &mut dyn Hospede, args: &Json) -> Saida {
    let mut lista = formularios::ler(h.pasta().formularios.as_deref())?;
    let nome = arg_texto(args, "nome");
    let texto = match arg_texto(args, "acao").unwrap_or("listar") {
        "listar" => if lista.is_empty() { "Nenhum formulário na pasta.".into() } else {
            lista.iter().map(|f| format!("{} \"{}\" · {}×{} px, {} controle(s)", f.nome, f.titulo, f.largura, f.altura, f.controles.len()))
                .collect::<Vec<_>>().join("\n")
        },
        "ler" => {
            let nome = nome.ok_or("Falta `nome`.")?;
            let f = formularios::obter(&lista, nome).ok_or_else(|| format!("Não existe o formulário \"{nome}\"."))?;
            serde_json::to_string_pretty(f).expect("formulário em memória")
        }
        "escrever" => {
            let f: Formulario = serde_json::from_value(args.get("formulario").cloned().ok_or("Falta `formulario`.")?)
                .map_err(|e| format!("`formulario` inválido: {e}"))?;
            let nome = f.nome.clone();
            // A macro de cada botão precisa existir, senão o clique só daria erro.
            let catalogo = h.pasta().macros.as_deref().map(feather_macros::Catalogo::importar_json).transpose()?.unwrap_or_default();
            if let Some(m) = f.controles.iter().filter_map(|c| c.macro_nome.as_deref()).find(|m| catalogo.obter(m).is_none()) {
                return Err(format!("Não existe a macro \"{m}\": crie antes com sheet_macro acao escrever."));
            }
            match lista.iter_mut().find(|g| g.nome.trim().eq_ignore_ascii_case(nome.trim())) {
                Some(g) => *g = f,
                None => lista.push(f),
            }
            gravar(h, &lista)?;
            format!("Formulário \"{nome}\" guardado. Abra pela macro com sheet.showComponent(\"{nome}\").")
        }
        "remover" => {
            let nome = nome.ok_or("Falta `nome`.")?;
            let antes = lista.len();
            lista.retain(|f| !f.nome.trim().eq_ignore_ascii_case(nome.trim()));
            if lista.len() == antes { return Err(format!("Não existe o formulário \"{nome}\".")); }
            gravar(h, &lista)?;
            format!("Formulário \"{nome}\" removido.")
        }
        outra => return Err(format!("Ação \"{outra}\" desconhecida: listar, ler, escrever ou remover.")),
    };
    Ok(conteudo(texto))
}

#[cfg(test)]
mod testes {
    use crate::ferramentas::chamar;
    use crate::hospede::{Avulso, Hospede};
    use serde_json::json;

    #[test]
    fn escrever_ler_e_remover() {
        let mut h = Avulso::default();
        let form = json!({ "nome": "cadastro", "titulo": "Cadastro", "controles": [
            { "tipo": "texto", "nome": "nome", "x": 10, "y": 10, "largura": 200, "altura": 26 },
            { "tipo": "botao", "nome": "", "texto": "Gravar", "macro": "Gravar", "x": 10, "y": 50, "largura": 100, "altura": 30, "limpar": true } ] });
        let erro = chamar(&mut h, "sheet_formulario", &json!({ "acao": "escrever", "formulario": form })).unwrap_err();
        assert!(erro.contains("Não existe a macro \"Gravar\""), "{erro}");
        chamar(&mut h, "sheet_macro", &json!({ "acao": "escrever", "nome": "Gravar", "codigo": "sheet.set('A1', valores.nome);" })).unwrap();
        chamar(&mut h, "sheet_formulario", &json!({ "acao": "escrever", "formulario": form })).unwrap();
        let lido = chamar(&mut h, "sheet_formulario", &json!({ "acao": "ler", "nome": "CADASTRO" })).unwrap();
        assert!(lido[0]["text"].as_str().unwrap().contains("\"limpar\": true"));
        chamar(&mut h, "sheet_formulario", &json!({ "acao": "remover", "nome": "cadastro" })).unwrap();
        assert!(h.pasta().formularios.is_none());
    }
}
