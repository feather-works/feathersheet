//! Cores da tabela dinâmica diretamente no Canvas (não modifica células).
//! Lê o mapa JSON uma vez por quadro, em vez de criar estilos por célula.
use feathersheet_core::{celulas::refs::Rect, planilha::sheet::Sheet};
use pelicano::Color;
use serde::Deserialize;

pub(crate) struct Visual {
    area: Rect,
    tema: String,
    tipos: Vec<char>,
    marcas: Vec<Vec<Marca>>,
    /// Filtros de relatório acima da tabela (campo | item) e, por linha, se
    /// o filtro esconde algo (funil cheio).
    filtros: Option<Rect>,
    filtrados: Vec<bool>,
}
#[derive(Clone)]
struct Marca {
    coluna: u32,
    caminho: String,
    recolhido: bool,
}

// Cache por thread e versão da aba: renderizações frequentes da grade
// não decodificam o mapa JSON novamente. Uma versão nova invalida tudo.
// A versão da aba nem sempre muda quando apenas o JSON da dinâmica muda:
// o metadata fica em sheet.dn, fora do índice de estilos/células. Acrescentar
// ponteiro e tamanho dessa String invalida cache sem varrer seu conteúdo
// completo em cada quadro. O novo JSON é alocado antes de substituir o antigo.
type CacheVisual = (usize, u32, usize, usize, std::sync::Arc<Vec<Visual>>);
thread_local! {
    static CACHE: std::cell::RefCell<Option<CacheVisual>> =
        const { std::cell::RefCell::new(None) };
}

pub(crate) fn com_cache(sheet: &Sheet, versao: u32) -> std::sync::Arc<Vec<Visual>> {
    let ponteiro = sheet as *const Sheet as usize;
    let (id_meta, tamanho) = sheet.dn.as_deref()
        .map(|d| (d.as_ptr() as usize, d.len())).unwrap_or((0,0));
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((id, v, p, l, ref visual)) = *cache {
            if id == ponteiro && v == versao && p == id_meta && l == tamanho {
                return visual.clone();
            }
        }
        let dados = std::sync::Arc::new(visuais(sheet));
        *cache = Some((ponteiro, versao, id_meta, tamanho, dados.clone()));
        dados
    })
}

#[derive(Deserialize)]
struct PivotDeArquivo {
    #[serde(rename = "areaResultado")]
    area: Option<Rect>,
    #[serde(default)]
    tema: Option<String>,
    #[serde(default)]
    mapa: Option<MapaCompacto>,
    #[serde(default, rename = "areaFiltros")]
    area_filtros: Option<Rect>,
    #[serde(default, rename = "filtrosPagina")]
    filtros_pagina: Vec<bool>,
}

#[derive(Deserialize, Default)]
struct MapaCompacto {
    #[serde(default)]
    linhas: Vec<(char, u32, Vec<(u32, String, bool)>)>,
}

pub(crate) fn visuais(sheet: &Sheet) -> Vec<Visual> {
    let Some(raw) = sheet.dn.as_deref() else { return Vec::new(); };
    // Serde ignora os demais atributos da dinâmica sem montar uma árvore
    // serde_json::Value enorme para cada linha do resultado.
    let Ok(lista) = serde_json::from_str::<Vec<PivotDeArquivo>>(raw) else {
        return Vec::new();
    };
    lista.into_iter().filter_map(|v| {
        let area = v.area?;
        if area.r0 > area.r1 || area.c0 > area.c1 { return None; }
        let mapa = v.mapa.unwrap_or_default();
        let tipos = mapa.linhas.iter().map(|linha|linha.0).collect();
        let marcas = mapa.linhas.into_iter().map(|(_,_,lista)| {
            lista.into_iter().map(|(coluna,caminho,recolhido)| {
                Marca { coluna, caminho, recolhido }
            }).collect()
        }).collect();
        let filtrados = v.filtros_pagina;
        Some(Visual {
            area, tema: v.tema.unwrap_or_else(|| "padrao".to_string()),
            tipos, marcas, filtros: v.area_filtros, filtrados,
        })
    }).collect()
}

fn tipo(v: &Visual, row: u32) -> char {
    if row == v.area.r0 { return 'c'; }
    if row == v.area.r1 { return 't'; }
    v.tipos.get((row-v.area.r0) as usize).copied().unwrap_or('i')
}

fn pertence(v: &Visual, row:u32,col:u32) -> bool {
    row >= v.area.r0 && row <= v.area.r1 && col >= v.area.c0 && col <= v.area.c1
}

fn cor_base(tema: &str, acento: Color) -> Color {
    match tema {
        "azul" => Color::rgb8(42,110,205),
        "verde" => Color::rgb8(34,137,94),
        "laranja" => Color::rgb8(200,111,30),
        "vermelho" => Color::rgb8(187,65,62),
        "roxo" => Color::rgb8(116,75,175),
        "cinza" => Color::rgb8(103,113,124),
        _ => acento,
    }
}

fn mistura(a:Color,b:Color,alpha:f32) -> Color {
    let inv = 1.0-alpha;
    Color { r:a.r*alpha+b.r*inv, g:a.g*alpha+b.g*inv,
        b:a.b*alpha+b.b*inv, a:1.0 }
}

pub(crate) fn fundo(lista: &[Visual], row:u32, col:u32, escuro:bool, acento:Color) -> Option<Color> {
    let visual = lista.iter().find(|v| pertence(v,row,col))?;
    let base = cor_base(&visual.tema,acento);
    let fundo = if escuro { Color::rgb8(34,37,42) } else { Color::WHITE };
    let classe = tipo(visual,row);
    Some(match classe {
        'c' => base,
        't' => mistura(base,fundo,if escuro {0.30}else{0.19}),
        's' => mistura(base,fundo,if escuro {0.25}else{0.12}),
        'g' => mistura(base,fundo,if escuro {0.17}else{0.075}),
        _ if (row-visual.area.r0) % 2 == 0 => mistura(base,fundo,if escuro {0.065}else{0.035}),
        _ => fundo,
    })
}

/// Marcador de recolher/expandir: somente na célula exata do mapa.
pub(crate) fn marca(lista:&[Visual],row:u32,col:u32) -> Option<(String,bool)> {
    let visual = lista.iter().find(|v| pertence(v,row,col))?;
    let indice = (row - visual.area.r0) as usize;
    let rel_coluna = col - visual.area.c0;
    let marca = visual.marcas.get(indice)?
        .iter().find(|m| m.coluna == rel_coluna)?;
    Some((marca.caminho.clone(), marca.recolhido))
}

pub(crate) fn recolhida(lista:&[Visual],row:u32,col:u32)->Option<bool> {
    let visual = lista.iter().find(|v| pertence(v,row,col))?;
    let indice = (row - visual.area.r0) as usize;
    let rel_coluna = col - visual.area.c0;
    visual.marcas.get(indice)?.iter().find(|m| m.coluna == rel_coluna)
        .map(|m|m.recolhido)
}

/// A célula do item de um filtro de relatório (a da direita, onde fica o
/// funil): o índice do filtro e se ele esconde algo.
pub(crate) fn filtro_na_grade(lista:&[Visual],row:u32,col:u32)->Option<(usize,bool)> {
    lista.iter().find_map(|v| {
        let f = v.filtros.as_ref()?;
        (col == f.c0 + 1 && row >= f.r0 && row <= f.r1).then(|| {
            let i = (row - f.r0) as usize;
            (i, v.filtrados.get(i).copied().unwrap_or(false))
        })
    })
}

pub(crate) fn destaque(lista:&[Visual],row:u32,col:u32)->bool {
    lista.iter().find(|v| pertence(v,row,col))
        .is_some_and(|v| matches!(tipo(v,row),'c'|'t'|'s'|'g'))
}

pub(crate) fn cabecalho(lista:&[Visual],row:u32,col:u32)->bool {
    lista.iter().find(|v| pertence(v,row,col))
        .is_some_and(|v| tipo(v,row) == 'c')
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn mapa_tipado_preserva_marcas_e_cache_por_versao() {
        let mut sheet = Sheet::new("Dados");
        sheet.dn = Some(r#"[{"tema":"roxo","areaResultado":{"r0":0,"c0":2,"r1":3,"c1":4},"mapa":{"linhas":[["c",0,[]],["g",0,[[0,"Sul",false]]],["i",1,[]],["t",0,[]]]}}]"#.into());
        let primeiro = com_cache(&sheet,5);
        assert_eq!(marca(&primeiro,1,2),Some(("Sul".into(),false)));
        assert_eq!(recolhida(&primeiro,1,2),Some(false));
        assert!(std::sync::Arc::ptr_eq(&primeiro,&com_cache(&sheet,5)));
        sheet.dn = Some(r#"[{"areaResultado":{"r0":0,"c0":2,"r1":1,"c1":4}}]"#.into());
        // Mesma versão de células, mas nova definição de dinâmica.
        let segundo = com_cache(&sheet,5);
        assert!(!std::sync::Arc::ptr_eq(&primeiro,&segundo));
        assert!(marca(&segundo,1,2).is_none());
    }

    #[test]
    fn visuais_de_dinamica_nao_criam_estilos_na_planilha() {
        let mut sheet = Sheet::new("Planilha1");
        sheet.dn = Some(r#"[{"tema":"verde","areaResultado":{"r0":0,"c0":0,"r1":2,"c1":1},"mapa":{"linhas":[["c",0,[]],["i",0,[]],["t",0,[]]]}}]"#.into());
        let p=visuais(&sheet);
        assert_eq!(p.len(),1);
        assert!(cabecalho(&p,0,1));
        assert!(destaque(&p,2,0));
        assert!(!destaque(&p,1,1));
        assert!(fundo(&p,0,0,false,Color::BLACK).is_some());
        assert!(fundo(&p,3,0,false,Color::BLACK).is_none());
        assert!(sheet.styles.is_empty());
    }
}
