//! Catálogo de menus herdado da interface TypeScript.
//! Os identificadores de ações, e não os rótulos traduzidos, são a fonte de verdade.
use serde::Deserialize;
use serde_json::Value;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
pub struct Catalogo {
    pub menus: Vec<Menu>,
}

#[derive(Debug, Deserialize)]
pub struct Menu {
    pub titulo: String,
    pub itens: Vec<Item>,
}

#[derive(Debug, Deserialize)]
pub struct Item {
    pub rotulo: String,
    #[serde(default)]
    pub icone: String,
    #[serde(default)]
    pub atalho: String,
    #[serde(default)]
    pub separador: bool,
    #[serde(default)]
    pub filhos: Vec<Item>,
    pub acao: Option<Acao>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Acao {
    pub id: String,
    #[serde(default)]
    pub args: Vec<Value>,
}

pub fn catalogo() -> &'static Catalogo {
    static DADOS: OnceLock<Catalogo> = OnceLock::new();
    DADOS.get_or_init(|| {
        serde_json::from_str(include_str!("../../dados/menus.json"))
            .expect("O catálogo original de menus precisa ser um JSON válido")
    })
}

/// Percorre submenus arbitrariamente profundos sem aplaná-los.
pub fn itens(grupo: usize, caminho: &[usize]) -> Option<&'static [Item]> {
    let mut atual = catalogo().menus.get(grupo)?.itens.as_slice();
    for &indice in caminho {
        atual = atual.get(indice)?.filhos.as_slice();
    }
    Some(atual)
}

/// Adapta o catálogo JSON existente para o modelo comum sem perder índices,
/// atalhos implícitos, submenus, ações ou o formato de arquivo da planilha.
/// O renderizador antigo permanece até teste visual da UI-005.
/// Atualmente só é exercitado pelos testes de equivalência do catálogo:
/// não compilar este adaptador no binário enquanto a GUI não o consumir.
#[cfg(test)]
pub fn modelo_compartilhado(grupo: usize) -> Option<Vec<feather_ui::ribbon::ItemMenu<Acao>>> {
    use feather_ui::ribbon::ItemMenu;
    fn converter(item: &Item, indice: usize) -> ItemMenu<Acao> {
        let id = item.acao.as_ref().map(|a| a.id.clone())
            .unwrap_or_else(|| format!("item_{indice}"));
        if !item.filhos.is_empty() {
            ItemMenu::submenu(
                id, item.rotulo.clone(),
                item.filhos.iter().enumerate()
                    .map(|(i, filho)| converter(filho, i)).collect(),
            )
        } else if !item.separador {
            match &item.acao {
                Some(acao) => ItemMenu::acao(id, item.rotulo.clone(), acao.clone()),
                None => ItemMenu::inerte(id, item.rotulo.clone()),
            }
        } else {
            ItemMenu::inerte(id, item.rotulo.clone())
        }
    }
    Some(catalogo().menus.get(grupo)?.itens.iter().enumerate()
        .map(|(indice, item)| converter(item, indice)).collect())
}

/// Nomes dos ícones da fonte Tabler compartilhada no feather-ui.
/// Ícones ainda sem equivalente usam o glifo genérico do catálogo.
/// Todos os nomes não vazios do catálogo TypeScript são traduzidos para
/// recursos SVG que realmente existem em feather-ui/dados/tabler-acoes.tsv.
pub fn icone_tabler(nome: &str) -> &'static str {
    match nome {
        "inicio" => "layout-sidebar",
        "arquivo" | "pagina" => "file-plus",
        "abrir" => "folder-open",
        "baixar" => "device-floppy",
        "copiar" | "duplicados" => "copy",
        "recortar" => "cut",
        "colar" | "colar-valores" => "clipboard",
        "imprimir" => "printer",
        "desfazer" => "arrow-back-up",
        "refazer" => "arrow-forward-up",
        "excluir" => "trash",
        "buscar" | "pesquisar" => "search",
        "mostrar" => "list-details",
        "congelar" | "cadeado" => "lock",
        "zoom" => "zoom-in",
        "tela" => "arrows-maximize",
        "linha" | "coluna" | "tabela" => "table",
        "grafico" | "estatistica" => "chart-bar",
        "imagem" => "photo",
        "funcao" => "math-function",
        "nomeado" => "tag",
        "paleta" | "condicional" => "palette",
        "numero" => "list-numbers",
        "texto" | "quebra" => "text-wrap",
        "alinhamento" => "align-center",
        "mesclar" => "square",
        "fonte" => "bold",
        "limpar" => "eraser",
        "ordenar" => "sort-ascending",
        "filtro" => "filter",
        "dividir" => "columns",
        "macros" => "code",
        "executar" => "player-play",
        "mais" => "plus",
        "lapis" => "edit",
        "ajuda" => "help",
        "config" | "configuracoes" => "settings",
        "teclado" => "keyboard",
        "rocket" => "rocket",
        "sparkles" => "sparkles",
        _ => "list",
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn modelo_ribbon_conserva_caminhos_e_callbacks_do_menu_original() {
        let catalogo = catalogo();
        for (g, menu) in catalogo.menus.iter().enumerate() {
            let adaptado = modelo_compartilhado(g).unwrap();
            assert_eq!(adaptado.len(), menu.itens.len());
            fn comparar(
                nativos: &[Item], novos: &[feather_ui::ribbon::ItemMenu<Acao>],
            ) {
                assert_eq!(nativos.len(), novos.len());
                for (origem, novo) in nativos.iter().zip(novos) {
                    assert_eq!(origem.rotulo, novo.rotulo);
                    assert_eq!(origem.filhos.len(), novo.filhos.len());
                    if origem.filhos.is_empty() {
                        assert_eq!(origem.acao.is_some() && !origem.separador,
                                   novo.mensagem.is_some());
                    } else {
                        comparar(&origem.filhos, &novo.filhos);
                        assert!(novo.mensagem.is_none());
                    }
                }
            }
            comparar(&menu.itens, &adaptado);
        }
        assert!(modelo_compartilhado(usize::MAX).is_none());
    }

    #[test]
    fn todo_item_com_acao_vira_comando() {
        fn mortos(app: &crate::app::App, itens: &[Item], saida: &mut Vec<String>) {
            for item in itens {
                // Dependem do estado: desmesclar só com mescla, excluir aba só com duas.
                let do_estado = item.acao.as_ref().is_some_and(|a| matches!(a.id.as_str(), "desmesclar" | "deleteSheet"));
                if !do_estado && item.acao.as_ref().is_some_and(|a| app.evento_menu(a).is_none()) {
                    saida.push(format!("{} ({})", item.rotulo, item.acao.as_ref().unwrap().id));
                }
                mortos(app, &item.filhos, saida);
            }
        }
        let app = crate::app::App::default();
        let mut saida = Vec::new();
        for menu in &catalogo().menus { mortos(&app, &menu.itens, &mut saida); }
        assert!(saida.is_empty(), "itens de menu mortos: {saida:#?}");
    }

    #[test]
    fn catalogo_mantem_ordem_e_submenus_originais() {
        let dados = catalogo();
        assert_eq!(dados.menus.first().unwrap().titulo, "Arquivo");
        assert!(dados.menus.iter().any(|m| m.titulo == "Formatar"));
        let arquivo = itens(0, &[]).unwrap();
        let exportar = arquivo.iter().position(|i| i.rotulo == "Exportar").unwrap();
        let formatos = itens(0, &[exportar]).unwrap();
        assert!(formatos.iter().any(|i|
            i.acao.as_ref().is_some_and(|a| a.id == "exportar"
                && a.args.first().and_then(Value::as_str) == Some("xlsx"))));
    }

    #[test]
    fn exportar_json_e_fazer_copia_do_ts_despacham_comandos_reais() {
        let app = super::super::App::default();
        let arquivo = itens(0, &[]).unwrap();
        let copia = arquivo.iter().find(|i| i.rotulo == "Fazer uma cópia").unwrap();
        assert!(matches!(
            copia.acao.as_ref().and_then(|a| app.evento_menu(a)),
            Some(super::super::Msg::MenuSalvarCopia),
        ));
        let exportar = arquivo.iter().find(|i| i.rotulo == "Exportar").unwrap();
        let json = exportar.filhos.iter().find(|i| i.acao.as_ref()
            .is_some_and(|a| a.id == "exportar"
                && a.args.first().and_then(Value::as_str) == Some("json"))).unwrap();
        assert!(matches!(
            json.acao.as_ref().and_then(|a| app.evento_menu(a)),
            Some(super::super::Msg::ExportarJson),
        ));
    }

    #[test]
    fn todos_os_icones_do_catalogo_apontam_para_svg_tabler_existente() {
        // O catálogo mora no feather-ui (repositório próprio): pergunta a ele.
        fn verificar(itens: &[Item]) {
            for item in itens {
                if !item.icone.is_empty() {
                    let nome = icone_tabler(&item.icone);
                    assert!(feather_ui::icones::existe(nome),
                        "Ícone TypeScript '{}' não tem SVG: {}", item.icone, nome);
                }
                verificar(&item.filhos);
            }
        }
        for menu in &catalogo().menus { verificar(&menu.itens); }
    }

    #[test]
    fn menu_legado_completo_tem_nove_grupos_e_acoes_adicionadas() {
        let esperado = ["Arquivo", "Editar", "Ver", "Inserir", "Formatar",
                        "Dados", "Macros", "Ferramentas", "Ajuda"];
        let atual: Vec<_> = catalogo().menus.iter().map(|m| m.titulo.as_str()).collect();
        assert_eq!(atual.as_slice(), esperado.as_slice());
        let mostrar = catalogo().menus.iter().find(|m| m.titulo == "Ver").unwrap()
            .itens.iter().find(|i| i.rotulo == "Mostrar").unwrap();
        assert!(mostrar.filhos.iter().any(|i| i.acao.as_ref()
            .is_some_and(|a| a.id == "toggleRealceEixos")));
        let colar = catalogo().menus.iter().find(|m| m.titulo == "Editar").unwrap()
            .itens.iter().find(|i| i.rotulo == "Colar especial").unwrap();
        assert!(colar.filhos.iter().any(|i| i.acao.as_ref()
            .is_some_and(|a| a.id == "pasteFormats")));
    }

    #[test]
    fn todas_as_folhas_com_acao_tem_identificador() {
        fn validar(itens: &[Item]) {
            for item in itens {
                if item.separador { continue; }
                if item.filhos.is_empty() {
                    // Itens dinâmicos podem não ter comando até a planilha ter dados.
                    if let Some(acao) = &item.acao {
                        assert!(!acao.id.trim().is_empty(), "{}", item.rotulo);
                    }
                } else {
                    validar(&item.filhos);
                }
            }
        }
        for menu in &catalogo().menus { validar(&menu.itens); }
    }
}
