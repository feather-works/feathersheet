//! UI-013: Ajuda nativa da planilha; corresponde à estrutura do TS.
//! A fonte dos tópicos é o JSON pt-BR original. Fórmulas vêm do core Rust,
//! macros do guia compilado em Rust e atalhos de tabelas portadas do TS.
use super::{ajuda_atalhos, ajuda_macros};
use feather_ui::ajuda::{Catalogo, Estado, Linha, Secao, Topico};
use feathersheet_core::{
    funcoes::function_help::{help_for, CATEGORIES},
    formula::assistente::aliases_of,
};
use serde_json::Value;
use std::sync::OnceLock;
const JSON_PT: &str = include_str!("../../../../src/idiomas/pt-BR.json");
const JSON_EN: &str = include_str!("../../../../src/idiomas/en-US.json");
const JSON_ES: &str = include_str!("../../../../src/idiomas/es-ES.json");
const TOPICOS: &[&str] = &[
    "comecar", "formulas", "preencher", "abas", "formatar", "tabelas",
    "graficos", "dados", "macros", "arquivos", "config",
];
fn tr(root: &Value, secao: &str, key: &str) -> String {
    root[secao][key].as_str().unwrap_or(key).to_owned()
}
fn topicos_uso(json: &Value) -> Vec<Topico> {
    TOPICOS.iter().map(|id| {
        let topic = &json["ajuda"]["comoUsar"][*id];
        Topico {
            id:(*id).into(),
            titulo:topic["titulo"].as_str().unwrap_or(id).into(),
            linhas:topic["corpo"].as_array().into_iter().flatten()
                .filter_map(Value::as_str).map(|l| {
                    match l.strip_prefix("• ") {
                        Some(item) => Linha::Marcador(item.to_owned()),
                        None => Linha::Texto(l.to_owned()),
                    }
                }).collect(),
        }
    }).collect()
}
fn topicos_funcoes(json: &Value, idioma: &str) -> Vec<Topico> {
    CATEGORIES.iter().map(|(categoria, nomes)| {
        let mut linhas=Vec::new();
        for nome in *nomes {
            if let Some(ajuda) = help_for(nome) {
                let apelidos = aliases_of(nome);
                let lingua = crate::idioma::Lingua::de_codigo(idioma);
                // O nome do de-para do idioma (SOMA, SUM, SUMA).
                let principal = crate::idioma::funcoes_de(lingua).nomes.get(*nome).map(String::as_str)
                    .unwrap_or_else(|| if idioma.starts_with("pt") { apelidos.first().copied().unwrap_or(nome) } else { nome });
                let (args, doc) = crate::idioma::ajuda_funcao_em(nome, idioma)
                    .unwrap_or_else(|| (ajuda.args.iter().map(|a| (*a).to_owned()).collect(), ajuda.doc.to_owned()));
                let sep = if idioma.starts_with("en") { ", " } else { "; " };
                linhas.push(Linha::Exemplo(format!("{}({})", principal, args.join(sep))));
                linhas.push(Linha::Texto(doc));
                // Português + nome internacional permanecem encontráveis.
                if !apelidos.is_empty() {
                    linhas.push(Linha::Texto(format!(
                        "{}: {}, {}",json["ajuda"]["rotulos"]["apelidos"]
                            .as_str().unwrap_or(tr!("Também")),nome,apelidos.join(", ")
                    )));
                }
            }
        }
        Topico {
            id:(*categoria).to_owned(),
            titulo:json["funcoes"]["categorias"][*categoria].as_str()
                .or_else(|| (!idioma.starts_with("pt")).then(||
                    crate::idioma::funcoes_de(crate::idioma::Lingua::de_codigo(idioma))
                        .categorias.get(*categoria).map(String::as_str)).flatten())
                .unwrap_or(categoria).into(),
            linhas,
        }
    }).collect()
}
fn secao(json: &Value, id: &str, topicos: Vec<Topico>) -> Secao {
    Secao {
        id:id.into(),
        titulo:tr(&json["ajuda"],"rotulos",id),
        introducao:tr(&json["ajuda"],"introducoes",id),
        topicos,
    }
}
fn construir(idioma:&str) -> Catalogo {
    let origem = if idioma.starts_with("en") { JSON_EN }
        else if idioma.starts_with("es") { JSON_ES } else { JSON_PT };
    let json:Value=serde_json::from_str(origem).expect("Ajuda de planilha inválida");
    Catalogo {
        titulo:tr(&json["ajuda"],"rotulos","titulo"),
        busca_rotulo:tr(&json["ajuda"],"rotulos","busca"),
        nada_encontrado:tr(&json["ajuda"],"rotulos","semResultado"),
        anterior:tr(&json["ajuda"],"rotulos","anterior"),
        proximo:tr(&json["ajuda"],"rotulos","proximo"),
        nesta_secao:tr(&json["ajuda"],"rotulos","nestaSecao"),
        secoes:vec![
            secao(&json,"uso",topicos_uso(&json)),
            secao(&json,"funcoes",topicos_funcoes(&json, idioma)),
            secao(&json,"macros",ajuda_macros::montar(&json)),
            secao(&json,"atalhos",ajuda_atalhos::montar(&json)),
        ],
    }
}
/// Idioma da Ajuda: segue Configurações → Idioma (ver [`definir_idioma`]).
static IDIOMA_AJUDA: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Chamado na abertura e ao trocar o idioma nas Configurações.
pub fn definir_idioma(idioma: &str) {
    // A interface inteira (menus, janelas, status) segue o mesmo idioma.
    crate::idioma::definir(idioma);
    // Global do processo: nos testes (paralelos) a Ajuda fica sempre em pt-BR.
    if cfg!(test) { return; }
    let i = match crate::idioma::Lingua::de_codigo(idioma) {
        crate::idioma::Lingua::Pt => 0, crate::idioma::Lingua::En => 1, crate::idioma::Lingua::Es => 2,
    };
    IDIOMA_AJUDA.store(i, std::sync::atomic::Ordering::Relaxed);
}
pub fn modelo() -> &'static Catalogo {
    static PT:OnceLock<Catalogo>=OnceLock::new();
    static EN:OnceLock<Catalogo>=OnceLock::new();
    static ES:OnceLock<Catalogo>=OnceLock::new();
    match IDIOMA_AJUDA.load(std::sync::atomic::Ordering::Relaxed) {
        1 => EN.get_or_init(|| construir("en-US")),
        2 => ES.get_or_init(|| construir("es-ES")),
        _ => PT.get_or_init(|| construir("pt-BR")),
    }
}
/// F1 dentro de fórmula: busca a assinatura da função ativa; não altera célula.
pub fn abrir_funcao(estado:&mut Estado, nome: &str) {
    estado.abrir(modelo(),"funcoes",None);
    if !nome.trim().is_empty() {
        estado.busca=nome.trim().to_owned();
    }
}
/// Texto que o evento pede para pôr na área de transferência (botão Copiar).
pub fn texto_a_copiar(evento: &feather_ui::ajuda::Evento) -> Option<&str> {
    match evento {
        feather_ui::ajuda::Evento::Copiar(texto) => Some(texto),
        _ => None,
    }
}
/// Só para a captura de tela: vai direto à página do primeiro tópico que tem um
/// exemplo com o termo da busca (em vez da lista de resultados).
pub fn abrir_pagina_da_busca(estado: &mut Estado) {
    let termo = feather_ui::ajuda::normalizar(estado.busca.trim());
    let achado = modelo().secoes.iter().enumerate().find_map(|(s, sec)| {
        sec.topicos.iter().position(|t| t.linhas.iter().any(|l| matches!(l,
            Linha::Exemplo(x) if feather_ui::ajuda::normalizar(x).contains(&termo))))
            .map(|i| (s, i))
    });
    if let Some((s, i)) = achado {
        estado.secao = s;
        estado.topico = Some(i);
        estado.busca.clear();
    }
}
/// Primeiro exemplo (sintaxe/código) da página aberta;
/// usado só pela captura de tela para simular o clique em Copiar.
pub fn primeiro_exemplo(estado: &Estado) -> Option<String> {
    let cat = modelo();
    let topico = cat.secoes.get(estado.secao)?.topicos.get(estado.topico?)?;
    topico.linhas.iter().find_map(|l| match l { Linha::Exemplo(x) => Some(x.clone()), _ => None })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn ambos_os_idiomas_carregam_guia_completo() {
        for idioma in ["pt-BR","en-US","es-ES"] {
            let c=construir(idioma);
            assert_eq!(c.secoes.len(),4);
            assert_eq!(c.secoes[2].topicos.len(),15);
            assert_eq!(c.secoes[3].topicos.len(),2);
        }
    }
    #[test] fn migracao_cobre_quatro_secoes_todas_as_funcoes_e_macros() {
        let cat=construir("pt-BR");
        assert_eq!(cat.secoes.iter().map(|s|s.id.as_str()).collect::<Vec<_>>(),
            ["uso","funcoes","macros","atalhos"]);
        assert_eq!(cat.secoes[0].topicos.len(),11);
        // Um tópico por categoria do catálogo de funções do motor.
        assert_eq!(cat.secoes[1].topicos.len(),CATEGORIES.len());
        assert_eq!(cat.secoes[2].topicos.len(),15);
        assert_eq!(cat.secoes[3].topicos.len(),2);
        // Cada função consta do catálogo compilado no core, não da lista TS.
        for (_,nomes) in CATEGORIES {
            for nome in *nomes {assert!(help_for(nome).is_some(),"{nome}");}
        }
        assert!(cat.quantidade_encontrada("ctrl+shift+v")>0);
        assert!(cat.quantidade_encontrada("sheet.set")>0);
        assert!(cat.quantidade_encontrada("soma")>0);
    }
    #[test] fn f1_em_formula_abre_busca_da_funcao_sem_editar_planilha() {
        let mut estado=Estado::default();
        abrir_funcao(&mut estado,"PROCV");
        assert_eq!(estado.secao,1);
        assert_eq!(estado.busca,"PROCV");
        assert!(modelo().quantidade_encontrada("PROCV")>0);
    }
    #[test] fn copiar_leva_o_texto_exato_do_exemplo_e_marca_copiado() {
        use feather_ui::ajuda::Evento;
        // Cada exemplo (sintaxe de função ou código de macro) vira a mensagem de copiar
        // com o mesmo texto exibido, nos dois idiomas.
        for idioma in ["pt-BR","en-US","es-ES"] {
            let cat=construir(idioma);
            let exemplos:Vec<&String>=cat.secoes.iter().flat_map(|s|&s.topicos)
                .flat_map(|t|&t.linhas).filter_map(|l|match l {Linha::Exemplo(x)=>Some(x),_=>None}).collect();
            assert!(exemplos.len()>300,"{idioma}: {}",exemplos.len());
            for x in exemplos {
                let ev=Evento::Copiar(x.clone());
                assert_eq!(texto_a_copiar(&ev),Some(x.as_str()));
            }
        }
        // pt-BR copia a fórmula em português com `;`; en-US em inglês com `,`.
        let pt=construir("pt-BR");
        assert!(pt.secoes[1].topicos.iter().flat_map(|t|&t.linhas)
            .any(|l|matches!(l,Linha::Exemplo(x) if x.starts_with("SOMA(") && x.contains(';'))));
        let mut estado=Estado::default();
        let texto="SOMA(valor1; valor2)".to_string();
        estado.atualizar(Evento::Copiar(texto.clone()),&pt);
        assert_eq!(estado.copiado.as_deref(),Some(texto.as_str()));
        let _:pelicano::Element<'_,Evento>=pt.view(&estado);
        estado.atualizar(Evento::LimparCopiado,&pt);
        assert!(estado.copiado.is_none());
        estado.atualizar(Evento::Copiar(texto),&pt);
        estado.atualizar(Evento::Buscar("x".into()),&pt);
        assert!(estado.copiado.is_none());
        assert!(texto_a_copiar(&Evento::Proximo).is_none());
    }
}
