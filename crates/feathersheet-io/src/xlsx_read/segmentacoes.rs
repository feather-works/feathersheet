//! Segmentação de dados de tabela dinâmica vinda do `.xlsx` (#145): o `xl/slicerCaches/slicerCacheK.xml` diz o
//! campo (`sourceName`), as dinâmicas que ele filtra (`tabId` + nome) e, item a item do cache, se está escolhido.
//! O `mod.rs` casa isso com as dinâmicas lidas e grava `segmentacoes` (e `liga`) no JSON da primeira.
// shortcut: segmentação de tabela comum (`x15:tableSlicerCache`, sem `tabular`) não converte e segue no aviso de
// "não convertido".
use super::xml::XmlNode;

pub(super) struct Lida {
    pub campo: String,
    /// (`sheetId`, nome da dinâmica), na ordem do arquivo: a primeira é a dona, as outras são as ligadas.
    pub dinamicas: Vec<(String, String)>,
    /// (`x`, escolhido) de cada item; nenhum desmarcado = sem filtro.
    pub itens: Vec<(usize, bool)>,
}

pub(super) fn ler(raiz: &XmlNode) -> Option<Lida> {
    let tabular = raiz.child("data")?.child("tabular")?;
    let itens = tabular.child("items").map(|i| i.children_named("i").filter_map(|i| {
        Some((i.attr("x")?.parse().ok()?, matches!(i.attr("s"), Some("1") | Some("true"))))
    }).collect()).unwrap_or_default();
    let dinamicas = raiz.child("pivotTables")?.children_named("pivotTable")
        .filter_map(|p| Some((p.attr("tabId")?.to_string(), p.attr("name")?.to_string()))).collect::<Vec<_>>();
    (!dinamicas.is_empty()).then(|| Lida { campo: raiz.attr("sourceName").unwrap_or_default().to_string(), dinamicas, itens })
}
